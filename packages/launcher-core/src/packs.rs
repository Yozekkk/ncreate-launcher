//! Safe mrpack import/export. Archives never write outside a fresh NCreate instance.
use crate::files::{Change, contained, safe_relative};
use crate::{CreateInstance, Engine, Error, InstalledContent, Instance, Loader, Operation, Result};
use serde::{Deserialize, Serialize};
use std::{
	collections::{BTreeMap, HashSet},
	io::{Read, Write},
	path::Path,
};
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackIndex {
	format_version: u32,
	game: String,
	version_id: String,
	name: String,
	files: Vec<PackFile>,
	dependencies: BTreeMap<String, String>,
}
#[derive(Serialize, Deserialize)]
struct PackFile {
	path: String,
	hashes: BTreeMap<String, String>,
	downloads: Vec<String>,
	#[serde(rename = "fileSize")]
	file_size: u64,
	#[serde(default)]
	env: BTreeMap<String, String>,
}
impl Engine {
	pub async fn import_pack(&self, path: &Path, name: &str, op: &Operation) -> Result<Instance> {
		let _guard = self.mutation.lock().await;
		let size = std::fs::metadata(path)?.len();
		if size > 2 * 1024 * 1024 * 1024 {
			return Err(Error::Invalid("modpack archive is too large".into()));
		}
		let mut archive = zip::ZipArchive::new(std::fs::File::open(path)?)?;
		if archive.len() > 20000 {
			return Err(Error::Invalid("modpack archive has too many files".into()));
		}
		let mut index_bytes = Vec::new();
		archive
			.by_name("modrinth.index.json")?
			.take(8 * 1024 * 1024 + 1)
			.read_to_end(&mut index_bytes)?;
		if index_bytes.len() > 8 * 1024 * 1024 {
			return Err(Error::Invalid("modpack index is too large".into()));
		}
		let index: PackIndex = serde_json::from_slice(&index_bytes)?;
		validate_index(&index)?;
		let minecraft = index
			.dependencies
			.get("minecraft")
			.ok_or_else(|| Error::Invalid("modpack does not specify Minecraft".into()))?
			.clone();
		let mut loader = Loader::Vanilla;
		let mut loader_version = None;
		for (key, value) in &index.dependencies {
			let candidate = match key.as_str() {
				"fabric-loader" => Some(Loader::Fabric),
				"quilt-loader" => Some(Loader::Quilt),
				"forge" => Some(Loader::Forge),
				"neoforge" => Some(Loader::Neoforge),
				"minecraft" => None,
				_ => {
					return Err(Error::Invalid(
						"unsupported modpack runtime dependency".into(),
					));
				}
			};
			if let Some(candidate) = candidate {
				if loader != Loader::Vanilla {
					return Err(Error::Invalid(
						"modpack specifies multiple mod loaders".into(),
					));
				}
				loader = candidate;
				loader_version = Some(value.clone());
			}
		}
		let instance = self
			.create_instance(CreateInstance {
				name: if name.trim().is_empty() {
					index.name.clone()
				} else {
					name.into()
				},
				game_version: minecraft,
				loader,
				loader_version,
				memory_mb: 4096,
				java_path: None,
			})
			.await?;
		let result = self.import_into(archive, index, &instance, op).await;
		if let Err(error) = result {
			let mut failed = instance.clone();
			failed.status = "error".into();
			self.save_instance(&failed).await?;
			return Err(error);
		}
		Ok(instance)
	}
	async fn import_into<R: Read + std::io::Seek + Send + 'static>(
		&self,
		mut archive: zip::ZipArchive<R>,
		index: PackIndex,
		instance: &Instance,
		op: &Operation,
	) -> Result<()> {
		let mut changes = Vec::new();
		let mut records = Vec::new();
		let mut files = Vec::new();
		let mut known = HashSet::new();
		for file in index.files {
			if file.env.get("client").is_some_and(|v| v == "unsupported") {
				continue;
			}
			op.check()?;
			let url = file
				.downloads
				.first()
				.ok_or_else(|| Error::Invalid("modpack file has no download source".into()))?
				.clone();
			crate::download::validate_content_url(&url)?;
			let algorithm = if file.hashes.contains_key("sha512") {
				"sha512"
			} else {
				"sha1"
			};
			let hash = file
				.hashes
				.get(algorithm)
				.ok_or_else(|| Error::Invalid("modpack file has no integrity hash".into()))?
				.clone();
			known.insert(file.path.to_ascii_lowercase());
			files.push((file.path, url, hash, algorithm, file.file_size));
		}
		let mut overrides: BTreeMap<String, (usize, bool)> = BTreeMap::new();
		let mut override_names = HashSet::new();
		let mut total = 0u64;
		for n in 0..archive.len() {
			let file = archive.by_index(n)?;
			if file.is_dir() {
				continue;
			}
			if file.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000) {
				return Err(Error::Invalid("modpack archive contains symlink".into()));
			}
			safe_relative(file.name())?;
			let relative = if let Some(path) = file.name().strip_prefix("client-overrides/") {
				Some(path.to_string())
			} else {
				file.name().strip_prefix("overrides/").map(str::to_string)
			};
			if let Some(relative) = relative {
				safe_relative(&relative)?;
				total = total
					.checked_add(file.size())
					.ok_or_else(|| Error::Invalid("archive size overflow".into()))?;
				if total > 2 * 1024 * 1024 * 1024
					|| file.size() > 512 * 1024 * 1024
					|| file.size() > file.compressed_size().max(1) * 1000
				{
					return Err(Error::Invalid(
						"modpack overrides exceed safe size limits".into(),
					));
				}

				if known.contains(&relative.to_ascii_lowercase()) {
					return Err(Error::Invalid(
						"modpack overrides collide with a downloaded file".into(),
					));
				}
				let client = file.name().starts_with("client-overrides/");
				if !override_names.insert(relative.to_ascii_lowercase()) {
					let Some((_, previous_client)) = overrides.get(&relative) else {
						return Err(Error::Invalid(
							"case-insensitive modpack override collision".into(),
						));
					};
					if *previous_client == client {
						return Err(Error::Invalid("duplicate modpack override entry".into()));
					}
					if *previous_client {
						continue;
					}
				}
				overrides.insert(relative, (n, client));
			}
		}
		let mut transaction = self.prepare_transaction(&instance.id, Vec::new()).await?;
		for (path, url, hash, algorithm, size) in files {
			let target = transaction
				.directory
				.join("stage")
				.join(safe_relative(&path)?);
			self.downloads
				.file(&url, &target, &hash, algorithm, size, op)
				.await?;
			let sha512 = crate::download::hash_file(&target).await?;
			changes.push(Change {
				path: path.clone(),
				before_hash: None,
				after_hash: Some(sha512.clone()),
			});
			records.push(pack_record(instance, &path, sha512, Some(url)));
		}
		for (path, (index, _)) in overrides {
			op.check()?;
			let target = transaction
				.directory
				.join("stage")
				.join(safe_relative(&path)?);
			if let Some(parent) = target.parent() {
				tokio::fs::create_dir_all(parent).await?;
			}
			let stage_path = target.clone();
			let cancellation = op.clone();
			archive = tokio::task::spawn_blocking(move || -> Result<zip::ZipArchive<R>> {
				{
					let mut input = archive.by_index(index)?;
					let expected = input.size();
					let mut output = std::fs::File::create(stage_path)?;
					let mut buffer = vec![0u8; 65536];
					let mut copied = 0u64;
					loop {
						cancellation.check()?;
						let n = input.read(&mut buffer)?;
						if n == 0 {
							break;
						}
						copied = copied
							.checked_add(n as u64)
							.ok_or_else(|| Error::Invalid("archive size overflow".into()))?;
						if copied > expected {
							return Err(Error::Invalid(
								"archive entry exceeds declared size".into(),
							));
						}
						output.write_all(&buffer[..n])?;
					}
					if copied != expected {
						return Err(Error::Invalid("archive entry is truncated".into()));
					}
					output.sync_all()?;
				}
				Ok(archive)
			})
			.await
			.map_err(|_| Error::Invalid("archive extraction worker failed".into()))??;
			let hash = crate::download::hash_file(&target).await?;
			changes.push(Change {
				path: path.clone(),
				before_hash: None,
				after_hash: Some(hash.clone()),
			});
			records.push(pack_record(instance, &path, hash, None));
		}
		transaction.journal.changes = changes;
		transaction.persist().await?;
		self.apply_files(&mut transaction, op).await?;
		self.complete_transaction(&mut transaction, self.save_content(&records))
			.await?;
		Ok(())
	}
	pub async fn install_modpack(
		&self,
		project_id: &str,
		version_id: &str,
		name: &str,
		op: &Operation,
	) -> Result<Instance> {
		crate::content::identifier(project_id)?;
		crate::content::identifier(version_id)?;
		let project = self.project(project_id).await?;
		if project["project_type"].as_str() != Some("modpack") {
			return Err(Error::Invalid("selected project is not a modpack".into()));
		}
		let version: serde_json::Value = self
			.downloads
			.json(
				&format!("https://api.modrinth.com/v2/version/{version_id}"),
				op,
			)
			.await?;
		if version["project_id"].as_str() != Some(project_id) {
			return Err(Error::Invalid(
				"version belongs to a different project".into(),
			));
		}
		let file = version["files"]
			.as_array()
			.and_then(|files| {
				files
					.iter()
					.find(|f| f["primary"].as_bool() == Some(true))
					.or_else(|| files.first())
			})
			.ok_or_else(|| Error::Invalid("modpack has no archive".into()))?;
		let hash = file["hashes"]["sha512"]
			.as_str()
			.ok_or_else(|| Error::Invalid("modpack has no SHA512".into()))?;
		let url = file["url"]
			.as_str()
			.ok_or_else(|| Error::Invalid("modpack has no download URL".into()))?;
		crate::download::validate_content_url(url)?;
		let path = self
			.root
			.join("imports")
			.join(format!("{}.mrpack", op.id()));
		self.downloads
			.file(
				url,
				&path,
				hash,
				"sha512",
				file["size"].as_u64().unwrap_or(0),
				op,
			)
			.await?;
		let result = self.import_pack(&path, name, op).await;
		let _ = tokio::fs::remove_file(path).await;
		result
	}
	pub async fn export_pack(&self, instance_id: &str, path: &Path) -> Result<()> {
		let _guard = self.mutation.lock().await;
		self.ensure_stopped(instance_id).await?;
		let instance = self.instance(instance_id).await?;
		if instance.kind == "official" {
			return Err(Error::Invalid(
				"official editions are distributed by their manifests".into(),
			));
		}
		if path.exists() {
			return Err(Error::Invalid("export destination already exists".into()));
		}
		let root = self.instance_path(instance_id)?;
		let contents = self.content(instance_id).await?;
		let mut index = PackIndex {
			format_version: 1,
			game: "minecraft".into(),
			version_id: "1.0.0".into(),
			name: instance.name.clone(),
			files: Vec::new(),
			dependencies: BTreeMap::from([("minecraft".into(), instance.game_version.clone())]),
		};
		if let Some(version) = instance.loader_version.clone() {
			index.dependencies.insert(
				match instance.loader {
					Loader::Fabric => "fabric-loader",
					Loader::Quilt => "quilt-loader",
					Loader::Forge => "forge",
					Loader::Neoforge => "neoforge",
					Loader::Vanilla => {
						return Err(Error::Invalid(
							"vanilla instance has a loader version".into(),
						));
					}
				}
				.into(),
				version,
			);
		}
		let mut referenced = HashSet::new();
		for item in contents {
			if item.enabled
				&& let Some(url) = item.source_url
				&& crate::download::validate_content_url(&url).is_ok()
			{
				let full = contained(&root, &item.path)?;
				crate::download::verify_file(&full, &item.sha512, "sha512", 0).await?;
				let size = tokio::fs::metadata(&full).await?.len();
				index.files.push(PackFile {
					path: item.path.clone(),
					hashes: BTreeMap::from([("sha512".into(), item.sha512)]),
					downloads: vec![url],
					file_size: size,
					env: BTreeMap::new(),
				});
				referenced.insert(item.path);
			}
		}
		let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
		let result = (|| -> Result<()> {
			let mut writer = zip::ZipWriter::new(std::fs::File::create(&temporary)?);
			let options = zip::write::SimpleFileOptions::default()
				.compression_method(zip::CompressionMethod::Deflated);
			writer.start_file("modrinth.index.json", options)?;
			writer.write_all(&serde_json::to_vec(&index)?)?;
			let mut entries = Vec::new();
			collect_export(&root, &root, &mut entries)?;
			for (relative, file) in entries {
				if referenced.contains(&relative) {
					continue;
				}
				writer.start_file(format!("overrides/{relative}"), options)?;
				let mut file = std::fs::File::open(file)?;
				std::io::copy(&mut file, &mut writer)?;
			}
			writer.finish()?.sync_all()?;
			Ok(())
		})();
		if let Err(error) = result {
			let _ = tokio::fs::remove_file(temporary).await;
			return Err(error);
		}
		tokio::fs::rename(temporary, path).await?;
		Ok(())
	}
	pub async fn duplicate_instance(&self, id: &str, name: &str) -> Result<Instance> {
		let original = self.instance(id).await?;
		let path = self
			.root
			.join("imports")
			.join(format!("duplicate-{}.mrpack", uuid::Uuid::new_v4()));
		tokio::fs::create_dir_all(
			path.parent()
				.ok_or_else(|| Error::Invalid("invalid import path".into()))?,
		)
		.await?;
		self.export_pack(id, &path).await?;
		let op = self.begin("duplicate", None);
		let result = self.import_pack(&path, name, &op).await;
		let _ = tokio::fs::remove_file(&path).await;
		let mut copy = result?;
		copy.memory_mb = original.memory_mb;
		copy.java_path = original.java_path;
		self.save_instance(&copy).await?;
		Ok(copy)
	}
	pub async fn set_instance_icon(&self, id: &str, path: &Path) -> Result<Instance> {
		if tokio::fs::metadata(path).await?.len() > 4 * 1024 * 1024 {
			return Err(Error::Invalid("instance icon exceeds 4 MiB".into()));
		}
		let bytes = tokio::fs::read(path).await?;
		if bytes.len() > 4 * 1024 * 1024 || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
			return Err(Error::Invalid(
				"instance icon must be a PNG under 4 MiB".into(),
			));
		}
		let mut decoder =
			image::ImageReader::with_format(std::io::Cursor::new(&bytes), image::ImageFormat::Png);
		let mut limits = image::Limits::default();
		limits.max_image_width = Some(1024);
		limits.max_image_height = Some(1024);
		limits.max_alloc = Some(8 * 1024 * 1024);
		decoder.limits(limits);
		decoder
			.decode()
			.map_err(|_| Error::Invalid("invalid or oversized PNG icon".into()))?;
		let target = self.instance_path(id)?.join(".ncreate-icon.png");
		tokio::fs::write(&target, &bytes).await?;
		let mut instance = self.instance(id).await?;
		use base64::Engine;
		instance.icon = Some(format!(
			"data:image/png;base64,{}",
			base64::prelude::BASE64_STANDARD.encode(bytes)
		));
		self.save_instance(&instance).await?;
		Ok(instance)
	}
}
fn pack_record(
	instance: &Instance,
	path: &str,
	sha512: String,
	source_url: Option<String>,
) -> InstalledContent {
	InstalledContent {
		id: uuid::Uuid::new_v4().to_string(),
		instance_id: instance.id.clone(),
		project_id: None,
		version_id: None,
		name: path.rsplit('/').next().unwrap_or(path).into(),
		kind: if path.starts_with("mods/") {
			"mod"
		} else if path.starts_with("resourcepacks/") {
			"resourcepack"
		} else {
			"file"
		}
		.into(),
		path: path.into(),
		sha512,
		enabled: !path.ends_with(".disabled"),
		managed: false,
		source_url,
		version_number: None,
	}
}
fn validate_index(index: &PackIndex) -> Result<()> {
	if index.format_version != 1 || index.game != "minecraft" || index.files.len() > 20000 {
		return Err(Error::Invalid(
			"unsupported or oversized modpack index".into(),
		));
	}
	let mut paths = HashSet::new();
	let mut total = 0u64;
	for file in &index.files {
		safe_relative(&file.path)?;
		if !paths.insert(file.path.to_ascii_lowercase()) {
			return Err(Error::Invalid("duplicate modpack path".into()));
		}
		if file.file_size > 2 * 1024 * 1024 * 1024 {
			return Err(Error::Invalid("modpack file too large".into()));
		}
		total = total.saturating_add(file.file_size);
		if total > 16 * 1024 * 1024 * 1024 {
			return Err(Error::Invalid("modpack total size exceeds limit".into()));
		}
		if !file.hashes.iter().any(|(kind, hash)| {
			(kind == "sha1" && hash.len() == 40 || kind == "sha512" && hash.len() == 128)
				&& hash.chars().all(|c| c.is_ascii_hexdigit())
		}) {
			return Err(Error::Invalid("modpack file has invalid hash".into()));
		}
		for url in &file.downloads {
			crate::download::validate_content_url(url)?;
		}
	}
	Ok(())
}
fn collect_export(
	root: &Path,
	directory: &Path,
	out: &mut Vec<(String, std::path::PathBuf)>,
) -> Result<()> {
	for entry in std::fs::read_dir(directory)? {
		let entry = entry?;
		let relative = entry
			.path()
			.strip_prefix(root)
			.map_err(|_| Error::Invalid("export escaped instance".into()))?
			.to_string_lossy()
			.replace('\\', "/");
		let first = relative.split('/').next().unwrap_or("");
		if [
			"saves",
			"screenshots",
			"logs",
			".ncreate-runtime",
			".ncreate-icon.png",
		]
		.contains(&first)
		{
			continue;
		}
		let metadata = entry.metadata()?;
		if entry.file_type()?.is_symlink() {
			return Err(Error::Invalid("cannot export symlinked files".into()));
		}
		if metadata.is_dir() {
			collect_export(root, &entry.path(), out)?;
		} else if metadata.is_file() {
			if out.len() > 20000 || metadata.len() > 512 * 1024 * 1024 {
				return Err(Error::Invalid("export exceeds archive limits".into()));
			}
			safe_relative(&relative)?;
			out.push((relative, entry.path()));
		}
	}
	Ok(())
}
#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn rejects_archive_downloads_from_unknown_sources() {
		let index = PackIndex {
			format_version: 1,
			game: "minecraft".into(),
			version_id: "1".into(),
			name: "fixture".into(),
			files: vec![PackFile {
				path: "mods/x.jar".into(),
				hashes: BTreeMap::from([("sha512".into(), "a".repeat(128))]),
				downloads: vec!["https://untrusted.example/file.jar".into()],
				file_size: 5,
				env: BTreeMap::new(),
			}],
			dependencies: BTreeMap::new(),
		};
		assert!(validate_index(&index).is_err());
	}
}
