//! Transactional NCreate installation engine, adapted from the upstream desktop backend.
mod content;
mod download;
mod editions;
mod files;
mod java;
mod minecraft;
mod models;
mod packs;
pub use download::{DownloadManager, Operation};
pub use models::*;
use sqlx::SqlitePool;
use std::{path::PathBuf, sync::Arc};
use tokio::sync::Mutex;

#[derive(thiserror::Error, Debug)]
pub enum Error {
	#[error("NCREATE_JAVA:{0}")]
	Java(String),
	#[error("{0}")]
	Invalid(String),
	#[error("operation cancelled")]
	Cancelled,
	#[error("file operation failed: {0}")]
	Io(#[from] std::io::Error),
	#[error("database operation failed: {0}")]
	Database(#[from] sqlx::Error),
	#[error("metadata could not be read: {0}")]
	Json(#[from] serde_json::Error),
	#[error("network request failed")]
	Network(#[from] reqwest::Error),
	#[error("archive could not be read: {0}")]
	Zip(#[from] zip::result::ZipError),
}
pub type Result<T> = std::result::Result<T, Error>;
pub(crate) struct GameProcess {
	pub child: tokio::process::Child,
	pub native_alias: Option<tempfile::TempDir>,
}
impl Drop for GameProcess {
	fn drop(&mut self) {
		// Closing the launcher must not remove a path still used by a living game.
		// Normal stop/exit removes the alias; an abrupt launcher exit leaves a tiny
		// private directory for the OS temporary-directory cleanup.
		if !matches!(self.child.try_wait(), Ok(Some(_)))
			&& let Some(directory) = self.native_alias.take()
		{
			let _ = directory.keep();
		}
	}
}
pub struct Engine {
	pub(crate) root: PathBuf,
	pub(crate) pool: SqlitePool,
	pub(crate) downloads: DownloadManager,
	pub(crate) mutation: Mutex<()>,
	pub(crate) running: Mutex<std::collections::HashMap<String, GameProcess>>,
	pub(crate) providers: Mutex<ManifestProviders>,
}
impl Engine {
	pub async fn open(root: PathBuf, pool: SqlitePool) -> Result<Arc<Self>> {
		tokio::fs::create_dir_all(root.join("instances")).await?;
		let mut tx = pool.begin().await?;
		sqlx::query(
			"CREATE TABLE IF NOT EXISTS launcher_instances (id TEXT PRIMARY KEY, data TEXT NOT NULL)",
		)
		.execute(&mut *tx)
		.await?;
		sqlx::query("CREATE TABLE IF NOT EXISTS launcher_content (id TEXT PRIMARY KEY, instance_id TEXT NOT NULL, data TEXT NOT NULL)").execute(&mut *tx).await?;
		sqlx::query("CREATE TABLE IF NOT EXISTS launcher_manifests (instance_id TEXT PRIMARY KEY, data TEXT NOT NULL)").execute(&mut *tx).await?;
		sqlx::query(
			"CREATE TABLE IF NOT EXISTS launcher_jobs (id TEXT PRIMARY KEY, data TEXT NOT NULL)",
		)
		.execute(&mut *tx)
		.await?;
		let version: i64 = sqlx::query_scalar("PRAGMA user_version")
			.fetch_one(&mut *tx)
			.await?;
		if version < 3 {
			sqlx::query("PRAGMA user_version=3")
				.execute(&mut *tx)
				.await?;
		}
		tx.commit().await?;
		let providers_path = root.join("manifest-providers.json");
		let providers = match tokio::fs::read(providers_path).await {
			Ok(bytes) => serde_json::from_slice(&bytes)?,
			Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
				ManifestProviders::github_ncreate()
			}
			Err(e) => return Err(e.into()),
		};
		let engine = Arc::new(Self {
			root,
			pool,
			downloads: DownloadManager::new()?,
			mutation: Mutex::new(()),
			running: Mutex::new(std::collections::HashMap::new()),
			providers: Mutex::new(providers),
		});
		engine.recover().await?;
		Ok(engine)
	}
	pub fn begin(&self, operation: &str, instance_id: Option<&str>) -> Operation {
		self.downloads.begin(operation, instance_id)
	}
	pub fn cancel(&self, id: &str) -> Result<()> {
		self.downloads.cancel(id)
	}
	pub fn jobs(&self) -> Vec<Progress> {
		self.downloads.jobs()
	}
	pub(crate) async fn read_json<T: serde::de::DeserializeOwned>(
		&self,
		url: &str,
		kind: &str,
	) -> Result<T> {
		let op = self.begin(kind, None);
		let result = self.downloads.json(url, &op).await;
		op.finish(&result);
		result
	}
	pub async fn instances(&self) -> Result<Vec<Instance>> {
		let rows: Vec<String> =
			sqlx::query_scalar("SELECT data FROM launcher_instances ORDER BY id")
				.fetch_all(&self.pool)
				.await?;
		let mut instances = rows
			.into_iter()
			.map(|s| Ok(serde_json::from_str::<Instance>(&s)?))
			.collect::<Result<Vec<_>>>()?;
		for instance in &mut instances {
			instance.mod_count = self
				.content(&instance.id)
				.await?
				.iter()
				.filter(|c| c.kind == "mod")
				.count() as u32;
		}
		for instance in &mut instances {
			self.refresh_running_status(instance).await?;
		}
		Ok(instances)
	}
	pub async fn instance(&self, id: &str) -> Result<Instance> {
		let data: Option<String> =
			sqlx::query_scalar("SELECT data FROM launcher_instances WHERE id=?")
				.bind(id)
				.fetch_optional(&self.pool)
				.await?;
		let mut instance: Instance = serde_json::from_str(
			&data.ok_or_else(|| Error::Invalid("instance not found".into()))?,
		)?;
		self.refresh_running_status(&mut instance).await?;
		Ok(instance)
	}
	async fn refresh_running_status(&self, instance: &mut Instance) -> Result<()> {
		let mut processes = self.running.lock().await;
		if let Some(child) = processes.get_mut(&instance.id) {
			if child.child.try_wait()?.is_none() {
				instance.status = "running".into();
			} else {
				processes.remove(&instance.id);
				if instance.status == "running" {
					instance.status = "ready".into();
				}
			}
		} else if instance.status == "running" {
			instance.status = "ready".into();
		}
		Ok(())
	}

	pub(crate) async fn save_instance(&self, instance: &Instance) -> Result<()> {
		sqlx::query("INSERT INTO launcher_instances(id,data) VALUES(?,?) ON CONFLICT(id) DO UPDATE SET data=excluded.data").bind(&instance.id).bind(serde_json::to_string(instance)?).execute(&self.pool).await?;
		Ok(())
	}
	pub async fn create_instance(&self, request: CreateInstance) -> Result<Instance> {
		validate_create(&request)?;
		let id = uuid::Uuid::new_v4().to_string();
		let directory = self.root.join("instances").join(&id);
		tokio::fs::create_dir_all(&directory).await?;
		let instance = Instance {
			id,
			name: request.name.trim().into(),
			game_version: request.game_version,
			loader: request.loader,
			loader_version: request.loader_version,
			kind: "custom".into(),
			edition: None,
			manifest_version: None,
			status: "created".into(),
			memory_mb: request.memory_mb,
			java_path: request.java_path,
			directory: directory.to_string_lossy().into(),
			icon: None,
			mod_count: 0,
			last_played: None,
		};
		self.validate_instance_java(&instance).await?;
		self.save_instance(&instance).await?;
		Ok(instance)
	}
	pub async fn edit_instance(&self, id: &str, request: CreateInstance) -> Result<Instance> {
		let _guard = self.mutation.lock().await;
		self.ensure_stopped(id).await?;
		validate_create(&request)?;
		let mut instance = self.instance(id).await?;
		if instance.kind == "official"
			&& (instance.game_version != request.game_version
				|| instance.loader != request.loader
				|| instance.loader_version != request.loader_version)
		{
			return Err(Error::Invalid(
				"official instance version is managed by its manifest".into(),
			));
		}
		if instance.game_version != request.game_version
			|| instance.loader != request.loader
			|| instance.loader_version != request.loader_version
		{
			instance.status = "created".into();
		}
		instance.name = request.name.trim().into();
		instance.game_version = request.game_version;
		instance.loader = request.loader;
		instance.loader_version = request.loader_version;
		instance.memory_mb = request.memory_mb;
		instance.java_path = request.java_path;
		self.validate_instance_java(&instance).await?;
		self.save_instance(&instance).await?;
		Ok(instance)
	}
	pub async fn delete_instance(&self, id: &str) -> Result<()> {
		let _guard = self.mutation.lock().await;
		self.ensure_stopped(id).await?;
		self.instance(id).await?;
		let source = self.root.join("instances").join(id);
		let trash = self.root.join("trash");
		tokio::fs::create_dir_all(&trash).await?;
		tokio::fs::rename(source, trash.join(format!("{id}-{}", uuid::Uuid::new_v4()))).await?;
		let mut tx = self.pool.begin().await?;
		for table in ["launcher_content", "launcher_manifests"] {
			sqlx::query(&format!("DELETE FROM {table} WHERE instance_id=?"))
				.bind(id)
				.execute(&mut *tx)
				.await?;
		}
		sqlx::query("DELETE FROM launcher_instances WHERE id=?")
			.bind(id)
			.execute(&mut *tx)
			.await?;
		tx.commit().await?;
		Ok(())
	}
}
fn validate_create(r: &CreateInstance) -> Result<()> {
	if r.name.trim().is_empty()
		|| r.name.len() > 120
		|| r.game_version.is_empty()
		|| r.game_version.len() > 100
		|| !(512..=65536).contains(&r.memory_mb)
	{
		return Err(Error::Invalid(
			"invalid instance name, version or memory limit".into(),
		));
	}
	files::safe_relative(&r.game_version)?;
	if r.loader != Loader::Vanilla && r.loader_version.as_deref().is_none_or(str::is_empty) {
		return Err(Error::Invalid("choose a loader version".into()));
	}
	Ok(())
}

#[cfg(test)]
mod integration_tests;
