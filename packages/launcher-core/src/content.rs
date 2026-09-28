//! Public Modrinth metadata and dependency resolution use the upstream resolver.
use crate::files::{Change, contained, safe_relative};
use crate::{
	ContentInstall, ContentUpdate, Engine, Error, InstalledContent, Operation, Result,
	SearchRequest, SearchResult,
};
use modrinth_content_management::{
	ContentMetadataProvider, ContentType, ResolutionPreferences, ResolveContentRequest, Version,
};
use serde_json::Value;
const API: &str = "https://api.modrinth.com/v2";
#[derive(Clone)]
struct Provider {
	downloads: crate::DownloadManager,
	op: Operation,
	channel: String,
}
#[async_trait::async_trait]
impl ContentMetadataProvider for Provider {
	async fn get_version(
		&mut self,
		id: &str,
	) -> std::result::Result<Option<Version>, modrinth_content_management::Error> {
		identifier(id).map_err(provider_error)?;
		let data: Value = self
			.downloads
			.json(&format!("{API}/version/{id}"), &self.op)
			.await
			.map_err(provider_error)?;
		if !channel_allows(&data, &self.channel) {
			return Ok(None);
		}
		Ok(Some(
			serde_json::from_value(data).map_err(|e| provider_error(Error::Json(e)))?,
		))
	}
	async fn get_project_versions(
		&mut self,
		id: &str,
	) -> std::result::Result<Vec<Version>, modrinth_content_management::Error> {
		identifier(id).map_err(provider_error)?;
		let values: Vec<Value> = self
			.downloads
			.json(&format!("{API}/project/{id}/version"), &self.op)
			.await
			.map_err(provider_error)?;
		values
			.into_iter()
			.filter(|v| channel_allows(v, &self.channel))
			.map(|v| serde_json::from_value(v).map_err(|e| provider_error(Error::Json(e))))
			.collect()
	}
}
fn provider_error(e: Error) -> modrinth_content_management::Error {
	modrinth_content_management::Error::Provider(e.to_string())
}
pub(crate) fn identifier(id: &str) -> Result<()> {
	if id.is_empty()
		|| id.len() > 120
		|| !id
			.chars()
			.all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
	{
		return Err(Error::Invalid(
			"invalid project or version identifier".into(),
		));
	}
	Ok(())
}
pub(crate) fn content_folder(kind: &str) -> Result<&'static str> {
	match kind {
		"mod" => Ok("mods"),
		"resourcepack" => Ok("resourcepacks"),
		"shader" => Ok("shaderpacks"),
		"datapack" => Err(Error::Invalid(
			"select a Minecraft world to install a datapack".into(),
		)),
		_ => Err(Error::Invalid("unsupported content type".into())),
	}
}
pub(crate) fn compatible(version: &Value, game: &str, loader: crate::Loader, kind: &str) -> bool {
	let game_ok = version["game_versions"]
		.as_array()
		.is_some_and(|xs| xs.iter().any(|x| x.as_str() == Some(game)));
	let loader_ok = kind != "mod"
		|| version["loaders"].as_array().is_some_and(|xs| {
			xs.iter().any(|x| {
				x.as_str() == Some(loader.key())
					|| (loader == crate::Loader::Quilt && x.as_str() == Some("fabric"))
			})
		});
	game_ok && loader_ok
}
fn validate_content_channel(channel: &str) -> Result<()> {
	if matches!(channel, "stable" | "beta") {
		Ok(())
	} else {
		Err(Error::Invalid("unsupported content release channel".into()))
	}
}
fn channel_allows(version: &Value, channel: &str) -> bool {
	version["version_type"].as_str() == Some("release")
		|| (channel == "beta" && version["version_type"].as_str() == Some("beta"))
}

impl Engine {
	pub async fn search(&self, request: SearchRequest) -> Result<SearchResult> {
		if !["mod", "modpack", "resourcepack", "shader"].contains(&request.kind.as_str())
			|| !["relevance", "downloads", "follows", "newest", "updated"]
				.contains(&request.sort.as_str())
			|| request.query.len() > 500
		{
			return Err(Error::Invalid("invalid search request".into()));
		}
		let mut facets = vec![vec![format!("project_type:{}", request.kind)]];
		if let Some(version) = request.game_version {
			facets.push(vec![format!("versions:{version}")]);
		}
		if let Some(loader) = request.loader {
			facets.push(vec![format!("categories:{loader}")]);
		}
		if let Some(category) = request.category {
			facets.push(vec![format!("categories:{category}")]);
		}
		let mut url = reqwest::Url::parse(&format!("{API}/search"))
			.map_err(|_| Error::Invalid("invalid API URL".into()))?;
		url.query_pairs_mut()
			.append_pair("query", &request.query)
			.append_pair("facets", &serde_json::to_string(&facets)?)
			.append_pair("index", &request.sort)
			.append_pair("offset", &request.offset.to_string())
			.append_pair("limit", &request.limit.clamp(1, 50).to_string());
		self.read_json(url.as_str(), "search").await
	}
	pub async fn project(&self, id: &str) -> Result<Value> {
		identifier(id)?;
		self.read_json(&format!("{API}/project/{id}"), "project")
			.await
	}
	pub async fn versions(
		&self,
		project: &str,
		game_version: Option<&str>,
		loader: Option<&str>,
	) -> Result<Vec<Value>> {
		identifier(project)?;
		let mut url = reqwest::Url::parse(&format!("{API}/project/{project}/version"))
			.map_err(|_| Error::Invalid("invalid API URL".into()))?;
		if let Some(game) = game_version {
			url.query_pairs_mut()
				.append_pair("game_versions", &serde_json::to_string(&[game])?);
		}
		if let Some(loader) = loader {
			url.query_pairs_mut()
				.append_pair("loaders", &serde_json::to_string(&[loader])?);
		}
		self.read_json(url.as_str(), "versions").await
	}
	pub async fn project_versions(&self, project: &str, instance_id: &str) -> Result<Vec<Value>> {
		let instance = self.instance(instance_id).await?;
		self.versions(
			project,
			Some(&instance.game_version),
			Some(instance.loader.key()),
		)
		.await
	}
	pub async fn categories(&self, kind: &str) -> Result<Vec<String>> {
		let tags: Vec<Value> = self
			.read_json(&format!("{API}/tag/category"), "categories")
			.await?;
		Ok(tags
			.into_iter()
			.filter(|x| {
				x["project_type"].as_str() == Some(kind)
					&& x["header"].as_str() != Some("modloader")
			})
			.filter_map(|x| x["name"].as_str().map(str::to_owned))
			.collect())
	}
	pub async fn content(&self, instance_id: &str) -> Result<Vec<InstalledContent>> {
		let rows: Vec<String> =
			sqlx::query_scalar("SELECT data FROM launcher_content WHERE instance_id=? ORDER BY id")
				.bind(instance_id)
				.fetch_all(&self.pool)
				.await?;
		rows.into_iter()
			.map(|row| Ok(serde_json::from_str(&row)?))
			.collect()
	}
	pub(crate) async fn save_content(&self, contents: &[InstalledContent]) -> Result<()> {
		let mut tx = self.pool.begin().await?;
		for content in contents {
			sqlx::query("INSERT INTO launcher_content(id,instance_id,data) VALUES(?,?,?) ON CONFLICT(id) DO UPDATE SET data=excluded.data").bind(&content.id).bind(&content.instance_id).bind(serde_json::to_string(content)?).execute(&mut *tx).await?;
		}
		tx.commit().await?;
		Ok(())
	}
	pub async fn install_content(
		&self,
		request: ContentInstall,
		op: &Operation,
	) -> Result<Vec<InstalledContent>> {
		self.install_content_for_channel(request, "stable", op)
			.await
	}
	pub async fn install_content_for_channel(
		&self,
		request: ContentInstall,
		channel: &str,
		op: &Operation,
	) -> Result<Vec<InstalledContent>> {
		validate_content_channel(channel)?;
		let _guard = self.mutation.lock().await;
		self.ensure_stopped(&request.instance_id).await?;
		self.install_content_inner(request, channel, op).await
	}
	async fn install_content_inner(
		&self,
		request: ContentInstall,
		channel: &str,
		op: &Operation,
	) -> Result<Vec<InstalledContent>> {
		let instance = self.instance(&request.instance_id).await?;
		let (records, files, changes) = self.plan_content(request, channel, op).await?;
		let mut transaction = self.prepare_transaction(&instance.id, changes).await?;
		for (path, url, hash, size) in files {
			self.downloads
				.file(
					&url,
					&transaction
						.directory
						.join("stage")
						.join(safe_relative(&path)?),
					&hash,
					"sha512",
					size,
					op,
				)
				.await?;
		}
		self.apply_files(&mut transaction, op).await?;
		self.complete_transaction(&mut transaction, self.save_content(&records))
			.await?;
		self.content(&instance.id).await
	}
	async fn plan_content(
		&self,
		request: ContentInstall,
		channel: &str,
		op: &Operation,
	) -> Result<(
		Vec<InstalledContent>,
		Vec<(String, String, String, u64)>,
		Vec<Change>,
	)> {
		let instance = self.instance(&request.instance_id).await?;
		let folder = content_folder(&request.kind)?;
		if instance.kind == "official" {
			return Err(Error::Invalid(
				"official content is managed by its edition manifest".into(),
			));
		}
		let existing = self.content(&instance.id).await?;
		// Resolve required versions even when that project is already installed: a
		// pinned dependency may need an upgrade alongside its parent project.
		let existing_ids = Vec::new();
		let provider = Provider {
			downloads: self.downloads.clone(),
			op: op.clone(),
			channel: channel.into(),
		};
		let plan = modrinth_content_management::resolve_content(
			provider,
			ResolveContentRequest {
				project_id: request.project_id,
				version_id: request.version_id,
				content_type: request
					.kind
					.parse::<ContentType>()
					.map_err(|e| Error::Invalid(e.to_string()))?,
				selected: ResolutionPreferences::default(),
				target: ResolutionPreferences {
					game_versions: vec![instance.game_version.clone()],
					loaders: vec![instance.loader.key().into()],
				},
				existing_project_ids: existing_ids,
			},
		)
		.await
		.map_err(|e| Error::Invalid(e.to_string()))?;
		if plan.skipped.iter().any(|s| {
			matches!(
				s.reason,
				modrinth_content_management::SkippedReason::NoCompatibleVersion
					| modrinth_content_management::SkippedReason::ConflictingDependency
					| modrinth_content_management::SkippedReason::MissingVersion
			)
		}) {
			return Err(Error::Invalid(
				"required dependencies could not be resolved compatibly".into(),
			));
		}
		let mut records = Vec::new();
		let mut files = Vec::new();
		let mut changes = Vec::new();
		for selected in std::iter::once(plan.primary).chain(plan.dependencies) {
			op.check()?;
			let version: Value = self
				.downloads
				.json(&format!("{API}/version/{}", selected.version_id), op)
				.await?;
			if !compatible(
				&version,
				&instance.game_version,
				instance.loader,
				&request.kind,
			) {
				return Err(Error::Invalid(
					"selected content version is incompatible with this instance".into(),
				));
			}
			let available = version["files"]
				.as_array()
				.ok_or_else(|| Error::Invalid("version has no files".into()))?;
			let file = available
				.iter()
				.find(|f| f["primary"].as_bool() == Some(true))
				.or_else(|| available.first())
				.ok_or_else(|| Error::Invalid("version has no downloadable file".into()))?;
			let filename = file["filename"]
				.as_str()
				.ok_or_else(|| Error::Invalid("file has no filename".into()))?;
			if !path_util::is_safe_file_name(filename) {
				return Err(Error::Invalid("unsafe content filename".into()));
			}
			let mut path = format!("{folder}/{filename}");
			let url = file["url"]
				.as_str()
				.ok_or_else(|| Error::Invalid("file has no source URL".into()))?
				.to_string();
			let hash = file["hashes"]["sha512"]
				.as_str()
				.ok_or_else(|| Error::Invalid("file has no SHA512".into()))?
				.to_string();
			let size = file["size"]
				.as_u64()
				.ok_or_else(|| Error::Invalid("file has no size".into()))?;
			let before = existing
				.iter()
				.find(|c| c.project_id.as_deref() == Some(&selected.project_id));
			if before.is_some_and(|c| !c.enabled) {
				path.push_str(".disabled");
			}
			if let Some(before) = before
				&& before.path != path
			{
				changes.push(Change {
					path: before.path.clone(),
					before_hash: Some(before.sha512.clone()),
					after_hash: None,
				});
			}
			changes.push(Change {
				path: path.clone(),
				before_hash: before.filter(|c| c.path == path).map(|c| c.sha512.clone()),
				after_hash: Some(hash.clone()),
			});
			records.push(InstalledContent {
				id: before
					.map(|c| c.id.clone())
					.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
				instance_id: instance.id.clone(),
				project_id: Some(selected.project_id),
				version_id: Some(selected.version_id),
				name: version["name"].as_str().unwrap_or(filename).into(),
				kind: request.kind.clone(),
				path: path.clone(),
				sha512: hash.clone(),
				enabled: before.is_none_or(|c| c.enabled),
				managed: false,
				source_url: Some(url.clone()),
				version_number: version["version_number"].as_str().map(str::to_owned),
			});
			files.push((path, url, hash, size));
		}
		Ok((records, files, changes))
	}
	pub async fn toggle_content(&self, id: &str, enabled: bool) -> Result<InstalledContent> {
		let _guard = self.mutation.lock().await;
		let (mut item, instance) = self.content_by_id(id).await?;
		self.ensure_stopped(&instance.id).await?;
		if item.managed {
			return Err(Error::Invalid(
				"official files cannot be toggled independently".into(),
			));
		}
		if item.enabled == enabled {
			return Ok(item);
		}
		let new_path = if enabled {
			item.path
				.strip_suffix(".disabled")
				.ok_or_else(|| Error::Invalid("disabled filename is inconsistent".into()))?
				.to_string()
		} else {
			format!("{}.disabled", item.path)
		};
		let root = self.instance_path(&instance.id)?;
		let old = contained(&root, &item.path)?;
		let target = contained(&root, &new_path)?;
		if target.exists() {
			return Err(Error::Invalid(
				"a user file already occupies the target path".into(),
			));
		}
		crate::download::verify_file(&old, &item.sha512, "sha512", 0).await?;
		tokio::fs::rename(&old, &target).await?;
		item.path = new_path;
		item.enabled = enabled;
		if let Err(e) = self.save_content(std::slice::from_ref(&item)).await {
			tokio::fs::rename(target, old).await?;
			return Err(e);
		}
		Ok(item)
	}
	async fn content_by_id(&self, id: &str) -> Result<(InstalledContent, crate::Instance)> {
		let raw: Option<String> =
			sqlx::query_scalar("SELECT data FROM launcher_content WHERE id=?")
				.bind(id)
				.fetch_optional(&self.pool)
				.await?;
		let content: InstalledContent =
			serde_json::from_str(&raw.ok_or_else(|| Error::Invalid("content not found".into()))?)?;
		let instance = self.instance(&content.instance_id).await?;
		Ok((content, instance))
	}
	pub async fn remove_content(&self, id: &str) -> Result<()> {
		let _guard = self.mutation.lock().await;
		let (item, instance) = self.content_by_id(id).await?;
		self.ensure_stopped(&instance.id).await?;
		if item.managed {
			return Err(Error::Invalid(
				"official files are managed by their manifest".into(),
			));
		}
		let mut tx = self
			.prepare_transaction(
				&instance.id,
				vec![Change {
					path: item.path,
					before_hash: Some(item.sha512),
					after_hash: None,
				}],
			)
			.await?;
		let op = self.begin("remove", Some(&instance.id));
		self.apply_files(&mut tx, &op).await?;
		let result = self
			.complete_transaction(&mut tx, async {
				sqlx::query("DELETE FROM launcher_content WHERE id=?")
					.bind(id)
					.execute(&self.pool)
					.await?;
				Ok(())
			})
			.await;
		op.finish(&result);
		result
	}
	async fn identify_imported_content(&self, instance: &crate::Instance) -> Result<()> {
		let root = self.instance_path(&instance.id)?;
		let mut candidates = Vec::new();
		for item in self.content(&instance.id).await? {
			if item.managed
				|| item.project_id.is_some()
				|| item.version_id.is_some()
				|| !matches!(item.kind.as_str(), "mod" | "resourcepack" | "shader")
			{
				continue;
			}
			let target = contained(&root, &item.path)?;
			if crate::download::verify_file(&target, &item.sha512, "sha512", 0)
				.await
				.is_ok()
			{
				candidates.push(item);
			}
		}
		if candidates.is_empty() {
			return Ok(());
		}
		let op = self.begin("identify_content", Some(&instance.id));
		let result = async {
			for batch in candidates.chunks(256) {
				let hashes = batch
					.iter()
					.map(|item| item.sha512.clone())
					.collect::<Vec<_>>();
				let versions = self.downloads.identify_hashes(&hashes, &op).await?;
				for item in batch {
					let Some(version) = versions.get(&item.sha512) else {
						continue;
					};
					if !compatible(version, &instance.game_version, instance.loader, &item.kind) {
						continue;
					}
					let Some(file) = version["files"].as_array().and_then(|files| {
						files.iter().find(|file| {
							file["hashes"]["sha512"]
								.as_str()
								.is_some_and(|hash| hash.eq_ignore_ascii_case(&item.sha512))
						})
					}) else {
						continue;
					};
					let Some(project) = version["project_id"].as_str() else {
						continue;
					};
					let Some(id) = version["id"].as_str() else {
						continue;
					};
					identifier(project)?;
					identifier(id)?;
					let Ok((mut current, _)) = self.content_by_id(&item.id).await else {
						continue;
					};
					if current.sha512 != item.sha512
						|| current.project_id.is_some()
						|| current.managed
					{
						continue;
					}
					let current_path = contained(&root, &current.path)?;
					if crate::download::verify_file(
						&current_path,
						&current.sha512,
						"sha512",
						file["size"].as_u64().unwrap_or(0),
					)
					.await
					.is_err()
					{
						continue;
					}
					let previous = serde_json::to_string(&current)?;
					current.project_id = Some(project.into());
					current.version_id = Some(id.into());
					current.version_number = version["version_number"].as_str().map(str::to_owned);
					current.name = version["name"].as_str().unwrap_or(&current.name).into();
					sqlx::query("UPDATE launcher_content SET data=? WHERE id=? AND data=?")
						.bind(serde_json::to_string(&current)?)
						.bind(&current.id)
						.bind(previous)
						.execute(&self.pool)
						.await?;
				}
			}
			Ok(())
		}
		.await;
		op.finish(&result);
		// Catalog misses/offline identification never invalidate a successfully imported pack.
		match result {
			Err(Error::Network(_)) | Err(Error::Io(_)) | Err(Error::Json(_)) => Ok(()),
			other => other,
		}
	}

	pub async fn check_content_updates(&self, instance_id: &str) -> Result<Vec<ContentUpdate>> {
		self.check_content_updates_for_channel(instance_id, "stable")
			.await
	}
	pub async fn check_content_updates_for_channel(
		&self,
		instance_id: &str,
		channel: &str,
	) -> Result<Vec<ContentUpdate>> {
		validate_content_channel(channel)?;
		let instance = self.instance(instance_id).await?;
		self.identify_imported_content(&instance).await?;
		let mut updates = Vec::new();
		for item in self.content(instance_id).await? {
			if item.managed {
				continue;
			}
			if let (Some(project), Some(current)) = (&item.project_id, &item.version_id) {
				let versions = self
					.versions(
						project,
						Some(&instance.game_version),
						if item.kind == "mod" {
							Some(instance.loader.key())
						} else {
							None
						},
					)
					.await?;
				if let Some(next) = versions.iter().find(|v| {
					channel_allows(v, channel)
						&& compatible(v, &instance.game_version, instance.loader, &item.kind)
				}) && next["id"].as_str() != Some(current)
				{
					updates.push(ContentUpdate {
						content_id: item.id,
						current_version: current.clone(),
						next_version: next["id"]
							.as_str()
							.ok_or_else(|| Error::Invalid("version has no id".into()))?
							.into(),
						name: item.name,
					});
				}
			}
		}
		Ok(updates)
	}
	pub async fn update_content(
		&self,
		instance_id: &str,
		ids: &[String],
		op: &Operation,
	) -> Result<Vec<InstalledContent>> {
		self.update_content_for_channel(instance_id, ids, "stable", op)
			.await
	}
	pub async fn update_content_for_channel(
		&self,
		instance_id: &str,
		ids: &[String],
		channel: &str,
		op: &Operation,
	) -> Result<Vec<InstalledContent>> {
		validate_content_channel(channel)?;
		let _guard = self.mutation.lock().await;
		self.ensure_stopped(instance_id).await?;
		let updates = self
			.check_content_updates_for_channel(instance_id, channel)
			.await?;
		let mut records = std::collections::BTreeMap::new();
		let mut changes: std::collections::BTreeMap<String, Change> =
			std::collections::BTreeMap::new();
		let mut files = std::collections::BTreeMap::new();
		for update in updates {
			if !ids.is_empty() && !ids.contains(&update.content_id) {
				continue;
			}
			let (item, _) = self.content_by_id(&update.content_id).await?;
			let (planned, downloads, edits) = self
				.plan_content(
					ContentInstall {
						instance_id: instance_id.into(),
						project_id: item.project_id.ok_or_else(|| {
							Error::Invalid("local files have no project update".into())
						})?,
						version_id: Some(update.next_version),
						kind: item.kind,
					},
					channel,
					op,
				)
				.await?;
			for record in planned {
				records.insert(record.id.clone(), record);
			}
			for edit in edits {
				if let Some(other) = changes.get(&edit.path)
					&& other.after_hash != edit.after_hash
				{
					return Err(Error::Invalid(
						"updates disagree on a dependency file".into(),
					));
				}
				changes.insert(edit.path.clone(), edit);
			}
			for file in downloads {
				files.insert(file.0.clone(), file);
			}
		}
		if changes.is_empty() {
			return self.content(instance_id).await;
		}
		let mut tx = self
			.prepare_transaction(instance_id, changes.into_values().collect())
			.await?;
		for (path, url, hash, size) in files.into_values() {
			self.downloads
				.file(
					&url,
					&tx.directory.join("stage").join(safe_relative(&path)?),
					&hash,
					"sha512",
					size,
					op,
				)
				.await?;
		}
		self.apply_files(&mut tx, op).await?;
		self.complete_transaction(
			&mut tx,
			self.save_content(&records.into_values().collect::<Vec<_>>()),
		)
		.await?;
		self.content(instance_id).await
	}
}
#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn compatibility_requires_game_and_loader() {
		let v = serde_json::json!({"game_versions":["1.21.1"],"loaders":["fabric"]});
		assert!(compatible(&v, "1.21.1", crate::Loader::Fabric, "mod"));
		assert!(!compatible(&v, "1.20.1", crate::Loader::Fabric, "mod"));
		assert!(!compatible(&v, "1.21.1", crate::Loader::Forge, "mod"));
	}
}
