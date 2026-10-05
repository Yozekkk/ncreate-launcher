//! Validated local discovery and transactional Mojang runtimes, isolated from user Java.
use crate::{Engine, Error, Instance, JavaRuntime, Operation, Result};
use futures_util::{StreamExt, TryStreamExt, stream};
use serde::Deserialize;
use std::{
	collections::BTreeMap,
	path::{Path, PathBuf},
	time::Duration,
};

const RUNTIME_INDEX: &str = "https://piston-meta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";
const PREPARE_LIMIT: Duration = Duration::from_secs(120);
const PROBE_LIMIT: Duration = Duration::from_secs(5);
fn executable() -> &'static str {
	if cfg!(windows) { "java.exe" } else { "java" }
}
fn platform() -> Result<&'static str> {
	match (std::env::consts::OS, std::env::consts::ARCH) {
		("linux", "x86_64") => Ok("linux"),
		("windows", "x86_64") => Ok("windows-x64"),
		_ => Err(Error::Invalid(
			"Автоматическая Java недоступна для этой платформы".into(),
		)),
	}
}
fn java_error(instance: &Instance, required: u32, message: impl ToString) -> Error {
	Error::Java(serde_json::json!({"instance_id":instance.id,"minecraft":instance.game_version,"required":required,"message":message.to_string()}).to_string())
}
fn major(version: &str) -> Option<u32> {
	let version = version.strip_prefix("1.").unwrap_or(version);
	version
		.split(|c: char| !c.is_ascii_digit())
		.next()?
		.parse()
		.ok()
}
fn parse_probe(path: &Path, text: &str) -> Result<JavaRuntime> {
	let property = |name: &str| text.lines().find_map(|line| line.trim().strip_prefix(name));
	let version = property("java.version = ")
		.or_else(|| text.split('"').nth(1))
		.and_then(major)
		.filter(|v| *v > 0)
		.ok_or_else(|| Error::Invalid("Не удалось определить версию Java".into()))?;
	let architecture = property("os.arch = ")
		.ok_or_else(|| Error::Invalid("Java не сообщила архитектуру".into()))?;
	let compatible = match std::env::consts::ARCH {
		"x86_64" => matches!(architecture, "amd64" | "x86_64"),
		"aarch64" => matches!(architecture, "aarch64" | "arm64"),
		"x86" => matches!(architecture, "x86" | "i386" | "i686"),
		_ => false,
	};
	if !compatible {
		return Err(Error::Invalid(format!(
			"Java имеет неподходящую архитектуру: {architecture}"
		)));
	}
	Ok(JavaRuntime {
		path: path.to_string_lossy().into_owned(),
		major: version,
		architecture: architecture.into(),
	})
}
pub(crate) async fn check_java(path: &Path) -> Result<JavaRuntime> {
	use tokio::io::AsyncReadExt;
	let probe = async {
		let mut command = tokio::process::Command::new(path);
		command
			.args(["-XshowSettings:properties", "-version"])
			.kill_on_drop(true)
			.stdin(std::process::Stdio::null())
			.stdout(std::process::Stdio::piped())
			.stderr(std::process::Stdio::piped());
		#[cfg(windows)]
		command.creation_flags(0x08000000);
		let mut child = command.spawn()?;
		let stdout = child
			.stdout
			.take()
			.ok_or_else(|| Error::Invalid("Java stdout unavailable".into()))?;
		let stderr = child
			.stderr
			.take()
			.ok_or_else(|| Error::Invalid("Java stderr unavailable".into()))?;
		let read = async move {
			let mut out = Vec::new();
			let mut err = Vec::new();
			let mut stdout = stdout.take(65537);
			let mut stderr = stderr.take(65537);
			tokio::try_join!(stdout.read_to_end(&mut out), stderr.read_to_end(&mut err))?;
			if out.len() > 65536 || err.len() > 65536 {
				return Err(Error::Invalid("Java probe output exceeds limit".into()));
			}
			out.extend(err);
			Ok::<_, Error>(out)
		};
		let bytes = read.await?;
		if !child.wait().await?.success() {
			return Err(Error::Invalid(
				"Java не запускается: проверка завершилась с ошибкой".into(),
			));
		}
		parse_probe(path, &String::from_utf8_lossy(&bytes))
	};
	tokio::time::timeout(PROBE_LIMIT, probe)
		.await
		.map_err(|_| Error::Invalid("Проверка Java превысила 5 секунд".into()))?
}
fn validate_major(runtime: JavaRuntime, required: u32, minecraft: &str) -> Result<JavaRuntime> {
	if runtime.major != required {
		return Err(Error::Invalid(format!(
			"Для Minecraft {minecraft} требуется Java {required}, но выбран Java {}.",
			runtime.major
		)));
	}
	Ok(runtime)
}
#[derive(Clone, Deserialize)]
struct Artifact {
	url: String,
	sha1: String,
	size: u64,
}
#[derive(Deserialize)]
struct Release {
	manifest: Artifact,
	version: RuntimeVersion,
}
#[derive(Deserialize)]
struct RuntimeVersion {
	name: String,
}
#[derive(Deserialize)]
struct RuntimeManifest {
	files: BTreeMap<String, RuntimeFile>,
}
#[derive(Clone, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum RuntimeFile {
	Directory,
	File {
		downloads: BTreeMap<String, Artifact>,
		#[serde(default)]
		executable: bool,
	},
	Link {
		target: String,
	},
}
fn mojang_url(value: &str) -> Result<()> {
	let url =
		reqwest::Url::parse(value).map_err(|_| Error::Invalid("Invalid runtime URL".into()))?;
	if url.scheme() != "https"
		|| !matches!(
			url.host_str(),
			Some("piston-meta.mojang.com" | "piston-data.mojang.com" | "launcher.mojang.com")
		) || !url.username().is_empty()
		|| url.password().is_some()
		|| url.port().is_some()
	{
		return Err(Error::Invalid("Runtime source is not Mojang HTTPS".into()));
	}
	Ok(())
}
// Materialize links as copies. Neither Unix symlinks nor Windows privileges are needed.
fn link_target(name: &str, target: &str) -> Result<String> {
	if target.starts_with('/') || target.contains(['\\', ':']) {
		return Err(Error::Invalid("Unsafe runtime link".into()));
	}
	let mut parts: Vec<&str> = name.split('/').collect();
	parts.pop();
	for part in target.split('/') {
		match part {
			".." => {
				if parts.pop().is_none() {
					return Err(Error::Invalid("Runtime link escapes root".into()));
				}
			}
			"." => {}
			"" => return Err(Error::Invalid("Invalid runtime link".into())),
			part => parts.push(part),
		}
	}
	let result = parts.join("/");
	crate::files::safe_relative(&result)?;
	Ok(result)
}
impl Engine {
	pub(crate) async fn validate_instance_java(&self, instance: &Instance) -> Result<()> {
		let Some(path) = instance
			.java_path
			.as_deref()
			.filter(|p| !p.trim().is_empty())
		else {
			return Ok(());
		};
		let runtime = check_java(Path::new(path)).await?;
		let op = self.begin("java_requirement", Some(&instance.id));
		let result = tokio::time::timeout(Duration::from_secs(30), async {
			let stored: Option<String> =
				sqlx::query_scalar("SELECT data FROM launcher_manifests WHERE instance_id=?")
					.bind(&instance.id)
					.fetch_optional(&self.pool)
					.await?;
			let required = if let Some(stored) = stored {
				serde_json::from_str::<crate::EditionManifest>(&stored)?
					.java
					.major
			} else {
				self.version_info(instance, &op)
					.await?
					.java_version
					.as_ref()
					.map_or(8, |v| v.major_version)
			};
			validate_major(runtime, required, &instance.game_version).map(|_| ())
		})
		.await
		.unwrap_or_else(|_| {
			Err(Error::Invalid(
				"Не удалось проверить требования Java за 30 секунд".into(),
			))
		});
		op.finish(&result);
		result
	}

	pub async fn java_runtimes(&self) -> Result<Vec<JavaRuntime>> {
		let discover = async {
			let mut candidates = Vec::new();
			if let Some(paths) = std::env::var_os("PATH") {
				for path in std::env::split_paths(&paths) {
					candidates.push(path.join(executable()));
				}
			}
			if let Some(home) = std::env::var_os("JAVA_HOME") {
				candidates.push(PathBuf::from(home).join("bin").join(executable()));
			}
			let mut roots = vec![self.root.join("java")];
			if cfg!(windows) {
				let program_files = std::env::var_os("ProgramFiles")
					.map(PathBuf::from)
					.unwrap_or_else(|| PathBuf::from("C:\\Program Files"));
				roots.extend([
					program_files.join("Java"),
					program_files.join("Eclipse Adoptium"),
					program_files.join("Microsoft"),
				]);
			} else {
				roots.extend([PathBuf::from("/usr/lib/jvm"), PathBuf::from("/usr/java")]);
			}
			for root in roots {
				if let Ok(mut entries) = tokio::fs::read_dir(root).await {
					let mut count = 0;
					while let Some(entry) = entries.next_entry().await? {
						if count >= 128 {
							break;
						}
						count += 1;
						if !entry.file_name().to_string_lossy().starts_with('.') {
							candidates.push(entry.path().join("bin").join(executable()));
						}
					}
				}
			}
			let mut unique = std::collections::HashSet::new();
			candidates.retain(|path| {
				path.is_file()
					&& unique.insert(std::fs::canonicalize(path).unwrap_or_else(|_| path.clone()))
			});
			candidates.truncate(128);
			Ok::<_, Error>(
				stream::iter(candidates)
					.map(|path| async move { check_java(&path).await.ok() })
					.buffer_unordered(8)
					.filter_map(|r| async move { r })
					.collect::<Vec<_>>()
					.await,
			)
		};
		tokio::time::timeout(Duration::from_secs(20), discover)
			.await
			.map_err(|_| Error::Invalid("Поиск Java превысил 20 секунд".into()))?
	}
	pub(crate) async fn choose_java(
		&self,
		instance: &Instance,
		required: u32,
		op: &Operation,
	) -> Result<JavaRuntime> {
		op.progress("java", 0, 0, &format!("Ищем и проверяем Java {required}…"));
		let prepare = async {
			if let Some(path) = instance
				.java_path
				.as_deref()
				.filter(|s| !s.trim().is_empty())
			{
				return validate_major(
					check_java(Path::new(path)).await?,
					required,
					&instance.game_version,
				);
			}
			if let Ok(runtimes) = self.java_runtimes().await
				&& let Some(runtime) = runtimes.into_iter().find(|r| r.major == required)
			{
				return Ok(runtime);
			}
			op.progress(
				"java",
				0,
				0,
				&format!("Загружаем проверенную Java {required} от Mojang…"),
			);
			self.managed_java(required, op).await
		};
		finish_preparation(instance, required, op.wait(prepare)).await
	}
}
async fn finish_preparation(
	instance: &Instance,
	required: u32,
	prepare: impl std::future::Future<Output = Result<JavaRuntime>>,
) -> Result<JavaRuntime> {
	match tokio::time::timeout(PREPARE_LIMIT, prepare).await {
		Ok(Ok(runtime)) => Ok(runtime),
		Ok(Err(Error::Cancelled)) => Err(Error::Cancelled),
		Ok(Err(error)) => Err(java_error(instance, required, error)),
		Err(_) => Err(java_error(
			instance,
			required,
			"Не удалось автоматически подготовить Java за 2 минуты. Выберите Java вручную или повторите попытку.",
		)),
	}
}
impl Engine {
	pub(crate) async fn managed_java(&self, required: u32, op: &Operation) -> Result<JavaRuntime> {
		let index: BTreeMap<String, BTreeMap<String, Vec<Release>>> =
			self.downloads.json(RUNTIME_INDEX, op).await?;
		let releases = index.get(platform()?).ok_or_else(|| {
			Error::Invalid("Mojang не предоставляет Java для этой платформы".into())
		})?;
		let release = releases
			.iter()
			.filter(|(key, _)| key.as_str() != "minecraft-java-exe")
			.flat_map(|(_, v)| v)
			.find(|r| major(&r.version.name) == Some(required))
			.ok_or_else(|| {
				Error::Invalid(format!(
					"Mojang не предоставляет Java {required}; выберите её вручную"
				))
			})?;
		mojang_url(&release.manifest.url)?;
		let bytes = self
			.downloads
			.bytes(&release.manifest.url, 8 * 1024 * 1024, op)
			.await?;
		crate::download::verify(&bytes, &release.manifest.sha1, "sha1")?;
		if bytes.len() as u64 != release.manifest.size {
			return Err(Error::Invalid("Runtime manifest size mismatch".into()));
		}
		let manifest: RuntimeManifest = serde_json::from_slice(&bytes)?;
		if manifest.files.len() > 8192 {
			return Err(Error::Invalid("Runtime manifest exceeds file limit".into()));
		}
		let root = self.root.join("java");
		tokio::fs::create_dir_all(&root).await?;
		let stage = tempfile::Builder::new()
			.prefix(".preparing-")
			.tempdir_in(&root)?;
		let mut total = 0u64;
		for (name, file) in &manifest.files {
			crate::files::safe_relative(name)?;
			if let RuntimeFile::File { downloads, .. } = file {
				let raw = downloads
					.get("raw")
					.ok_or_else(|| Error::Invalid("Runtime has no raw download".into()))?;
				mojang_url(&raw.url)?;
				total = total.saturating_add(raw.size);
			}
		}
		if total > 1024 * 1024 * 1024 {
			return Err(Error::Invalid("Runtime exceeds size limit".into()));
		}
		stream::iter(manifest.files.clone())
			.map(|(name, file)| {
				let path = stage.path().join(name);
				let manager = self.downloads.clone();
				let op = op.clone();
				async move {
					match file {
						RuntimeFile::Directory => tokio::fs::create_dir_all(path).await?,
						RuntimeFile::File {
							downloads,
							executable,
						} => {
							let raw = &downloads["raw"];
							manager
								.file(&raw.url, &path, &raw.sha1, "sha1", raw.size, &op)
								.await?;
							#[cfg(unix)]
							{
								use std::os::unix::fs::PermissionsExt;
								if executable {
									tokio::fs::set_permissions(
										&path,
										std::fs::Permissions::from_mode(0o755),
									)
									.await?;
								}
							}
							#[cfg(not(unix))]
							let _ = executable;
						}
						RuntimeFile::Link { .. } => {}
					}
					Ok::<_, Error>(())
				}
			})
			.buffer_unordered(6)
			.try_collect::<Vec<_>>()
			.await?;
		for (name, file) in &manifest.files {
			if let RuntimeFile::Link { target } = file {
				let target = link_target(name, target)?;
				let destination = stage.path().join(name);
				match manifest.files.get(&target) {
					Some(RuntimeFile::File { .. }) => {
						if let Some(parent) = destination.parent() {
							tokio::fs::create_dir_all(parent).await?;
						}
						tokio::fs::copy(stage.path().join(&target), &destination).await?;
					}
					Some(RuntimeFile::Directory) => {
						tokio::fs::create_dir_all(&destination).await?;
						let prefix = format!("{target}/");
						for (child, entry) in &manifest.files {
							if let Some(relative) = child.strip_prefix(&prefix) {
								let output = destination.join(relative);
								match entry {
									RuntimeFile::Directory => {
										tokio::fs::create_dir_all(output).await?
									}
									RuntimeFile::File { .. } => {
										if let Some(parent) = output.parent() {
											tokio::fs::create_dir_all(parent).await?;
										}
										tokio::fs::copy(stage.path().join(child), output).await?;
									}
									RuntimeFile::Link { .. } => {
										return Err(Error::Invalid(
											"Nested runtime link is not supported".into(),
										));
									}
								}
							}
						}
					}
					_ => {
						return Err(Error::Invalid(
							"Runtime link does not target verified content".into(),
						));
					}
				}
			}
		}
		op.check()?;
		let runtime = validate_major(
			check_java(&stage.path().join("bin").join(executable())).await?,
			required,
			"runtime",
		)?;
		op.check()?;
		let destination = root.join(format!("mojang-{required}-{}", uuid::Uuid::new_v4()));
		tokio::fs::rename(stage.path(), &destination).await?;
		Ok(JavaRuntime {
			path: destination
				.join("bin")
				.join(executable())
				.to_string_lossy()
				.into_owned(),
			..runtime
		})
	}
	pub async fn required_java(&self, instance_id: &str) -> Result<u32> {
		let instance = self.instance(instance_id).await?;
		let stored: Option<String> =
			sqlx::query_scalar("SELECT data FROM launcher_manifests WHERE instance_id=?")
				.bind(instance_id)
				.fetch_optional(&self.pool)
				.await?;
		if let Some(stored) = stored {
			return Ok(serde_json::from_str::<crate::EditionManifest>(&stored)?
				.java
				.major);
		}
		let op = self.begin("java_requirement", Some(instance_id));
		let result = tokio::time::timeout(Duration::from_secs(30), async {
			let path = self
				.instance_path(instance_id)?
				.join(".ncreate-runtime/version.json");
			let info = if instance.status == "ready" && path.is_file() {
				serde_json::from_slice(&tokio::fs::read(path).await?)?
			} else {
				self.version_info(&instance, &op).await?
			};
			Ok(info.java_version.as_ref().map_or(8, |v| v.major_version))
		})
		.await
		.unwrap_or_else(|_| {
			Err(Error::Invalid(
				"Не удалось получить требования Java за 30 секунд".into(),
			))
		});
		op.finish(&result);
		result
	}
	pub async fn validate_java(
		&self,
		path: &str,
		instance_id: Option<&str>,
	) -> Result<JavaRuntime> {
		let runtime = check_java(Path::new(path)).await?;
		if let Some(id) = instance_id {
			let required = self.required_java(id).await?;
			let instance = self.instance(id).await?;
			validate_major(runtime, required, &instance.game_version)
		} else {
			Ok(runtime)
		}
	}
	pub async fn select_java(&self, instance_id: &str, path: &str) -> Result<JavaRuntime> {
		let _guard = self.mutation.lock().await;
		self.ensure_stopped(instance_id).await?;
		let runtime = self.validate_java(path, Some(instance_id)).await?;
		let mut instance = self.instance(instance_id).await?;
		instance.java_path = Some(runtime.path.clone());
		self.save_instance(&instance).await?;
		Ok(runtime)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn metadata_generations_and_selection() {
		let mapping: Vec<serde_json::Value> =
			serde_json::from_str(include_str!("../tests/fixtures/java-requirements.json")).unwrap();
		assert!(mapping.len() > 80);
		for (version, required) in [
			("1.16.5", 8),
			("1.17", 16),
			("1.18", 17),
			("1.20.4", 17),
			("1.20.5", 21),
			("1.21.1", 21),
		] {
			let row = mapping.iter().find(|v| v["minecraft"] == version).unwrap();
			assert_eq!(row["required"], required);
		}
		for row in mapping {
			let metadata = &row["javaVersion"];
			let expected = metadata["majorVersion"].as_u64().unwrap_or(8) as u32;
			assert_eq!(row["required"], expected);
			let runtime = JavaRuntime {
				path: "C:\\Program Files\\Eclipse Adoptium\\jdk\\bin\\java.exe".into(),
				major: expected,
				architecture: "amd64".into(),
			};
			assert!(
				validate_major(
					runtime.clone(),
					expected,
					row["minecraft"].as_str().unwrap()
				)
				.is_ok()
			);
			assert!(validate_major(runtime, expected + 1, "test").is_err());
		}
	}
	#[test]
	fn rejects_missing_architecture_wrong_architecture_and_bad_versions() {
		assert_eq!(major("1.8.0_202"), Some(8));
		assert_eq!(major("8u202"), Some(8));
		assert_eq!(major("21-ea"), Some(21));
		assert!(parse_probe(Path::new("java"), "java.version = 21.0.1").is_err());
		assert!(parse_probe(Path::new("java"), "java.version = invalid\nos.arch = amd64").is_err());
		#[cfg(target_arch = "x86_64")]
		assert!(parse_probe(Path::new("java"), "java.version = 21.0.1\nos.arch = x86").is_err());
	}
	#[test]
	fn runtime_paths_and_hosts_are_confined() {
		assert_eq!(
			link_target("legal/java.compiler/LICENSE", "../java.base/LICENSE").unwrap(),
			"legal/java.base/LICENSE"
		);
		for target in [
			"../../outside",
			"/etc/passwd",
			"C:\\Windows\\java.exe",
			"..\\outside",
		] {
			assert!(link_target("bin/java", target).is_err());
		}
		for name in [
			"../java",
			"/bin/java",
			"C:/java.exe",
			"bin/java:stream",
			"bin/java.",
		] {
			assert!(crate::files::safe_relative(name).is_err());
		}
		assert!(mojang_url("https://piston-data.mojang.com/v1/objects/hash/java").is_ok());
		for url in [
			"http://piston-data.mojang.com/java",
			"https://example.com/java",
			"https://piston-data.mojang.com.evil.test/java",
		] {
			assert!(mojang_url(url).is_err());
		}
	}
	#[cfg(unix)]
	#[tokio::test]
	async fn real_process_failure_spaces_and_bounded_probe() {
		use std::os::unix::fs::PermissionsExt;
		let dir = tempfile::tempdir().unwrap();
		let file = dir.path().join("Java With Spaces");
		for (body, success) in [
			(
				"#!/bin/sh\necho 'java.version = 21.0.1' >&2\necho 'os.arch = amd64' >&2\n",
				true,
			),
			("#!/bin/sh\nexit 1\n", false),
			("broken", false),
		] {
			std::fs::write(&file, body).unwrap();
			std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
			assert_eq!(check_java(&file).await.is_ok(), success);
		}
		std::fs::write(&file, "#!/bin/sh\nexec sleep 30\n").unwrap();
		let started = std::time::Instant::now();
		assert!(check_java(&file).await.is_err());
		assert!(started.elapsed() < Duration::from_secs(7));
	}
	async fn engine(root: PathBuf) -> std::sync::Arc<Engine> {
		let pool = sqlx::sqlite::SqlitePoolOptions::new()
			.max_connections(1)
			.connect("sqlite::memory:")
			.await
			.unwrap();
		Engine::open(root, pool).await.unwrap()
	}
	#[tokio::test]
	async fn rejects_unverified_and_unsafe_runtime_without_publishing() {
		use sha1::{Digest, Sha1};
		for (name, hash) in [
			("bin/java", "0".repeat(40)),
			("../escape", hex::encode(Sha1::digest(b"broken"))),
			("bin/java", hex::encode(Sha1::digest(b"broken"))),
		] {
			let root = tempfile::tempdir().unwrap();
			let e = engine(root.path().to_owned()).await;
			let manifest_url = "https://piston-meta.mojang.com/test-manifest";
			let binary_url = "https://piston-data.mojang.com/test-binary";
			let manifest = serde_json::to_vec(&serde_json::json!({"files":{name:{"type":"file","executable":true,"downloads":{"raw":{"url":binary_url,"sha1":hash,"size":6}}}}})).unwrap();
			let index = serde_json::json!({platform().unwrap():{"test":[{"version":{"name":"21.0.1"},"manifest":{"url":manifest_url,"sha1":hex::encode(Sha1::digest(&manifest)),"size":manifest.len()}}]}});
			e.downloads
				.fixture(RUNTIME_INDEX, serde_json::to_vec(&index).unwrap());
			e.downloads.fixture(manifest_url, manifest);
			e.downloads.fixture(binary_url, b"broken".to_vec());
			let op = e.begin("java_test", None);
			assert!(e.managed_java(21, &op).await.is_err());
			assert_eq!(
				std::fs::read_dir(root.path().join("java")).unwrap().count(),
				0
			);
			assert!(!root.path().join("escape").exists());
		}
	}
	#[tokio::test]
	async fn cancelled_preparation_stays_cancelled() {
		let root = tempfile::tempdir().unwrap();
		let e = engine(root.path().to_owned()).await;
		let op = e.begin("java_test", None);
		e.cancel(&op.snapshot().id).unwrap();
		assert!(matches!(
			op.wait(std::future::pending::<Result<JavaRuntime>>()).await,
			Err(Error::Cancelled)
		));
	}
	#[tokio::test]
	#[ignore = "requires CI setup-java or explicit JAVA_HOME"]
	async fn ci_installed_java() {
		let home = PathBuf::from(std::env::var_os("JAVA_HOME").expect("JAVA_HOME required"));
		let runtime = check_java(&home.join("bin").join(executable()))
			.await
			.unwrap();
		assert_eq!(runtime.major, 21);
		assert!(validate_major(runtime, 17, "1.18").is_err());
	}

	#[tokio::test]
	#[ignore = "real Mojang downloads and executable validation on this host"]
	async fn real_managed_generations() {
		let root = std::env::temp_dir().join("ncreate-stable-java-verification");
		let e = engine(root).await;
		for required in [8, 16, 17, 21] {
			let op = e.begin("java_runtime_test", None);
			let started = std::time::Instant::now();
			let runtime = tokio::time::timeout(PREPARE_LIMIT, e.managed_java(required, &op))
				.await
				.unwrap()
				.unwrap();
			assert_eq!(
				check_java(Path::new(&runtime.path)).await.unwrap().major,
				required
			);
			println!(
				"REAL JAVA {required} {} {:?} {}",
				runtime.architecture,
				started.elapsed(),
				runtime.path
			);
		}
	}
}

#[cfg(test)]
mod deadline_tests {
	use super::*;
	#[tokio::test(start_paused = true)]
	async fn two_minute_deadline_reports_recoverable_requirement() {
		let instance: Instance = serde_json::from_value(serde_json::json!({"id":"test","name":"Test","game_version":"1.21.1","loader":"vanilla","kind":"custom","status":"created","memory_mb":2048,"directory":"test"})).unwrap();
		let start = tokio::time::Instant::now();
		let error = finish_preparation(&instance, 21, std::future::pending())
			.await
			.unwrap_err();
		assert_eq!(start.elapsed(), Duration::from_secs(120));
		let Error::Java(payload) = error else {
			panic!("not recoverable")
		};
		let payload: serde_json::Value = serde_json::from_str(&payload).unwrap();
		assert_eq!(payload["required"], 21);
		assert_eq!(payload["minecraft"], "1.21.1");
	}
	#[test]
	fn full_metadata_and_loader_override_preserve_authoritative_java() {
		for (bytes, expected) in [
			(include_str!("../tests/fixtures/minecraft-1.16.5.json"), 8),
			(include_str!("../tests/fixtures/minecraft-1.17.json"), 16),
			(include_str!("../tests/fixtures/minecraft-1.18.json"), 17),
			(include_str!("../tests/fixtures/minecraft-1.20.5.json"), 21),
			(include_str!("../tests/fixtures/minecraft-1.21.1.json"), 21),
		] {
			let info: daedalus::minecraft::VersionInfo = serde_json::from_str(bytes).unwrap();
			assert_eq!(info.java_version.as_ref().unwrap().major_version, expected);
			let partial: daedalus::modded::PartialVersionInfo = serde_json::from_value(serde_json::json!({"id":"loader","inheritsFrom":info.id,"releaseTime":"2024-01-01T00:00:00Z","time":"2024-01-01T00:00:00Z","libraries":[],"type":"release","javaVersion":{"component":"custom","majorVersion":25}})).unwrap();
			assert_eq!(
				daedalus::modded::merge_partial_version(partial, info)
					.java_version
					.unwrap()
					.major_version,
				25
			);
		}
	}
}

#[cfg(test)]
mod live_pipeline_tests {
	use super::*;
	#[tokio::test]
	#[ignore = "downloads real Minecraft and official NCreate pack, opens game windows"]
	async fn real_minecraft_and_official_pack() {
		let root = std::env::temp_dir().join("ncreate-stable-java-verification");
		tokio::fs::create_dir_all(&root).await.unwrap();
		let options = sqlx::sqlite::SqliteConnectOptions::new()
			.filename(root.join("verification.sqlite"))
			.create_if_missing(true);
		let pool = sqlx::sqlite::SqlitePoolOptions::new()
			.max_connections(4)
			.connect_with(options)
			.await
			.unwrap();
		let e = Engine::open(root.clone(), pool).await.unwrap();
		for (version, required) in [
			("1.16.5", 8),
			("1.17", 16),
			("1.18", 17),
			("1.21.1", 21),
			("official", 21),
		] {
			if std::env::var("NCREATE_TEST_VERSION").is_ok_and(|selected| {
				selected != version
					&& !(selected == "remaining"
						&& ["1.18", "1.21.1", "official"].contains(&version))
			}) {
				continue;
			}
			let op = e.begin("runtime_verification", None);
			let existing = e.instances().await.unwrap().into_iter().find(|i| {
				if version == "official" {
					i.kind == "official"
				} else {
					i.name == format!("Verification {version}")
				}
			});
			let instance = if let Some(instance) = existing {
				instance
			} else if version == "official" {
				e.install_edition_files("ncreate-server", "stable", &op)
					.await
					.unwrap()
			} else {
				e.create_instance(crate::CreateInstance {
					name: format!("Verification {version}"),
					game_version: version.into(),
					loader: crate::Loader::Vanilla,
					loader_version: None,
					memory_mb: 2048,
					java_path: None,
				})
				.await
				.unwrap()
			};
			assert_eq!(e.required_java(&instance.id).await.unwrap(), required);
			let installed =
				tokio::time::timeout(Duration::from_secs(900), e.install_game(&instance.id, &op))
					.await
					.unwrap();
			op.finish(&installed);
			let installed = installed.unwrap();
			println!("REAL INSTALL {version} Java {required}: {}", installed.id);
			let identity = crate::LaunchIdentity {
				nickname: "NCreateTest".into(),
				uuid: "00000000-0000-3000-8000-000000000001".into(),
				access_token: "0".into(),
				user_type: "legacy".into(),
				xuid: None,
				authlib_injector: None,
			};
			let game = e.launch(&installed.id, identity).await.unwrap();
			println!("REAL LAUNCH {version}: pid {}", game.pid);
			tokio::time::sleep(Duration::from_secs(if version == "official" {
				60
			} else {
				15
			}))
			.await;
			assert_eq!(
				e.instance(&installed.id).await.unwrap().status,
				"running",
				"{version} exited; inspect instance logs under {}",
				root.display()
			);
			e.stop(&installed.id).await.unwrap();
			println!("REAL STOP {version}: PASS");
		}
	}
}
