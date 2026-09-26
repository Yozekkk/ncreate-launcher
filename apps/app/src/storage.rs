//! NCreate metadata store. Uses the upstream SQLite/settings architecture in an isolated directory.
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool, sqlite::SqliteConnectOptions};
use std::path::PathBuf;
use uuid::Uuid;

pub type Result<T> = std::result::Result<T, String>;

#[derive(Serialize, Deserialize, Clone, FromRow)]
pub struct Account {
	pub uuid: String,
	pub nickname: String,
	pub kind: String,
	pub active: bool,
	pub skin_provider: String,
	#[serde(skip)]
	pub skin_url: Option<String>,
}
#[derive(Serialize, Deserialize, Clone)]
pub struct Settings {
	pub locale: String,
	pub theme: String,
	pub animations: bool,
	pub blur: bool,
	pub reduced_motion: bool,
	pub memory_mb: u32,
	pub java_path: String,
	pub game_directory: String,
	pub auto_updates: bool,
}
impl Default for Settings {
	fn default() -> Self {
		Self {
			locale: "ru".into(),
			theme: "dark".into(),
			animations: true,
			blur: true,
			reduced_motion: false,
			memory_mb: 4096,
			java_path: String::new(),
			game_directory: String::new(),
			auto_updates: false,
		}
	}
}
#[derive(Serialize)]
pub struct Snapshot {
	pub accounts: Vec<Account>,
	pub settings: Settings,
	pub data_dir: String,
}
pub struct Store {
	pub pool: SqlitePool,
	pub directory: PathBuf,
}
impl Store {
	pub async fn new() -> Result<Self> {
		let directory = dirs::data_local_dir()
			.ok_or("Не удалось найти каталог данных")?
			.join("ncreate-launcher");
		Self::open(directory).await
	}
	pub async fn open(directory: PathBuf) -> Result<Self> {
		tokio::fs::create_dir_all(&directory)
			.await
			.map_err(db_error)?;
		#[cfg(unix)]
		{
			use std::os::unix::fs::PermissionsExt;
			tokio::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))
				.await
				.map_err(db_error)?;
		}
		let options = SqliteConnectOptions::new()
			.filename(directory.join("launcher.db"))
			.create_if_missing(true)
			.journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
			.busy_timeout(std::time::Duration::from_secs(5));
		let pool = sqlx::sqlite::SqlitePoolOptions::new()
			.max_connections(1)
			.connect_with(options)
			.await
			.map_err(db_error)?;
		sqlx::query("CREATE TABLE IF NOT EXISTS accounts (uuid TEXT PRIMARY KEY, nickname TEXT NOT NULL, kind TEXT NOT NULL CHECK(kind IN ('offline','microsoft')), active BOOLEAN NOT NULL DEFAULT 0, skin_provider TEXT NOT NULL, skin_url TEXT)").execute(&pool).await.map_err(db_error)?;
		sqlx::query(
			"CREATE UNIQUE INDEX IF NOT EXISTS one_active_account ON accounts(active) WHERE active = 1",
		)
		.execute(&pool)
		.await
		.map_err(db_error)?;
		sqlx::query("CREATE TABLE IF NOT EXISTS settings (id INTEGER PRIMARY KEY CHECK(id = 0), value TEXT NOT NULL)").execute(&pool).await.map_err(db_error)?;
		Ok(Self { pool, directory })
	}
	pub async fn account(&self, uuid: &str) -> Result<Account> {
		sqlx::query_as("SELECT * FROM accounts WHERE uuid = ?")
			.bind(uuid)
			.fetch_optional(&self.pool)
			.await
			.map_err(db_error)?
			.ok_or("Аккаунт не найден".into())
	}
	pub async fn snapshot(&self) -> Result<Snapshot> {
		let accounts =
			sqlx::query_as("SELECT * FROM accounts ORDER BY active DESC, nickname COLLATE NOCASE")
				.fetch_all(&self.pool)
				.await
				.map_err(db_error)?;
		let json: Option<String> = sqlx::query_scalar("SELECT value FROM settings WHERE id = 0")
			.fetch_optional(&self.pool)
			.await
			.map_err(db_error)?;
		let mut settings: Settings = match json {
			Some(json) => serde_json::from_str(&json).map_err(db_error)?,
			None => Settings::default(),
		};
		if settings.game_directory.is_empty() {
			settings.game_directory = self.directory.join("games").display().to_string();
		}
		Ok(Snapshot {
			accounts,
			settings,
			data_dir: self.directory.display().to_string(),
		})
	}
	pub async fn add_offline(&self, nickname: &str) -> Result<Snapshot> {
		validate_nickname(nickname)?;
		let uuid = offline_uuid(nickname).to_string();
		let mut tx = self.pool.begin().await.map_err(db_error)?;
		let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM accounts")
			.fetch_one(&mut *tx)
			.await
			.map_err(db_error)?;
		let duplicate: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM accounts WHERE uuid = ?")
			.bind(&uuid)
			.fetch_one(&mut *tx)
			.await
			.map_err(db_error)?;
		if duplicate > 0 {
			return Err("Этот офлайн аккаунт уже добавлен".into());
		}
		sqlx::query("INSERT INTO accounts(uuid,nickname,kind,active,skin_provider) VALUES(?,?,'offline',?,'fallback')").bind(&uuid).bind(nickname).bind(count == 0).execute(&mut *tx).await.map_err(db_error)?;
		tx.commit().await.map_err(db_error)?;
		self.snapshot().await
	}
	pub async fn rename(&self, uuid: &str, nickname: &str) -> Result<Snapshot> {
		validate_nickname(nickname)?;
		let account = self.account(uuid).await?;
		if account.kind != "offline" {
			return Err("Имя Microsoft меняется в Minecraft-профиле".into());
		}
		let new_uuid = offline_uuid(nickname).to_string();
		if new_uuid != uuid
			&& sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM accounts WHERE uuid = ?")
				.bind(&new_uuid)
				.fetch_one(&self.pool)
				.await
				.map_err(db_error)?
				> 0
		{
			return Err("Этот офлайн аккаунт уже добавлен".into());
		}
		sqlx::query("UPDATE accounts SET uuid = ?, nickname = ?, skin_provider = 'fallback', skin_url = NULL WHERE uuid = ?").bind(new_uuid).bind(nickname).bind(uuid).execute(&self.pool).await.map_err(db_error)?;
		self.snapshot().await
	}
	pub async fn activate(&self, uuid: &str) -> Result<Snapshot> {
		self.account(uuid).await?;
		let mut tx = self.pool.begin().await.map_err(db_error)?;
		sqlx::query("UPDATE accounts SET active = 0")
			.execute(&mut *tx)
			.await
			.map_err(db_error)?;
		let selected = sqlx::query("UPDATE accounts SET active = 1 WHERE uuid = ?")
			.bind(uuid)
			.execute(&mut *tx)
			.await
			.map_err(db_error)?;
		if selected.rows_affected() != 1 {
			return Err("Аккаунт не найден".into());
		}
		tx.commit().await.map_err(db_error)?;
		self.snapshot().await
	}
	pub async fn remove(&self, uuid: &str) -> Result<Snapshot> {
		let mut tx = self.pool.begin().await.map_err(db_error)?;
		sqlx::query("DELETE FROM accounts WHERE uuid = ?")
			.bind(uuid)
			.execute(&mut *tx)
			.await
			.map_err(db_error)?;
		sqlx::query("UPDATE accounts SET active = 1 WHERE uuid = (SELECT uuid FROM accounts ORDER BY nickname LIMIT 1) AND NOT EXISTS (SELECT 1 FROM accounts WHERE active = 1)").execute(&mut *tx).await.map_err(db_error)?;
		tx.commit().await.map_err(db_error)?;
		self.snapshot().await
	}
	pub async fn microsoft(
		&self,
		profile: ncreate_app_lib::MicrosoftProfile,
		make_active: bool,
	) -> Result<Snapshot> {
		let mut tx = self.pool.begin().await.map_err(db_error)?;
		if make_active {
			sqlx::query("UPDATE accounts SET active = 0")
				.execute(&mut *tx)
				.await
				.map_err(db_error)?;
		}
		sqlx::query("INSERT INTO accounts(uuid,nickname,kind,active,skin_provider,skin_url) VALUES(?,?,'microsoft',?,'mojang',?) ON CONFLICT(uuid) DO UPDATE SET nickname = excluded.nickname, skin_url = excluded.skin_url, active = CASE WHEN excluded.active THEN 1 ELSE accounts.active END")
			.bind(profile.uuid.to_string()).bind(profile.nickname).bind(make_active).bind(profile.skin_url).execute(&mut *tx).await.map_err(db_error)?;
		tx.commit().await.map_err(db_error)?;
		self.snapshot().await
	}
	pub async fn settings(&self, mut settings: Settings) -> Result<Snapshot> {
		if settings.locale != "ru"
			|| !["dark", "oled"].contains(&settings.theme.as_str())
			|| !(512..=32768).contains(&settings.memory_mb)
			|| settings.java_path.len() > 4096
			|| settings.game_directory.len() > 4096
		{
			return Err("Проверьте значения настроек".into());
		}
		settings.auto_updates = false;
		let json = serde_json::to_string(&settings).map_err(db_error)?;
		sqlx::query("INSERT INTO settings(id,value) VALUES(0,?) ON CONFLICT(id) DO UPDATE SET value = excluded.value").bind(json).execute(&self.pool).await.map_err(db_error)?;
		self.snapshot().await
	}
}
pub fn db_error(_: impl std::fmt::Display) -> String {
	"Не удалось прочитать или сохранить локальные данные. Проверьте доступ к каталогу NCreate Launcher.".into()
}
pub fn validate_nickname(nickname: &str) -> Result<()> {
	if !(3..=16).contains(&nickname.len())
		|| !nickname
			.bytes()
			.all(|c| c.is_ascii_alphanumeric() || c == b'_')
	{
		return Err("Никнейм: 3–16 латинских букв, цифр или символов _".into());
	}
	Ok(())
}
/// Matches Java UUID.nameUUIDFromBytes("OfflinePlayer:" + nickname), case-sensitive.
pub fn offline_uuid(nickname: &str) -> Uuid {
	let mut bytes = md5::compute(format!("OfflinePlayer:{nickname}")).0;
	bytes[6] = (bytes[6] & 0x0f) | 0x30;
	bytes[8] = (bytes[8] & 0x3f) | 0x80;
	Uuid::from_bytes(bytes)
}
#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn matches_java_offline_uuid() {
		assert_eq!(
			offline_uuid("Notch").to_string(),
			"b50ad385-829d-3141-a216-7e7d7539ba7f"
		);
		assert_ne!(offline_uuid("Notch"), offline_uuid("notch"));
	}
	#[tokio::test]
	async fn persistence_and_single_active_account() {
		let directory = std::env::temp_dir().join(format!("ncreate-test-{}", Uuid::new_v4()));
		let store = Store::open(directory.clone()).await.expect("open test db");
		store.add_offline("NCreateQA").await.expect("add");
		store.add_offline("SecondQA").await.expect("add second");
		assert!(store.add_offline("NCreateQA").await.is_err());
		store
			.activate(&offline_uuid("SecondQA").to_string())
			.await
			.expect("activate");
		store
			.rename(&offline_uuid("SecondQA").to_string(), "RenamedQA")
			.await
			.expect("rename");
		store.pool.close().await;
		let reopened = Store::open(directory.clone()).await.expect("reopen");
		let snapshot = reopened.snapshot().await.expect("snapshot");
		assert_eq!(snapshot.accounts.len(), 2);
		assert_eq!(snapshot.accounts.iter().filter(|a| a.active).count(), 1);
		assert!(snapshot.accounts.iter().any(|a| a.active
			&& a.nickname == "RenamedQA"
			&& a.uuid == offline_uuid("RenamedQA").to_string()));
		reopened
			.remove(&offline_uuid("RenamedQA").to_string())
			.await
			.expect("remove");
		assert!(reopened.snapshot().await.expect("snapshot").accounts[0].active);
		reopened.pool.close().await;
		tokio::fs::remove_dir_all(directory).await.expect("cleanup");
	}
}
