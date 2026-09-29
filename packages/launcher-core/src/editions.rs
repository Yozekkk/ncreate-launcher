//! Versioned official manifests stay unavailable until a trusted production provider is configured.
use crate::files::{Change, contained, safe_relative};
use crate::{
	CreateInstance, EditionAvailability, EditionManifest, Engine, Error, InstalledContent,
	Instance, ManifestProviders, Operation, Result, UpdatePlan, UpdatePolicy,
};
use std::collections::{BTreeMap, HashSet};
impl Engine {
	pub async fn configure_manifest_providers(&self, providers: ManifestProviders) -> Result<()> {
		for channel in [&providers.stable, &providers.beta] {
			for (id, url) in channel {
				if !["minimal", "standard", "ultra"].contains(&id.as_str()) {
					return Err(Error::Invalid("unknown official edition".into()));
				}
				self.downloads.trust_manifest_origin(url)?;
			}
		}
		let path = self.root.join("manifest-providers.json");
		let temporary = path.with_extension("next");
		tokio::fs::write(&temporary, serde_json::to_vec_pretty(&providers)?).await?;
		tokio::fs::rename(temporary, path).await?;
		*self.providers.lock().await = providers;
		Ok(())
	}
	pub async fn edition_availability(&self, channel: &str) -> Result<Vec<EditionAvailability>> {
		validate_channel(channel)?;
		let providers = self.providers.lock().await.clone();
		let urls = if channel == "beta" {
			providers.beta
		} else {
			providers.stable
		};
		let mut editions = Vec::new();
		for id in ["minimal", "standard", "ultra"] {
			let result = if let Some(url) = urls.get(id) {
				self.downloads.trust_manifest_origin(url)?;
				let op = self.begin("edition_manifest", None);
				let result = self.downloads.json::<EditionManifest>(url, &op).await;
				op.finish(&result);
				match result {
					Ok(manifest) => match validate_manifest(&manifest, id, channel) {
						Ok(()) => EditionAvailability {
							id: id.into(),
							channel: channel.into(),
							available: true,
							manifest: Some(manifest),
							error: None,
						},
						Err(e) => EditionAvailability {
							id: id.into(),
							channel: channel.into(),
							available: false,
							manifest: None,
							error: Some(e.to_string()),
						},
					},
					Err(Error::Network(e))
						if e.status() == Some(reqwest::StatusCode::NOT_FOUND) =>
					{
						EditionAvailability {
							id: id.into(),
							channel: channel.into(),
							available: false,
							manifest: None,
							error: None,
						}
					}
					Err(e) => EditionAvailability {
						id: id.into(),
						channel: channel.into(),
						available: false,
						manifest: None,
						error: Some(e.to_string()),
					},
				}
			} else {
				EditionAvailability {
					id: id.into(),
					channel: channel.into(),
					available: false,
					manifest: None,
					error: None,
				}
			};
			editions.push(result);
		}
		Ok(editions)
	}
	async fn remote_manifest(
		&self,
		id: &str,
		channel: &str,
		op: &Operation,
	) -> Result<EditionManifest> {
		validate_channel(channel)?;
		let providers = self.providers.lock().await;
		let map = if channel == "beta" {
			&providers.beta
		} else {
			&providers.stable
		};
		let url = map
			.get(id)
			.ok_or_else(|| {
				Error::Invalid(
					"official edition is coming soon: no production manifest is configured".into(),
				)
			})?
			.clone();
		drop(providers);
		self.downloads.trust_manifest_origin(&url)?;
		let manifest = match self.downloads.json(&url, op).await {
			Err(Error::Network(e)) if e.status() == Some(reqwest::StatusCode::NOT_FOUND) => {
				return Err(Error::Invalid(
					"official edition is coming soon: no manifest has been published".into(),
				));
			}
			result => result?,
		};
		validate_manifest(&manifest, id, channel)?;
		Ok(manifest)
	}
	pub async fn install_edition(
		&self,
		id: &str,
		channel: &str,
		op: &Operation,
	) -> Result<Instance> {
		let manifest = self.remote_manifest(id, channel, op).await?;
		let mut instance = self
			.create_instance(CreateInstance {
				name: format!("NCreate {}", manifest.id),
				game_version: manifest.minecraft.clone(),
				loader: manifest.loader.kind,
				loader_version: manifest.loader.version.clone(),
				memory_mb: manifest.memory.recommended_mb,
				java_path: None,
			})
			.await?;
		instance.kind = "official".into();
		instance.edition = Some(id.into());
		self.save_instance(&instance).await?;
		let _guard = self.mutation.lock().await;
		self.apply_manifest(&instance, &manifest, op).await?;
		self.instance(&instance.id).await
	}
	pub async fn check_edition_update(
		&self,
		instance_id: &str,
		channel: &str,
	) -> Result<UpdatePlan> {
		let instance = self.instance(instance_id).await?;
		let id = instance.edition.as_deref().ok_or_else(|| {
			Error::Invalid("custom instances do not use official manifests".into())
		})?;
		let op = self.begin("edition_update_check", Some(instance_id));
		let result = async {
			let manifest = self.remote_manifest(id, channel, &op).await?;
			self.manifest_plan(&instance, &manifest).await
		}
		.await;
		op.finish(&result);
		result
	}
	pub async fn apply_edition_update(
		&self,
		instance_id: &str,
		channel: &str,
		op: &Operation,
	) -> Result<Instance> {
		let _guard = self.mutation.lock().await;
		self.ensure_stopped(instance_id).await?;
		let instance = self.instance(instance_id).await?;
		let id = instance.edition.as_deref().ok_or_else(|| {
			Error::Invalid("custom instances do not use official manifests".into())
		})?;
		let manifest = self.remote_manifest(id, channel, op).await?;
		self.apply_manifest(&instance, &manifest, op).await?;
		self.instance(instance_id).await
	}
	async fn manifest_plan(
		&self,
		instance: &Instance,
		manifest: &EditionManifest,
	) -> Result<UpdatePlan> {
		let contents = self.content(&instance.id).await?;
		let old: BTreeMap<_, _> = contents
			.iter()
			.filter(|c| c.managed)
			.map(|c| (c.path.clone(), c))
			.collect();
		let new: BTreeMap<_, _> = manifest.files.iter().map(|f| (f.path.clone(), f)).collect();
		let previous_manifest: Option<String> =
			sqlx::query_scalar("SELECT data FROM launcher_manifests WHERE instance_id=?")
				.bind(&instance.id)
				.fetch_optional(&self.pool)
				.await?;
		let preserved: HashSet<String> = previous_manifest
			.map(|s| serde_json::from_str::<EditionManifest>(&s))
			.transpose()?
			.map(|m| {
				m.files
					.into_iter()
					.filter(|f| f.update_policy == UpdatePolicy::Preserve)
					.map(|f| f.path)
					.collect()
			})
			.unwrap_or_default();
		let root = self.instance_path(&instance.id)?;
		let mut added = Vec::new();
		let mut changed = Vec::new();
		let mut removed = Vec::new();
		let mut unchanged = Vec::new();
		let mut conflicts = Vec::new();
		let mut bytes = 0u64;
		for file in &manifest.files {
			let target = contained(&root, &file.path)?;
			if let Some(previous) = old.get(&file.path) {
				let file_unchanged =
					crate::download::verify_file(&target, &file.sha256, "sha256", file.size)
						.await
						.is_ok();
				if file_unchanged {
					unchanged.push(file.path.clone());
				} else if file.update_policy != UpdatePolicy::Preserve {
					changed.push(file.path.clone());
					bytes = bytes.saturating_add(file.size);
					if crate::download::verify_file(&target, &previous.sha512, "sha512", 0)
						.await
						.is_err()
					{
						conflicts.push(file.path.clone());
					}
				}
			} else {
				added.push(file.path.clone());
				bytes = bytes.saturating_add(file.size);
				if target.exists() {
					conflicts.push(file.path.clone());
				}
			}
		}
		for (path, previous) in old {
			if !new.contains_key(&path) && !preserved.contains(&path) {
				removed.push(path.clone());
				let target = contained(&root, &path)?;
				if crate::download::verify_file(&target, &previous.sha512, "sha512", 0)
					.await
					.is_err()
				{
					conflicts.push(path);
				}
			}
		}
		Ok(UpdatePlan {
			instance_id: instance.id.clone(),
			from_version: instance.manifest_version.clone(),
			to_version: manifest.version.clone(),
			added,
			changed,
			removed,
			unchanged,
			conflicts,
			changelog: manifest.changelog.clone(),
			download_bytes: bytes,
		})
	}
	async fn apply_manifest(
		&self,
		instance: &Instance,
		manifest: &EditionManifest,
		op: &Operation,
	) -> Result<()> {
		let plan = self.manifest_plan(instance, manifest).await?;
		if !plan.conflicts.is_empty() {
			return Err(Error::Invalid(format!(
				"update would replace modified or unowned user files: {}",
				plan.conflicts.join(", ")
			)));
		}
		let contents = self.content(&instance.id).await?;
		let old: BTreeMap<_, _> = contents
			.iter()
			.filter(|c| c.managed)
			.map(|c| (c.path.clone(), c))
			.collect();
		let mut changes = Vec::new();
		let mut records = Vec::new();
		let mut transaction = self.prepare_transaction(&instance.id, Vec::new()).await?;
		let total_files = plan.added.len() + plan.changed.len();
		let mut remaining_files = total_files;
		for file in &manifest.files {
			let needs = plan.added.contains(&file.path) || plan.changed.contains(&file.path);
			if !needs {
				if let Some(record) = old.get(&file.path) {
					records.push((*record).clone());
				}
				continue;
			}
			op.check()?;
			op.edition_files_remaining(remaining_files, total_files);
			let path = transaction
				.directory
				.join("stage")
				.join(safe_relative(&file.path)?);
			self.downloads
				.file(&file.url, &path, &file.sha256, "sha256", file.size, op)
				.await?;
			remaining_files -= 1;
			op.edition_files_remaining(remaining_files, total_files);
			let hash = crate::download::hash_file(&path).await?;
			changes.push(Change {
				path: file.path.clone(),
				before_hash: old.get(&file.path).map(|c| c.sha512.clone()),
				after_hash: Some(hash.clone()),
			});
			records.push(InstalledContent {
				id: old
					.get(&file.path)
					.map(|c| c.id.clone())
					.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
				instance_id: instance.id.clone(),
				project_id: None,
				version_id: None,
				name: file.path.rsplit('/').next().unwrap_or(&file.path).into(),
				kind: "official_file".into(),
				path: file.path.clone(),
				sha512: hash,
				enabled: true,
				managed: true,
				source_url: Some(file.url.clone()),
				version_number: Some(manifest.version.clone()),
			});
		}
		for (path, record) in &old {
			if !manifest.files.iter().any(|f| &f.path == path) && !plan.removed.contains(path) {
				let mut released = (*record).clone();
				released.managed = false;
				records.push(released);
			}
		}
		for path in &plan.removed {
			if let Some(previous) = old.get(path) {
				changes.push(Change {
					path: path.clone(),
					before_hash: Some(previous.sha512.clone()),
					after_hash: None,
				});
			}
		}
		// Suggested servers are created only once; an existing player list is never replaced.
		if !manifest.servers.is_empty()
			&& !contained(&self.instance_path(&instance.id)?, "servers.dat")?.exists()
		{
			let bytes = server_list(&manifest.servers);
			tokio::fs::write(transaction.directory.join("stage/servers.dat"), &bytes).await?;
			changes.push(Change {
				path: "servers.dat".into(),
				before_hash: None,
				after_hash: Some(crate::download::hash(&bytes)),
			});
		}
		transaction.journal.changes = changes;
		transaction.persist().await?;
		self.apply_files(&mut transaction, op).await?;
		self.complete_transaction(&mut transaction, async {
            let mut updated=instance.clone();
            if updated.game_version!=manifest.minecraft||updated.loader!=manifest.loader.kind||updated.loader_version!=manifest.loader.version {updated.status="created".into();}
            updated.game_version=manifest.minecraft.clone();
            updated.loader=manifest.loader.kind;
            updated.loader_version=manifest.loader.version.clone();
            updated.manifest_version=Some(manifest.version.clone());
            updated.memory_mb=updated.memory_mb.clamp(manifest.memory.minimum_mb,manifest.memory.maximum_mb);
            let mut db=self.pool.begin().await?;
            for path in &plan.removed {if let Some(record)=old.get(path) {sqlx::query("DELETE FROM launcher_content WHERE id=?").bind(&record.id).execute(&mut *db).await?;}}
            for record in &records {sqlx::query("INSERT INTO launcher_content(id,instance_id,data) VALUES(?,?,?) ON CONFLICT(id) DO UPDATE SET data=excluded.data").bind(&record.id).bind(&record.instance_id).bind(serde_json::to_string(record)?).execute(&mut *db).await?;}
            sqlx::query("UPDATE launcher_instances SET data=? WHERE id=?").bind(serde_json::to_string(&updated)?).bind(&updated.id).execute(&mut *db).await?;
            sqlx::query("INSERT INTO launcher_manifests(instance_id,data) VALUES(?,?) ON CONFLICT(instance_id) DO UPDATE SET data=excluded.data").bind(&instance.id).bind(serde_json::to_string(manifest)?).execute(&mut *db).await?;
            db.commit().await?;
            Ok(())
        }).await?;
		Ok(())
	}
}
fn validate_channel(channel: &str) -> Result<()> {
	if !["stable", "beta"].contains(&channel) {
		return Err(Error::Invalid("unsupported release channel".into()));
	}
	Ok(())
}
pub fn validate_manifest(manifest: &EditionManifest, id: &str, channel: &str) -> Result<()> {
	if manifest.schema_version != 1
		|| manifest.id != id
		|| !["minimal", "standard", "ultra"].contains(&id)
		|| manifest.release_channel != channel
		|| manifest.version.is_empty()
		|| manifest.minecraft.is_empty()
		|| manifest.files.len() > 20000
		|| manifest.java.major < 8
		|| manifest.memory.minimum_mb < 512
		|| manifest.memory.maximum_mb > 65536
		|| manifest.memory.minimum_mb > manifest.memory.recommended_mb
		|| manifest.memory.recommended_mb > manifest.memory.maximum_mb
	{
		return Err(Error::Invalid("invalid official edition manifest".into()));
	}
	validate_channel(channel)?;
	safe_relative(&manifest.minecraft)?;
	if manifest.loader.kind != crate::Loader::Vanilla
		&& manifest.loader.version.as_deref().is_none_or(str::is_empty)
	{
		return Err(Error::Invalid("manifest loader version is required".into()));
	}
	if manifest.launch.jvm_args.len() > 100
		|| manifest.launch.game_args.len() > 100
		|| manifest
			.launch
			.jvm_args
			.iter()
			.chain(&manifest.launch.game_args)
			.any(|arg| arg.len() > 4096 || arg.contains('\0'))
	{
		return Err(Error::Invalid(
			"manifest launch arguments exceed limits".into(),
		));
	}
	if manifest.servers.len() > 50
		|| manifest.servers.iter().any(|s| {
			s.name.is_empty()
				|| s.name.len() > 120
				|| s.address.is_empty()
				|| s.address.len() > 255
				|| s.address
					.chars()
					.any(|c| c.is_whitespace() || c.is_control())
				|| s.address.contains('/')
				|| s.address.contains('\\')
		}) {
		return Err(Error::Invalid("invalid suggested server".into()));
	}
	let mut paths = HashSet::new();
	let mut total = 0u64;
	for file in &manifest.files {
		safe_relative(&file.path)?;
		let first = file
			.path
			.split('/')
			.next()
			.unwrap_or("")
			.to_ascii_lowercase();
		let lower_path = file.path.to_ascii_lowercase();
		if [
			"saves",
			"screenshots",
			"logs",
			"options.txt",
			"servers.dat",
			".ncreate-runtime",
			".ncreate-icon.png",
		]
		.contains(&first.as_str())
			|| [
				".exe", ".com", ".msi", ".dll", ".so", ".dylib", ".bat", ".cmd", ".ps1", ".sh",
			]
			.iter()
			.any(|extension| lower_path.ends_with(extension))
			|| !paths.insert(file.path.to_ascii_lowercase())
			|| file.sha256.len() != 64
			|| !file.sha256.chars().all(|c| c.is_ascii_hexdigit())
			|| file.size > 2 * 1024 * 1024 * 1024
		{
			return Err(Error::Invalid(
				"manifest contains unsafe, duplicate or invalid file".into(),
			));
		}
		let url = reqwest::Url::parse(&file.url)
			.map_err(|_| Error::Invalid("invalid manifest file URL".into()))?;
		crate::download::validate_url(&url)?;
		total = total.saturating_add(file.size);
		if total > 32 * 1024 * 1024 * 1024 {
			return Err(Error::Invalid(
				"edition exceeds total download limit".into(),
			));
		}
	}
	if manifest.launch.jvm_args.iter().any(|a| {
		!a.starts_with('-')
			|| [
				"-javaagent:",
				"-agentpath:",
				"-agentlib:",
				"-Xbootclasspath",
				"-XX:OnError",
				"-XX:OnOutOfMemoryError",
				"-Djava.system.class.loader",
				"-Djava.library.path",
			]
			.iter()
			.any(|p| a.starts_with(p))
			|| [
				"-cp",
				"-classpath",
				"--class-path",
				"-jar",
				"-m",
				"--module",
			]
			.contains(&a.as_str())
	}) {
		return Err(Error::Invalid(
			"remote edition cannot override the launch boundary or execute agents/commands".into(),
		));
	}

	Ok(())
}
fn server_list(servers: &[crate::ManifestServer]) -> Vec<u8> {
	fn string(output: &mut Vec<u8>, value: &str) {
		output.extend_from_slice(&(value.len() as u16).to_be_bytes());
		output.extend_from_slice(value.as_bytes());
	}
	let mut output = vec![10, 0, 0, 9];
	string(&mut output, "servers");
	output.push(10);
	output.extend_from_slice(&(servers.len() as i32).to_be_bytes());
	for server in servers {
		output.push(8);
		string(&mut output, "name");
		string(&mut output, &server.name);
		output.push(8);
		string(&mut output, "ip");
		string(&mut output, &server.address);
		output.push(0);
	}
	output.push(0);
	output
}

#[cfg(test)]
mod tests {
	use super::*;
	fn fixture() -> EditionManifest {
		serde_json::from_value(serde_json::json!({"schemaVersion":1,"id":"minimal","version":"1.0.0","minecraft":"1.21.1","loader":{"kind":"vanilla","version":null},"files":[{"path":"mods/a.jar","url":"https://cdn.modrinth.com/a.jar","sha256":"a".repeat(64),"size":5,"required":true,"updatePolicy":"managed_only"}],"java":{"major":21},"memory":{"minimumMb":512,"recommendedMb":2048,"maximumMb":8192},"releaseChannel":"stable"})).unwrap()
	}
	#[test]
	fn schema_uses_sha256_and_protects_user_saves() {
		let mut m = fixture();
		assert!(validate_manifest(&m, "minimal", "stable").is_ok());
		m.files[0].path = "saves/world/level.dat".into();
		assert!(validate_manifest(&m, "minimal", "stable").is_err());
		m.files[0].path = "Saves/world/level.dat".into();
		assert!(validate_manifest(&m, "minimal", "stable").is_err());
		m.files[0].path = "mods/setup.exe".into();
		assert!(validate_manifest(&m, "minimal", "stable").is_err());
	}
	#[test]
	fn channel_is_explicit() {
		assert!(validate_manifest(&fixture(), "minimal", "beta").is_err());
	}
	#[tokio::test]
	async fn default_github_provider_can_still_show_coming_soon() {
		let root = tempfile::tempdir().unwrap();
		let pool = sqlx::sqlite::SqlitePoolOptions::new()
			.max_connections(1)
			.connect("sqlite::memory:")
			.await
			.unwrap();
		let engine = Engine::open(root.path().to_path_buf(), pool).await.unwrap();
		assert_eq!(
			engine.providers.lock().await.stable["minimal"],
			"https://raw.githubusercontent.com/Yozekkk/ncreate-manifests/main/channels/stable/minimal.json"
		);
		engine
			.configure_manifest_providers(ManifestProviders::default())
			.await
			.unwrap();
		let reopened = Engine::open(root.path().to_path_buf(), engine.pool.clone())
			.await
			.unwrap();
		assert!(reopened.providers.lock().await.stable.is_empty());
		assert!(
			reopened
				.edition_availability("stable")
				.await
				.unwrap()
				.iter()
				.all(|e| !e.available && e.manifest.is_none())
		);
	}
}
