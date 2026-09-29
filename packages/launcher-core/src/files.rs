//! Journaled file ownership and crash recovery. Unknown or modified user files are never overwritten.
use crate::{Engine, Error, InstalledContent, Instance, Operation, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub(crate) fn safe_relative(value: &str) -> Result<PathBuf> {
	path_util::SafeRelativeUtf8UnixPathBuf::try_from(value.to_owned())
		.map_err(|_| Error::Invalid("unsafe relative file path".into()))?;
	if value.len() > 1024
		|| value.split('/').any(|x| {
			x.len() > 255
				|| x.chars()
					.any(|c| c.is_control() || matches!(c, '<' | '>' | '"' | '|' | '?' | '*'))
				|| x.is_empty()
				|| x == "." || x.contains(':')
				|| x.ends_with('.')
				|| x.ends_with(' ')
		}) {
		return Err(Error::Invalid("ambiguous file path".into()));
	}
	Ok(PathBuf::from(value))
}
pub(crate) fn contained(root: &Path, relative: &str) -> Result<PathBuf> {
	let path = safe_relative(relative)?;
	let mut current = root.to_path_buf();
	if std::fs::symlink_metadata(root).is_ok_and(|m| m.file_type().is_symlink()) {
		return Err(Error::Invalid("symlinked instance directory".into()));
	}
	for part in path.components() {
		current.push(part);
		if std::fs::symlink_metadata(&current).is_ok_and(|m| m.file_type().is_symlink()) {
			return Err(Error::Invalid("symlinked instance path".into()));
		}
	}
	Ok(current)
}
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Change {
	pub path: String,
	pub before_hash: Option<String>,
	pub after_hash: Option<String>,
}
#[derive(Serialize, Deserialize)]
pub(crate) struct Journal {
	pub id: String,
	pub instance_id: String,
	pub state: String,
	pub changes: Vec<Change>,
	pub content_before: Vec<InstalledContent>,
	pub instance_before: Instance,
	pub manifest_before: Option<String>,
}
pub(crate) struct Transaction {
	pub directory: PathBuf,
	pub journal: Journal,
}
impl Engine {
	pub(crate) fn instance_path(&self, id: &str) -> Result<PathBuf> {
		uuid::Uuid::parse_str(id).map_err(|_| Error::Invalid("invalid instance id".into()))?;
		Ok(self.root.join("instances").join(id))
	}
	pub(crate) async fn prepare_transaction(
		&self,
		id: &str,
		changes: Vec<Change>,
	) -> Result<Transaction> {
		let instance = self.instance(id).await?;
		let transaction_id = uuid::Uuid::new_v4().to_string();
		let directory = self.root.join("transactions").join(&transaction_id);
		tokio::fs::create_dir_all(directory.join("stage")).await?;
		tokio::fs::create_dir_all(directory.join("backup")).await?;
		let manifest: Option<String> =
			sqlx::query_scalar("SELECT data FROM launcher_manifests WHERE instance_id=?")
				.bind(id)
				.fetch_optional(&self.pool)
				.await?;
		let journal = Journal {
			id: transaction_id,
			instance_id: id.into(),
			state: "prepared".into(),
			changes,
			content_before: self.content(id).await?,
			instance_before: instance,
			manifest_before: manifest,
		};
		let tx = Transaction { directory, journal };
		tx.persist().await?;
		Ok(tx)
	}
	pub(crate) async fn apply_files(&self, tx: &mut Transaction, op: &Operation) -> Result<()> {
		let result = self.apply_files_inner(tx, op).await;
		if result.is_err() && tx.journal.state == "applying" {
			self.restore_transaction(tx).await?;
		}
		result
	}
	pub(crate) async fn complete_transaction<F>(
		&self,
		tx: &mut Transaction,
		metadata: F,
	) -> Result<()>
	where
		F: std::future::Future<Output = Result<()>>,
	{
		let result = match metadata.await {
			Ok(()) => self.commit_transaction(tx).await,
			Err(e) => Err(e),
		};
		if result.is_err() {
			self.restore_transaction(tx).await?;
		}
		result
	}
	async fn apply_files_inner(&self, tx: &mut Transaction, op: &Operation) -> Result<()> {
		let root = self.instance_path(&tx.journal.instance_id)?;
		for change in &tx.journal.changes {
			op.check()?;
			let dest = contained(&root, &change.path)?;
			let actual = match crate::download::hash_file(&dest).await {
				Ok(hash) => Some(hash),
				Err(Error::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => None,
				Err(e) => return Err(e),
			};
			if actual != change.before_hash {
				return Err(Error::Invalid(format!(
					"user file changed: {}",
					change.path
				)));
			}
			if change.after_hash.is_some() {
				let hash = crate::download::hash_file(
					&tx.directory
						.join("stage")
						.join(safe_relative(&change.path)?),
				)
				.await?;
				if Some(hash) != change.after_hash {
					return Err(Error::Invalid("staged file integrity mismatch".into()));
				}
			}
		}
		op.check()?;
		op.committing();
		tx.journal.state = "applying".into();
		tx.persist().await?;
		for change in &tx.journal.changes {
			let relative = safe_relative(&change.path)?;
			let dest = contained(&root, &change.path)?;
			if let Some(parent) = dest.parent() {
				tokio::fs::create_dir_all(parent).await?;
			}
			if change.before_hash.is_some() {
				let backup = tx.directory.join("backup").join(&relative);
				if let Some(parent) = backup.parent() {
					tokio::fs::create_dir_all(parent).await?;
				}
				tokio::fs::rename(&dest, backup).await?;
			}
			if change.after_hash.is_some() {
				tokio::fs::rename(tx.directory.join("stage").join(relative), dest).await?;
			}
		}
		Ok(())
	}
	pub(crate) async fn commit_transaction(&self, tx: &mut Transaction) -> Result<()> {
		tx.journal.state = "committed".into();
		tx.persist().await?;
		sqlx::query("INSERT INTO launcher_jobs(id,data) VALUES(?,?) ON CONFLICT(id) DO UPDATE SET data=excluded.data").bind(&tx.journal.id).bind(serde_json::to_string(&tx.journal)?).execute(&self.pool).await?;
		Ok(())
	}
	async fn restore_transaction(&self, tx: &mut Transaction) -> Result<()> {
		let root = self.instance_path(&tx.journal.instance_id)?;
		for change in tx.journal.changes.iter().rev() {
			let relative = safe_relative(&change.path)?;
			let dest = contained(&root, &change.path)?;
			let backup = tx.directory.join("backup").join(&relative);
			if backup.exists() || change.before_hash.is_none() {
				if dest.exists() {
					let current = crate::download::hash_file(&dest).await?;
					if change.after_hash.as_deref() != Some(current.as_str()) {
						let conflict = self
							.root
							.join("recovery-conflicts")
							.join(&tx.journal.id)
							.join(&relative);
						if let Some(parent) = conflict.parent() {
							tokio::fs::create_dir_all(parent).await?;
						}
						tokio::fs::rename(&dest, conflict).await?;
					} else {
						tokio::fs::remove_file(&dest).await?;
					}
				}
				if backup.exists() {
					if let Some(parent) = dest.parent() {
						tokio::fs::create_dir_all(parent).await?;
					}
					tokio::fs::rename(backup, dest).await?;
				}
			}
		}
		let mut db = self.pool.begin().await?;
		sqlx::query("DELETE FROM launcher_content WHERE instance_id=?")
			.bind(&tx.journal.instance_id)
			.execute(&mut *db)
			.await?;
		for content in &tx.journal.content_before {
			sqlx::query("INSERT INTO launcher_content(id,instance_id,data) VALUES(?,?,?)")
				.bind(&content.id)
				.bind(&content.instance_id)
				.bind(serde_json::to_string(content)?)
				.execute(&mut *db)
				.await?;
		}
		sqlx::query("UPDATE launcher_instances SET data=? WHERE id=?")
			.bind(serde_json::to_string(&tx.journal.instance_before)?)
			.bind(&tx.journal.instance_id)
			.execute(&mut *db)
			.await?;
		sqlx::query("DELETE FROM launcher_manifests WHERE instance_id=?")
			.bind(&tx.journal.instance_id)
			.execute(&mut *db)
			.await?;
		if let Some(manifest) = &tx.journal.manifest_before {
			sqlx::query("INSERT INTO launcher_manifests(instance_id,data) VALUES(?,?)")
				.bind(&tx.journal.instance_id)
				.bind(manifest)
				.execute(&mut *db)
				.await?;
		}
		db.commit().await?;
		tx.journal.state = "rolled_back".into();
		tx.persist().await?;
		Ok(())
	}
	pub(crate) async fn recover(&self) -> Result<()> {
		let directory = self.root.join("transactions");
		tokio::fs::create_dir_all(&directory).await?;
		let mut entries = tokio::fs::read_dir(directory).await?;
		while let Some(entry) = entries.next_entry().await? {
			if !entry.file_type().await?.is_dir() {
				continue;
			}
			let path = entry.path();
			if let Ok(bytes) = tokio::fs::read(path.join("journal.json")).await {
				let journal: Journal = serde_json::from_slice(&bytes)?;
				if path.file_name().and_then(|s| s.to_str()) != Some(journal.id.as_str())
					|| uuid::Uuid::parse_str(&journal.id).is_err()
				{
					return Err(Error::Invalid(
						"transaction journal identity mismatch".into(),
					));
				}
				if journal.state == "prepared" {
					tokio::fs::remove_dir_all(&path).await?;
				} else if journal.state == "applying" {
					self.restore_transaction(&mut Transaction {
						directory: path,
						journal,
					})
					.await?;
				} else if journal.state == "committed" {
					sqlx::query(
						"INSERT INTO launcher_jobs(id,data) VALUES(?,?) ON CONFLICT(id) DO NOTHING",
					)
					.bind(&journal.id)
					.bind(serde_json::to_string(&journal)?)
					.execute(&self.pool)
					.await?;
				}
			}
		}
		Ok(())
	}
	pub async fn rollback(&self, instance_id: &str) -> Result<()> {
		let _guard = self.mutation.lock().await;
		self.ensure_stopped(instance_id).await?;
		let instance = self.instance(instance_id).await?;
		let rows: Vec<String> =
			sqlx::query_scalar("SELECT data FROM launcher_jobs ORDER BY rowid DESC")
				.fetch_all(&self.pool)
				.await?;
		for row in rows {
			let journal: Journal = serde_json::from_str(&row)?;
			if journal.instance_id == instance_id
				&& journal.state == "committed"
				&& (instance.kind != "official" || journal.manifest_before.is_some())
			{
				let directory = self.root.join("transactions").join(&journal.id);
				let mut tx = Transaction { directory, journal };
				self.preflight_manual_rollback(&tx).await?;
				self.restore_transaction(&mut tx).await?;
				sqlx::query("UPDATE launcher_jobs SET data=? WHERE id=?")
					.bind(serde_json::to_string(&tx.journal)?)
					.bind(&tx.journal.id)
					.execute(&self.pool)
					.await?;
				return Ok(());
			}
		}
		Err(Error::Invalid(
			"no reversible installation is available".into(),
		))
	}
	pub async fn rollback_available(&self, instance_id: &str) -> Result<bool> {
		let instance = self.instance(instance_id).await?;
		let rows: Vec<String> =
			sqlx::query_scalar("SELECT data FROM launcher_jobs ORDER BY rowid DESC")
				.fetch_all(&self.pool)
				.await?;
		for row in rows {
			let journal: Journal = serde_json::from_str(&row)?;
			if journal.instance_id == instance_id
				&& journal.state == "committed"
				&& (instance.kind != "official" || journal.manifest_before.is_some())
			{
				let directory = self.root.join("transactions").join(&journal.id);
				return Ok(directory.join("journal.json").exists()
					&& journal.changes.iter().all(|change| {
						change.before_hash.as_ref().is_none_or(|_| {
							safe_relative(&change.path).is_ok_and(|relative| {
								directory.join("backup").join(relative).is_file()
							})
						})
					}));
			}
		}
		Ok(false)
	}
	async fn preflight_manual_rollback(&self, tx: &Transaction) -> Result<()> {
		if !tx.directory.join("journal.json").exists() {
			return Err(Error::Invalid("rollback snapshot is missing".into()));
		}
		let root = self.instance_path(&tx.journal.instance_id)?;
		for change in &tx.journal.changes {
			let relative = safe_relative(&change.path)?;
			let current = contained(&root, &change.path)?;
			let actual = match crate::download::hash_file(&current).await {
				Ok(hash) => Some(hash),
				Err(Error::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => None,
				Err(e) => return Err(e),
			};
			if actual != change.after_hash {
				return Err(Error::Invalid(format!(
					"rollback would replace modified user file: {}",
					change.path
				)));
			}
			if let Some(expected) = &change.before_hash {
				let backup = tx.directory.join("backup").join(relative);
				if crate::download::verify_file(&backup, expected, "sha512", 0)
					.await
					.is_err()
				{
					return Err(Error::Invalid(format!(
						"rollback snapshot is missing or damaged: {}",
						change.path
					)));
				}
			}
		}
		Ok(())
	}
	pub(crate) async fn ensure_stopped(&self, id: &str) -> Result<()> {
		let mut running = self.running.lock().await;
		if let Some(child) = running.get_mut(id) {
			if child.try_wait()?.is_none() {
				return Err(Error::Invalid(
					"stop the game before changing its files".into(),
				));
			}
			running.remove(id);
		}
		Ok(())
	}
}
impl Transaction {
	pub async fn persist(&self) -> Result<()> {
		let bytes = serde_json::to_vec(&self.journal)?;
		let temporary = self.directory.join("journal.next");
		let mut file = tokio::fs::File::create(&temporary).await?;
		use tokio::io::AsyncWriteExt;
		file.write_all(&bytes).await?;
		file.sync_all().await?;
		drop(file);
		tokio::fs::rename(temporary, self.directory.join("journal.json")).await?;
		Ok(())
	}
}
#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn paths_reject_traversal_and_devices() {
		for path in [
			"../x",
			"/root",
			"C:/x",
			"mods/../../x",
			"CON.txt",
			"mods/x:ads",
			"mods\\x",
			"mods/x.",
			"mods/invalid?.jar",
			"mods/invalid*.jar",
			"mods/invalid|.jar",
			"mods/invalid\n.jar",
		] {
			assert!(safe_relative(path).is_err(), "{path}");
		}
		assert!(safe_relative("mods/example.jar").is_ok());
	}
	#[cfg(unix)]
	#[test]
	fn rejects_symlink_component() {
		let root = tempfile::tempdir().unwrap();
		std::os::unix::fs::symlink("/tmp", root.path().join("mods")).unwrap();
		assert!(contained(root.path(), "mods/payload").is_err());
	}
}
