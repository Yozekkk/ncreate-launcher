//! Minecraft metadata, loader merging and argument substitutions adapted from Modrinth's launcher.
use crate::{
	Engine, Error, GameVersion, Instance, JavaRuntime, LaunchIdentity, Loader, LoaderVersion,
	Operation, Result, RunningGame,
};
use daedalus::minecraft::{
	Argument, ArgumentType, ArgumentValue, DownloadType, Os, Rule, RuleAction, VersionInfo,
	VersionManifest,
};
use futures_util::{StreamExt, TryStreamExt, stream};
use std::{
	collections::HashMap,
	path::{Path, PathBuf},
	process::Stdio,
};
impl Engine {
	pub async fn game_versions(&self) -> Result<Vec<GameVersion>> {
		let manifest: VersionManifest = self
			.read_json(daedalus::minecraft::VERSION_MANIFEST_URL, "metadata")
			.await?;
		Ok(manifest
			.versions
			.into_iter()
			.map(|v| GameVersion {
				id: v.id,
				kind: v.type_.as_str().into(),
				release_time: v.release_time.to_rfc3339(),
			})
			.collect())
	}
	async fn loader_manifest(
		&self,
		loader: Loader,
		op: &Operation,
	) -> Result<daedalus::modded::Manifest> {
		let key = if loader == Loader::Neoforge {
			"neo"
		} else {
			loader.key()
		};
		let metadata = daedalus::modded::loader_manifest_metadata(key);
		self.downloads
			.json(
				&format!("https://launcher-meta.modrinth.com/{}", metadata.path),
				op,
			)
			.await
	}
	pub async fn loader_versions(
		&self,
		loader: Loader,
		game_version: &str,
	) -> Result<Vec<LoaderVersion>> {
		if loader == Loader::Vanilla {
			return Ok(Vec::new());
		}
		let op = self.begin("loader_versions", None);
		let result = async {
			let manifest = self.loader_manifest(loader, &op).await?;
			let versions = loader_matches(&manifest, game_version)?;
			Ok(versions
				.into_iter()
				.map(|v| LoaderVersion {
					id: v.id,
					stable: v.stable,
				})
				.collect())
		}
		.await;
		op.finish(&result);
		result
	}
	async fn version_info(&self, instance: &Instance, op: &Operation) -> Result<VersionInfo> {
		let manifest: VersionManifest = self
			.downloads
			.json(daedalus::minecraft::VERSION_MANIFEST_URL, op)
			.await?;
		let version = manifest
			.versions
			.iter()
			.find(|v| v.id == instance.game_version)
			.ok_or_else(|| {
				Error::Invalid("Minecraft version not found in official metadata".into())
			})?;
		let bytes = self
			.downloads
			.bytes(&version.url, 32 * 1024 * 1024, op)
			.await?;
		crate::download::verify(&bytes, &version.sha1, "sha1")?;
		let mut info: VersionInfo = serde_json::from_slice(&bytes)?;
		if instance.loader != Loader::Vanilla {
			let partial: daedalus::modded::PartialVersionInfo =
				if instance.loader == Loader::Neoforge {
					self.neoforge_installer_info(instance, op).await?
				} else {
					let manifest = self.loader_manifest(instance.loader, op).await?;
					let versions = loader_matches(&manifest, &instance.game_version)?;
					let selected = versions
						.into_iter()
						.find(|v| Some(v.id.as_str()) == instance.loader_version.as_deref())
						.ok_or_else(|| {
							Error::Invalid(
							"loader version is incompatible with the selected Minecraft version".into(),
						)
						})?;
					self.downloads.json(&selected.url, op).await?
				};
			info = daedalus::modded::merge_partial_version(partial, info);
		}
		Ok(info)
	}
	async fn neoforge_installer_info(
		&self,
		instance: &Instance,
		op: &Operation,
	) -> Result<daedalus::modded::PartialVersionInfo> {
		let version = instance
			.loader_version
			.as_deref()
			.ok_or_else(|| Error::Invalid("NeoForge version is missing".into()))?;
		if version.is_empty()
			|| version.len() > 64
			|| !version
				.bytes()
				.all(|byte| byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'-')
		{
			return Err(Error::Invalid("invalid NeoForge version".into()));
		}
		let base = format!(
			"https://maven.neoforged.net/releases/net/neoforged/neoforge/{version}/neoforge-{version}-installer.jar"
		);
		let expected = String::from_utf8(
			self.downloads
				.bytes(&format!("{base}.sha256"), 1024, op)
				.await?,
		)
		.map_err(|_| Error::Invalid("NeoForge checksum is invalid".into()))?;
		let expected = expected.split_whitespace().next().unwrap_or("");
		if expected.len() != 64 || !expected.bytes().all(|byte| byte.is_ascii_hexdigit()) {
			return Err(Error::Invalid("NeoForge checksum is invalid".into()));
		}
		let path = self
			.root
			.join("minecraft/installers")
			.join(format!("neoforge-{version}-installer.jar"));
		if crate::download::verify_file(&path, expected, "sha256", 0)
			.await
			.is_err()
		{
			let bytes = self.downloads.bytes(&base, 64 * 1024 * 1024, op).await?;
			crate::download::verify(&bytes, expected, "sha256")?;
			tokio::fs::create_dir_all(
				path.parent()
					.ok_or_else(|| Error::Invalid("installer path has no parent".into()))?,
			)
			.await?;
			let temporary = path.with_extension("jar.next");
			tokio::fs::write(&temporary, bytes).await?;
			tokio::fs::rename(temporary, &path).await?;
		}
		let (mut partial, mut data, bytes) = {
			let mut archive = zip::ZipArchive::new(std::fs::File::open(&path)?)?;
			let (version_json, profile) = {
				let mut read_json = |name: &str| -> Result<serde_json::Value> {
					use std::io::Read;
					let entry = archive.by_name(name)?;
					if entry.size() > 16 * 1024 * 1024 {
						return Err(Error::Invalid(
							"NeoForge installer metadata exceeds limit".into(),
						));
					}
					let mut bytes = Vec::with_capacity(entry.size() as usize);
					entry.take(16 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
					Ok(serde_json::from_slice(&bytes)?)
				};
				(
					read_json("version.json")?,
					read_json("install_profile.json")?,
				)
			};
			if version_json["inheritsFrom"] != instance.game_version
				|| profile["minecraft"] != instance.game_version
				|| profile["version"] != format!("neoforge-{version}")
			{
				return Err(Error::Invalid(
					"NeoForge installer targets a different Minecraft version".into(),
				));
			}
			let mut partial: daedalus::modded::PartialVersionInfo =
				serde_json::from_value(version_json)?;
			let mut profile_libraries: Vec<daedalus::minecraft::Library> =
				serde_json::from_value(profile["libraries"].clone())?;
			for mut library in profile_libraries.drain(..) {
				if !partial
					.libraries
					.iter()
					.any(|existing| existing.name == library.name)
				{
					library.include_in_classpath = false;
					partial.libraries.push(library);
				}
			}
			partial.processors = Some(serde_json::from_value(profile["processors"].clone())?);
			let data: std::collections::HashMap<String, daedalus::modded::SidedDataEntry> =
				serde_json::from_value(profile["data"].clone())?;
			let patch = archive.by_name("data/client.lzma")?;
			if patch.size() > 32 * 1024 * 1024 {
				return Err(Error::Invalid("NeoForge client patch exceeds limit".into()));
			}
			use std::io::Read;
			let mut bytes = Vec::with_capacity(patch.size() as usize);
			patch.take(32 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
			(partial, data, bytes)
		};
		let patch_path = self
			.root
			.join("minecraft/installers")
			.join(format!("neoforge-{version}-client.lzma"));
		tokio::fs::write(&patch_path, bytes).await?;
		let client = patch_path.to_string_lossy().into_owned();
		data.insert(
			"BINPATCH".into(),
			daedalus::modded::SidedDataEntry {
				client: client.clone(),
				server: client,
			},
		);
		partial.data = Some(data);
		Ok(partial)
	}
	pub async fn java_runtimes(&self) -> Result<Vec<JavaRuntime>> {
		let mut candidates = vec![PathBuf::from(if cfg!(windows) {
			"java.exe"
		} else {
			"java"
		})];
		if let Some(home) = std::env::var_os("JAVA_HOME") {
			candidates.push(PathBuf::from(home).join("bin").join(if cfg!(windows) {
				"java.exe"
			} else {
				"java"
			}));
		}
		let roots = if cfg!(windows) {
			vec![
				PathBuf::from("C:\\Program Files\\Java"),
				PathBuf::from("C:\\Program Files\\Eclipse Adoptium"),
			]
		} else {
			vec![PathBuf::from("/usr/lib/jvm"), self.root.join("java")]
		};
		for root in roots {
			if let Ok(mut entries) = tokio::fs::read_dir(root).await {
				while let Some(entry) = entries.next_entry().await? {
					candidates.push(entry.path().join("bin").join(if cfg!(windows) {
						"java.exe"
					} else {
						"java"
					}));
				}
			}
		}
		let mut runtimes = Vec::new();
		for path in candidates {
			if let Ok(runtime) = check_java(&path).await
				&& !runtimes
					.iter()
					.any(|r: &JavaRuntime| r.path == runtime.path)
			{
				runtimes.push(runtime);
			}
		}
		Ok(runtimes)
	}
	async fn choose_java(&self, instance: &Instance, required: u32) -> Result<JavaRuntime> {
		if let Some(path) = &instance.java_path {
			let runtime = check_java(Path::new(path)).await?;
			if runtime.major != required {
				return Err(Error::Invalid(format!(
					"this version needs Java {required}; selected runtime is Java {}",
					runtime.major
				)));
			}
			return Ok(runtime);
		}
		self.java_runtimes()
			.await?
			.into_iter()
			.find(|r| r.major == required)
			.ok_or_else(|| {
				Error::Invalid(format!(
					"install Java {required} or choose its executable in the instance settings"
				))
			})
	}
	pub async fn install_game(&self, instance_id: &str, op: &Operation) -> Result<Instance> {
		let _guard = self.mutation.lock().await;
		self.ensure_stopped(instance_id).await?;
		let mut instance = self.instance(instance_id).await?;
		let info = self.version_info(&instance, op).await?;
		let stored: Option<String> =
			sqlx::query_scalar("SELECT data FROM launcher_manifests WHERE instance_id=?")
				.bind(instance_id)
				.fetch_optional(&self.pool)
				.await?;
		let edition = stored
			.map(|s| serde_json::from_str::<crate::EditionManifest>(&s))
			.transpose()?;
		let required = edition.as_ref().map_or_else(
			|| info.java_version.as_ref().map_or(8, |j| j.major_version),
			|m| m.java.major,
		);
		let java = self.choose_java(&instance, required).await?;
		let runtime = self.root.join("minecraft");
		let libraries = runtime.join("libraries");
		let assets = runtime.join("assets");
		let natives = self
			.instance_path(instance_id)?
			.join(".ncreate-runtime/natives");
		tokio::fs::create_dir_all(&natives).await?;
		tokio::fs::create_dir_all(runtime.join("versions")).await?;
		let client = info
			.downloads
			.get(&DownloadType::Client)
			.ok_or_else(|| Error::Invalid("Minecraft metadata has no client artifact".into()))?;
		let client_path = runtime
			.join("versions")
			.join(format!("{}.jar", instance.game_version));
		self.downloads
			.file(
				&client.url,
				&client_path,
				&client.sha1,
				"sha1",
				client.size as u64,
				op,
			)
			.await?;
		let index_path = assets
			.join("indexes")
			.join(crate::files::safe_relative(&format!(
				"{}.json",
				info.asset_index.id
			))?);
		self.downloads
			.file(
				&info.asset_index.url,
				&index_path,
				&info.asset_index.sha1,
				"sha1",
				info.asset_index.size as u64,
				op,
			)
			.await?;
		let index: daedalus::minecraft::AssetsIndex =
			serde_json::from_slice(&tokio::fs::read(&index_path).await?)?;
		stream::iter(index.objects.into_values())
			.map(|asset| {
				let assets = assets.clone();
				async move {
					op.check()?;
					if asset.hash.len() != 40 || !asset.hash.chars().all(|c| c.is_ascii_hexdigit())
					{
						return Err(Error::Invalid("invalid asset hash".into()));
					}
					let prefix = &asset.hash[..2];
					self.downloads
						.file(
							&format!(
								"https://resources.download.minecraft.net/{prefix}/{}",
								asset.hash
							),
							&assets.join("objects").join(prefix).join(&asset.hash),
							&asset.hash,
							"sha1",
							asset.size as u64,
							op,
						)
						.await
				}
			})
			.buffer_unordered(6)
			.try_collect::<Vec<_>>()
			.await?;
		for library in &info.libraries {
			op.check()?;
			// Forge metadata includes server-only installer patches in the merged
			// library list. The desktop client never executes server processors.
			if instance.loader == Loader::Forge
				&& library
					.name
					.starts_with("com.modrinth.daedalus:forge-installer-extracts:")
				&& library.name.ends_with(":server@lzma")
			{
				continue;
			}
			if library
				.rules
				.as_deref()
				.is_some_and(|r| !rules_allow(r, &java.architecture))
				|| !library.downloadable
			{
				continue;
			}
			if let Some(artifact) = library.downloads.as_ref().and_then(|d| d.artifact.as_ref()) {
				if !artifact.url.is_empty() {
					let relative = artifact.path.clone().unwrap_or(
						daedalus::get_path_from_artifact(&library.name)
							.map_err(|e| Error::Invalid(e.to_string()))?,
					);
					self.downloads
						.file(
							&artifact.url,
							&libraries.join(crate::files::safe_relative(&relative)?),
							&artifact.sha1,
							"sha1",
							artifact.size as u64,
							op,
						)
						.await?;
				}
			} else {
				let relative = daedalus::get_path_from_artifact(&library.name)
					.map_err(|e| Error::Invalid(e.to_string()))?;
				let base = library_base(
					library
						.url
						.as_deref()
						.unwrap_or("https://libraries.minecraft.net/"),
					&library.name,
					instance.loader,
				);
				let url = format!(
					"{}{relative}",
					if base.ends_with('/') {
						base.to_string()
					} else {
						format!("{base}/")
					}
				);
				// The upstream Forge installer extract has no checksum sidecar.
				// Accept only this exact metadata-owned client patch, with a strict
				// byte limit. The Forge processor verifies its patched output SHA-1.
				if instance.loader == Loader::Forge
					&& library
						.name
						.starts_with("com.modrinth.daedalus:forge-installer-extracts:")
					&& library.name.ends_with(":client@lzma")
					&& base == "https://launcher-meta.modrinth.com/maven/"
				{
					let target = libraries.join(crate::files::safe_relative(&relative)?);
					let bytes = self.downloads.bytes(&url, 16 * 1024 * 1024, op).await?;
					if let Some(parent) = target.parent() {
						tokio::fs::create_dir_all(parent).await?;
					}
					let temporary =
						target.with_extension(format!("lzma.{}.part", uuid::Uuid::new_v4()));
					tokio::fs::write(&temporary, bytes).await?;
					if target.exists() {
						tokio::fs::remove_file(&target).await?;
					}
					tokio::fs::rename(temporary, target).await?;
					continue;
				}
				let hash = if let Some(hash) = library.checksums.as_ref().and_then(|x| x.first()) {
					hash.clone()
				} else {
					String::from_utf8(
						self.downloads
							.bytes(&format!("{url}.sha1"), 4096, op)
							.await?,
					)
					.map_err(|_| Error::Invalid("library checksum is invalid".into()))?
					.split_whitespace()
					.next()
					.ok_or_else(|| Error::Invalid("library checksum is empty".into()))?
					.to_string()
				};
				self.downloads
					.file(
						&url,
						&libraries.join(crate::files::safe_relative(&relative)?),
						&hash,
						"sha1",
						0,
						op,
					)
					.await?;
			}
			if let Some((classifier, classifiers)) =
				library.natives_os_key_and_classifiers(&java.architecture)
			{
				let classifier = classifier.replace(
					"${arch}",
					if cfg!(target_pointer_width = "64") {
						"64"
					} else {
						"32"
					},
				);
				if let Some(native) = classifiers.get(&classifier) {
					let path = runtime
						.join("native-cache")
						.join(format!("{}.jar", native.sha1));
					self.downloads
						.file(
							&native.url,
							&path,
							&native.sha1,
							"sha1",
							native.size as u64,
							op,
						)
						.await?;
					extract_natives(
						&path,
						&natives,
						library.extract.as_ref().and_then(|e| e.exclude.as_deref()),
					)?;
				}
			}
		}
		if let Some(logging) = &info.logging {
			for config in logging.values() {
				match config {
					daedalus::minecraft::LoggingConfiguration::Log4j2Xml { file, .. } => {
						self.downloads
							.file(
								&file.url,
								&runtime
									.join("log-configs")
									.join(crate::files::safe_relative(&file.id)?),
								&file.sha1,
								"sha1",
								file.size as u64,
								op,
							)
							.await?;
					}
				}
			}
		}
		self.run_processors(&info, &java, &libraries, &client_path, &instance, op)
			.await?;
		op.check()?;
		let path = self
			.instance_path(instance_id)?
			.join(".ncreate-runtime/version.json");
		let temporary = path.with_extension("next");
		tokio::fs::write(&temporary, serde_json::to_vec(&info)?).await?;
		tokio::fs::rename(temporary, path).await?;
		instance.status = "ready".into();
		instance.java_path = Some(java.path);
		self.save_instance(&instance).await?;
		Ok(instance)
	}
	async fn run_processors(
		&self,
		info: &VersionInfo,
		java: &JavaRuntime,
		libraries: &Path,
		client: &Path,
		instance: &Instance,
		op: &Operation,
	) -> Result<()> {
		let Some(processors) = &info.processors else {
			return Ok(());
		};
		let mut data: HashMap<String, String> = info
			.data
			.as_ref()
			.map(|d| {
				d.iter()
					.map(|(k, v)| (k.clone(), v.client.clone()))
					.collect()
			})
			.unwrap_or_default();
		data.extend([
			("SIDE".into(), "client".into()),
			("MINECRAFT_JAR".into(), client.to_string_lossy().into()),
			("MINECRAFT_VERSION".into(), instance.game_version.clone()),
			("ROOT".into(), instance.directory.clone()),
			("LIBRARY_DIR".into(), libraries.to_string_lossy().into()),
		]);
		for (index, processor) in processors.iter().enumerate() {
			op.check()?;
			if processor
				.sides
				.as_ref()
				.is_some_and(|s| !s.iter().any(|v| v == "client"))
			{
				continue;
			}
			let jar = libraries.join(
				daedalus::get_path_from_artifact(&processor.jar)
					.map_err(|e| Error::Invalid(e.to_string()))?,
			);
			let mut archive = zip::ZipArchive::new(std::fs::File::open(&jar)?)?;
			let mut manifest = String::new();
			use std::io::Read;
			archive
				.by_name("META-INF/MANIFEST.MF")?
				.take(65536)
				.read_to_string(&mut manifest)?;
			let main = manifest
				.lines()
				.find_map(|l| l.strip_prefix("Main-Class:").map(str::trim))
				.ok_or_else(|| Error::Invalid("loader processor has no main class".into()))?;
			let cp = processor
				.classpath
				.iter()
				.chain(std::iter::once(&processor.jar))
				.map(|a| {
					daedalus::get_path_from_artifact(a)
						.map(|p| libraries.join(p).to_string_lossy().into_owned())
						.map_err(|e| Error::Invalid(e.to_string()))
				})
				.collect::<Result<Vec<_>>>()?
				.join(separator());
			let args = processor
				.args
				.iter()
				.map(|a| processor_argument(a, &data, libraries))
				.collect::<Result<Vec<_>>>()?;
			op.progress(
				"loader_processors",
				index as u64,
				processors.len() as u64,
				"Running loader installation processors",
			);
			let mut child = tokio::process::Command::new(&java.path)
				.arg("-cp")
				.arg(cp)
				.arg(main)
				.args(args)
				.current_dir(&instance.directory)
				.stdout(Stdio::null())
				.stderr(Stdio::null())
				.kill_on_drop(true)
				.spawn()?;
			loop {
				op.check()?;
				if let Some(status) = child.try_wait()? {
					if !status.success() {
						return Err(Error::Invalid(
							"loader processor failed; installation was not marked ready".into(),
						));
					}
					break;
				}
				tokio::time::sleep(std::time::Duration::from_millis(100)).await;
			}
			if let Some(outputs) = &processor.outputs {
				for (path, hash) in outputs {
					let path = processor_argument(path, &data, libraries)?;
					let hash = processor_argument(hash, &data, libraries)?;
					let hash = processor_output_hash(&hash)?;
					crate::download::verify_file(Path::new(&path), hash, "sha1", 0)
						.await
						.map_err(|error| {
							Error::Invalid(format!(
								"loader processor {index} output verification failed: {error}"
							))
						})?;
				}
			}
		}
		Ok(())
	}
	pub async fn launch(&self, instance_id: &str, identity: LaunchIdentity) -> Result<RunningGame> {
		let _guard = self.mutation.lock().await;
		self.ensure_stopped(instance_id).await?;
		let mut instance = self.instance(instance_id).await?;
		if instance.status != "ready" {
			return Err(Error::Invalid(
				"install Minecraft before launching this instance".into(),
			));
		}
		let info: VersionInfo = serde_json::from_slice(
			&tokio::fs::read(
				self.instance_path(instance_id)?
					.join(".ncreate-runtime/version.json"),
			)
			.await?,
		)?;
		let stored: Option<String> =
			sqlx::query_scalar("SELECT data FROM launcher_manifests WHERE instance_id=?")
				.bind(instance_id)
				.fetch_optional(&self.pool)
				.await?;
		let edition = stored
			.map(|s| serde_json::from_str::<crate::EditionManifest>(&s))
			.transpose()?;
		let java_major = edition.as_ref().map_or_else(
			|| info.java_version.as_ref().map_or(8, |v| v.major_version),
			|m| m.java.major,
		);
		if let Some(m) = &edition
			&& (instance.memory_mb < m.memory.minimum_mb
				|| instance.memory_mb > m.memory.maximum_mb)
		{
			return Err(Error::Invalid(
				"RAM is outside this edition manifest bounds".into(),
			));
		}

		let java = self.choose_java(&instance, java_major).await?;
		let runtime = self.root.join("minecraft");
		let libraries = runtime.join("libraries");
		let natives = self
			.instance_path(instance_id)?
			.join(".ncreate-runtime/natives");
		let mut classpath = Vec::new();
		for lib in &info.libraries {
			if !lib.include_in_classpath
				|| lib
					.rules
					.as_deref()
					.is_some_and(|r| !rules_allow(r, &java.architecture))
			{
				continue;
			}
			let relative = lib
				.downloads
				.as_ref()
				.and_then(|d| d.artifact.as_ref())
				.and_then(|d| d.path.clone())
				.unwrap_or(
					daedalus::get_path_from_artifact(&lib.name)
						.map_err(|e| Error::Invalid(e.to_string()))?,
				);
			let path = libraries.join(crate::files::safe_relative(&relative)?);
			if path.exists() {
				classpath.push(path.to_string_lossy().into_owned());
			} else if lib.natives.is_none() {
				return Err(Error::Invalid(format!(
					"missing runtime library {}",
					lib.name
				)));
			}
		}
		classpath.push(
			runtime
				.join("versions")
				.join(format!("{}.jar", instance.game_version))
				.to_string_lossy()
				.into_owned(),
		);
		let substitutions = SecretMap(HashMap::from([
			("natives_directory", natives.to_string_lossy().into_owned()),
			(
				"library_directory",
				libraries.to_string_lossy().into_owned(),
			),
			("classpath", classpath.join(separator())),
			("classpath_separator", separator().into()),
			("launcher_name", "NCreate Launcher".into()),
			("launcher_version", env!("CARGO_PKG_VERSION").into()),
			("version_name", info.id.clone()),
			("auth_player_name", identity.nickname.clone()),
			("auth_uuid", identity.uuid.replace('-', "")),
			("uuid", identity.uuid.replace('-', "")),
			("auth_access_token", identity.access_token.clone()),
			("auth_session", identity.access_token.clone()),
			("accessToken", identity.access_token.clone()),
			(
				"auth_xuid",
				identity.xuid.clone().unwrap_or_else(|| "0".into()),
			),
			("clientid", "0".into()),
			("user_properties", "{}".into()),
			("user_type", identity.user_type.clone()),
			("game_directory", instance.directory.clone()),
			(
				"assets_root",
				runtime.join("assets").to_string_lossy().into_owned(),
			),
			(
				"game_assets",
				runtime.join("assets").to_string_lossy().into_owned(),
			),
			("assets_index_name", info.asset_index.id.clone()),
			("version_type", info.type_.as_str().into()),
		]));
		let mut jvm = if let Some(args) = info
			.arguments
			.as_ref()
			.and_then(|a| a.get(&ArgumentType::Jvm))
		{
			expand_arguments(args, &substitutions, &java.architecture)?
		} else {
			vec![
				format!("-Djava.library.path={}", natives.display()),
				"-cp".into(),
				classpath.join(separator()),
			]
		};
		if instance.loader == crate::Loader::Neoforge {
			// The NeoForge client processor supplies the transformed Minecraft module.
			// Keep the inherited vanilla client JAR on the classpath for launcher
			// compatibility, but prevent ModLauncher from discovering it as a second
			// Minecraft module.
			ignore_vanilla_client_module(&mut jvm, &instance.game_version);
		}
		jvm.push(format!("-Xmx{}M", instance.memory_mb));
		if let Some(agent) = identity.authlib_injector.as_ref() {
			if !Path::new(&agent).is_file() {
				return Err(Error::Invalid("Ely.by authlib-injector is missing".into()));
			}
			jvm.push(format!("-javaagent:{agent}=ely.by"));
		}
		let mut game = if let Some(args) = info
			.arguments
			.as_ref()
			.and_then(|a| a.get(&ArgumentType::Game))
		{
			expand_arguments(args, &substitutions, &java.architecture)?
		} else {
			info.minecraft_arguments
				.as_deref()
				.unwrap_or_default()
				.split_whitespace()
				.map(|x| substitute(x, &substitutions))
				.collect::<Result<Vec<_>>>()?
		};
		if let Some(m) = &edition {
			for argument in &m.launch.jvm_args {
				jvm.push(substitute(argument, &substitutions)?);
			}
			for argument in &m.launch.game_args {
				game.push(substitute(argument, &substitutions)?);
			}
		}
		if let Some(logging) = info
			.logging
			.as_ref()
			.and_then(|v| v.get(&daedalus::minecraft::LoggingSide::Client))
		{
			let daedalus::minecraft::LoggingConfiguration::Log4j2Xml { file, argument, .. } =
				logging;
			let log_config = runtime.join("log-configs").join(&file.id);
			jvm.push(argument.replace("${path}", &log_config.to_string_lossy()));
		}
		let logs = self.instance_path(instance_id)?.join("logs");
		tokio::fs::create_dir_all(&logs).await?;
		let log_path = logs.join(format!(
			"launcher-{}.log",
			chrono::Utc::now().timestamp_millis()
		));
		let jvm = zeroize::Zeroizing::new(jvm);
		let game = zeroize::Zeroizing::new(game);
		let mut child = tokio::process::Command::new(&java.path)
			.args(jvm.iter())
			.arg(&info.main_class)
			.args(game.iter())
			.current_dir(&instance.directory)
			.stdout(Stdio::piped())
			.stderr(Stdio::piped())
			.spawn()?;
		let pid = child
			.id()
			.ok_or_else(|| Error::Invalid("game process did not start".into()))?;
		if let Some(stdout) = child.stdout.take() {
			capture_log(stdout, log_path.clone(), identity.access_token.clone());
		}
		if let Some(stderr) = child.stderr.take() {
			capture_log(stderr, log_path.clone(), identity.access_token.clone());
		}
		self.running.lock().await.insert(instance_id.into(), child);
		instance.last_played = Some(chrono::Utc::now().timestamp());
		self.save_instance(&instance).await?;
		Ok(RunningGame {
			instance_id: instance_id.into(),
			pid,
			started_at: chrono::Utc::now().timestamp(),
			log_path: log_path.to_string_lossy().into(),
		})
	}
	pub async fn prepare_ely_runtime(&self, op: &Operation) -> Result<String> {
		let release: serde_json::Value = self
			.downloads
			.json(
				"https://api.github.com/repos/yushijinhun/authlib-injector/releases/latest",
				op,
			)
			.await?;
		let assets = release["assets"]
			.as_array()
			.ok_or_else(|| Error::Invalid("authlib-injector release has no assets".into()))?;
		let asset = assets
			.iter()
			.find(|a| {
				a["name"]
					.as_str()
					.is_some_and(|n| n.starts_with("authlib-injector-") && n.ends_with(".jar"))
			})
			.ok_or_else(|| Error::Invalid("official authlib-injector JAR not found".into()))?;
		let name = asset["name"]
			.as_str()
			.ok_or_else(|| Error::Invalid("authlib-injector filename missing".into()))?;
		if !path_util::is_safe_file_name(name) {
			return Err(Error::Invalid("unsafe authlib-injector filename".into()));
		}
		let hash=asset["digest"].as_str().and_then(|s|s.strip_prefix("sha256:")).ok_or_else(||Error::Invalid("official authlib-injector release has no SHA256 digest; configure a verified release before Ely.by launch".into()))?;
		if hash.len() != 64 {
			return Err(Error::Invalid("invalid authlib-injector digest".into()));
		}
		let url = asset["browser_download_url"]
			.as_str()
			.filter(|u| {
				u.starts_with("https://github.com/yushijinhun/authlib-injector/releases/download/")
			})
			.ok_or_else(|| Error::Invalid("unexpected authlib-injector download origin".into()))?;
		let size = asset["size"]
			.as_u64()
			.filter(|s| *s < 16 * 1024 * 1024)
			.ok_or_else(|| Error::Invalid("authlib-injector exceeds safe size".into()))?;
		let path = self.root.join("java-agents").join(name);
		self.downloads
			.file(url, &path, hash, "sha256", size, op)
			.await?;
		let mut jar = zip::ZipArchive::new(std::fs::File::open(&path)?)?;
		let mut manifest = String::new();
		use std::io::Read;
		jar.by_name("META-INF/MANIFEST.MF")?
			.take(65536)
			.read_to_string(&mut manifest)?;
		if !manifest.contains("Premain-Class:") {
			return Err(Error::Invalid(
				"authlib-injector JAR has no Java agent entrypoint".into(),
			));
		}
		Ok(path.to_string_lossy().into())
	}
	pub async fn stop(&self, instance_id: &str) -> Result<()> {
		if let Some(mut child) = self.running.lock().await.remove(instance_id) {
			child.kill().await?;
			child.wait().await?;
			Ok(())
		} else {
			Err(Error::Invalid("this instance is not running".into()))
		}
	}
}
fn library_base<'a>(base: &'a str, coordinate: &str, loader: Loader) -> &'a str {
	if base != "https://launcher-meta.modrinth.com/maven/" {
		return base;
	}
	let group = coordinate.split(':').next().unwrap_or("");
	match group {
		"net.fabricmc" => "https://maven.fabricmc.net/",
		"org.ow2.asm" => "https://repo.maven.apache.org/maven2/",
		"org.quiltmc" => "https://maven.quiltmc.org/repository/release/",
		"net.minecraftforge" => "https://maven.minecraftforge.net/",
		g if g.starts_with("net.neoforged") => "https://maven.neoforged.net/releases/",
		"cpw.mods" if loader == Loader::Neoforge => "https://maven.neoforged.net/releases/",
		"cpw.mods" => "https://maven.minecraftforge.net/",
		_ => base,
	}
}

fn loader_matches(
	manifest: &daedalus::modded::Manifest,
	game: &str,
) -> Result<Vec<daedalus::modded::LoaderVersion>> {
	let version = manifest
		.game_versions
		.iter()
		.find(|v| v.id.replace(daedalus::modded::DUMMY_REPLACE_STRING, game) == game)
		.ok_or_else(|| Error::Invalid("loader does not support this Minecraft version".into()))?;
	if let Some(group) = &version.version_group {
		manifest
			.version_groups
			.iter()
			.find(|g| &g.id == group)
			.map(|g| g.loaders.clone())
			.ok_or_else(|| Error::Invalid("loader metadata has a missing version group".into()))
	} else {
		Ok(version.loaders.clone())
	}
}
fn separator() -> &'static str {
	if cfg!(windows) { ";" } else { ":" }
}
async fn check_java(path: &Path) -> Result<JavaRuntime> {
	let output = tokio::time::timeout(
		std::time::Duration::from_secs(10),
		tokio::process::Command::new(path)
			.arg("-XshowSettings:properties")
			.arg("-version")
			.kill_on_drop(true)
			.output(),
	)
	.await
	.map_err(|_| Error::Invalid("Java version probe timed out".into()))??;
	if !output.status.success() {
		return Err(Error::Invalid("Java version probe failed".into()));
	}
	let text = String::from_utf8_lossy(&output.stderr);
	let version = text
		.lines()
		.find_map(|l| l.trim().strip_prefix("java.version = "))
		.or_else(|| text.split('"').nth(1))
		.ok_or_else(|| Error::Invalid("Java version could not be parsed".into()))?;
	let parts: Vec<&str> = version.split('.').collect();
	let major = if parts.first() == Some(&"1") {
		parts.get(1)
	} else {
		parts.first()
	}
	.and_then(|s| s.parse().ok())
	.ok_or_else(|| Error::Invalid("Java version could not be parsed".into()))?;
	let architecture = text
		.lines()
		.find_map(|l| l.trim().strip_prefix("os.arch = "))
		.unwrap_or("amd64")
		.into();
	Ok(JavaRuntime {
		path: path.to_string_lossy().into(),
		major,
		architecture,
	})
}
fn rules_allow(rules: &[Rule], arch: &str) -> bool {
	let mut allow = false;
	for rule in rules {
		let os_match = rule.os.as_ref().is_none_or(|os| {
			os.name
				.as_ref()
				.is_none_or(|name| name.get_os() == Os::native())
				&& os
					.arch
					.as_ref()
					.is_none_or(|a| a == arch || (a == "x86_64" && arch == "amd64"))
		});
		let feature_match = rule.features.as_ref().is_none_or(|f| {
			!f.is_demo_user.unwrap_or(false)
				&& !f.has_custom_resolution.unwrap_or(false)
				&& !f.has_quick_plays_support.unwrap_or(false)
				&& !f.is_quick_play_singleplayer.unwrap_or(false)
				&& !f.is_quick_play_multiplayer.unwrap_or(false)
				&& !f.is_quick_play_realms.unwrap_or(false)
		});
		if os_match && feature_match {
			allow = matches!(rule.action, RuleAction::Allow);
		}
	}
	allow
}
fn substitute(arg: &str, values: &HashMap<&str, String>) -> Result<String> {
	let mut out = arg.to_string();
	for (key, value) in values {
		out = out.replace(&format!("${{{key}}}"), value);
	}
	if out.contains("${") {
		return Err(Error::Invalid(
			"Minecraft argument contains unsupported feature placeholders".into(),
		));
	}
	Ok(out)
}
fn expand_arguments(
	args: &[Argument],
	values: &HashMap<&str, String>,
	arch: &str,
) -> Result<Vec<String>> {
	let mut out = Vec::new();
	for arg in args {
		match arg {
			Argument::Normal(v) => out.push(substitute(v, values)?),
			Argument::Ruled { rules, value } if rules_allow(rules, arch) => match value {
				ArgumentValue::Single(v) => out.push(substitute(v, values)?),
				ArgumentValue::Many(vs) => {
					for v in vs {
						out.push(substitute(v, values)?);
					}
				}
			},
			_ => {}
		}
	}
	Ok(out)
}
fn processor_argument(
	arg: &str,
	data: &HashMap<String, String>,
	libraries: &Path,
) -> Result<String> {
	if let Some(artifact) = arg.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
		return Ok(libraries
			.join(
				daedalus::get_path_from_artifact(artifact)
					.map_err(|e| Error::Invalid(e.to_string()))?,
			)
			.to_string_lossy()
			.into());
	}
	let mut out = arg.to_string();
	for (k, v) in data {
		let value = if let Some(artifact) = v.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
			libraries
				.join(
					daedalus::get_path_from_artifact(artifact)
						.map_err(|e| Error::Invalid(e.to_string()))?,
				)
				.to_string_lossy()
				.into_owned()
		} else {
			v.clone()
		};
		out = out.replace(&format!("{{{k}}}"), &value);
	}
	Ok(out)
}
fn processor_output_hash(value: &str) -> Result<&str> {
	let hash = value
		.strip_prefix('\'')
		.and_then(|quoted| quoted.strip_suffix('\''))
		.unwrap_or(value);
	if hash.len() != 40 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
		return Err(Error::Invalid(
			"invalid loader processor output hash".into(),
		));
	}
	Ok(hash)
}
fn ignore_vanilla_client_module(jvm: &mut [String], game_version: &str) {
	let vanilla_jar = format!("{game_version}.jar");
	for argument in jvm {
		if argument.starts_with("-DignoreList=")
			&& !argument.split(',').any(|part| part == vanilla_jar)
		{
			argument.push(',');
			argument.push_str(&vanilla_jar);
		}
	}
}
fn extract_natives(archive: &Path, directory: &Path, exclude: Option<&[String]>) -> Result<()> {
	let mut zip = zip::ZipArchive::new(std::fs::File::open(archive)?)?;
	if zip.len() > 10000 {
		return Err(Error::Invalid("native archive has too many entries".into()));
	}
	let mut total = 0u64;
	for index in 0..zip.len() {
		let mut file = zip.by_index(index)?;
		if file.is_dir()
			|| file.name().starts_with("META-INF/")
			|| exclude.is_some_and(|xs| xs.iter().any(|prefix| file.name().starts_with(prefix)))
		{
			continue;
		}
		if file.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000) {
			return Err(Error::Invalid("native archive contains symlink".into()));
		}
		total = total.saturating_add(file.size());
		if total > 512 * 1024 * 1024 {
			return Err(Error::Invalid("native archive too large".into()));
		}
		let path = crate::files::contained(directory, file.name())?;
		if let Some(parent) = path.parent() {
			std::fs::create_dir_all(parent)?;
		}
		let mut out = std::fs::File::create(path)?;
		std::io::copy(&mut file, &mut out)?;
	}
	Ok(())
}
fn capture_log<R: tokio::io::AsyncRead + Unpin + Send + 'static>(
	reader: R,
	path: PathBuf,
	token: String,
) {
	tokio::spawn(async move {
		use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
		let token = zeroize::Zeroizing::new(token);
		let mut reader = tokio::io::BufReader::new(reader);
		let mut line = String::new();
		let Ok(mut file) = tokio::fs::OpenOptions::new()
			.append(true)
			.create(true)
			.open(path)
			.await
		else {
			return;
		};
		loop {
			line.clear();
			match reader.read_line(&mut line).await {
				Ok(0) | Err(_) => break,
				Ok(_) => {
					let safe = if token.is_empty() {
						line.clone()
					} else {
						line.replace(token.as_str(), "[redacted]")
					};
					if file.write_all(safe.as_bytes()).await.is_err() {
						break;
					}
				}
			}
		}
	});
}
struct SecretMap(HashMap<&'static str, String>);
impl std::ops::Deref for SecretMap {
	type Target = HashMap<&'static str, String>;
	fn deref(&self) -> &Self::Target {
		&self.0
	}
}
impl Drop for SecretMap {
	fn drop(&mut self) {
		use zeroize::Zeroize;
		for value in self.0.values_mut() {
			value.zeroize();
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn neoforge_does_not_load_the_vanilla_jar_as_a_second_minecraft_module() {
		let mut args = vec!["-DignoreList=client-extra,neoforge-21.1.250.jar".into()];
		ignore_vanilla_client_module(&mut args, "1.21.1");
		assert_eq!(
			args[0],
			"-DignoreList=client-extra,neoforge-21.1.250.jar,1.21.1.jar"
		);
		ignore_vanilla_client_module(&mut args, "1.21.1");
		assert_eq!(args[0].matches("1.21.1.jar").count(), 1);
	}
	#[test]
	fn upstream_mirror_maven_coordinates_use_official_repositories() {
		let mirror = "https://launcher-meta.modrinth.com/maven/";
		for (coordinate, loader, expected) in [
			(
				"org.ow2.asm:asm:9.8",
				Loader::Fabric,
				"https://repo.maven.apache.org/maven2/",
			),
			(
				"net.fabricmc:intermediary:1.21.1",
				Loader::Fabric,
				"https://maven.fabricmc.net/",
			),
			(
				"org.quiltmc:quilt-loader:0.28.1",
				Loader::Quilt,
				"https://maven.quiltmc.org/repository/release/",
			),
			(
				"net.minecraftforge:forge:52.1.14",
				Loader::Forge,
				"https://maven.minecraftforge.net/",
			),
			(
				"net.neoforged:neoforge:21.1.219",
				Loader::Neoforge,
				"https://maven.neoforged.net/releases/",
			),
		] {
			assert_eq!(library_base(mirror, coordinate, loader), expected);
		}
		assert_eq!(
			library_base(mirror, "unknown:library:1", Loader::Fabric),
			mirror
		);
		assert_eq!(
			library_base(
				"https://libraries.minecraft.net/",
				"org.ow2.asm:asm:9.8",
				Loader::Fabric
			),
			"https://libraries.minecraft.net/"
		);
	}
	#[test]
	fn arguments_keep_spaces_and_tokens_out_of_models() {
		let values = HashMap::from([
			("game_directory", "/home/test/my game".into()),
			("auth_access_token", "secret".into()),
		]);
		assert_eq!(
			substitute("${game_directory}", &values).unwrap(),
			"/home/test/my game"
		);
		assert!(substitute("${unhandled}", &values).is_err());
	}
	#[test]
	fn processor_placeholders_resolve_maven() {
		let values = HashMap::from([("ROOT".into(), "/game".into())]);
		assert_eq!(
			processor_argument("{ROOT}/libraries", &values, Path::new("/cache")).unwrap(),
			"/game/libraries"
		);
		assert!(processor_argument("[invalid]", &values, Path::new("/cache")).is_err());
	}
	#[test]
	fn forge_processor_hashes_accept_one_metadata_quote_pair() {
		let hash = "2a8064cf9359dd03125bc6f4aa99715dbbf50492";
		assert_eq!(processor_output_hash(hash).unwrap(), hash);
		assert_eq!(processor_output_hash(&format!("'{hash}'")).unwrap(), hash);
		assert!(processor_output_hash(&format!("''{hash}''")).is_err());
		assert!(processor_output_hash("'not-a-hash'").is_err());
	}
}
