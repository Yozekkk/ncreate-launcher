//! NCreate metadata store. Uses the upstream SQLite/settings architecture in an isolated directory.
use ncreate_app_lib::{AccountProvider, CredentialReference, GameIdentity};
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
	#[sqlx(skip)]
	pub account_provider: AccountProvider,
	#[sqlx(skip)]
	pub game_identity: GameIdentity,
	#[sqlx(skip)]
	pub credential_reference: Option<CredentialReference>,
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
	#[serde(default = "stable_channel")]
	pub release_channel: String,
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
			release_channel: stable_channel(),
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
		#[cfg(debug_assertions)]
		if let Some(directory) = std::env::var_os("NCREATE_TEST_DATA_DIR") {
			let directory = PathBuf::from(directory);
			if !directory.is_absolute()
				|| !directory
					.file_name()
					.and_then(|name| name.to_str())
					.is_some_and(|name| name.starts_with("ncreate"))
			{
				return Err("Тестовый каталог должен быть абсолютным каталогом NCreate".into());
			}
			return Self::open(directory).await;
		}
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
		migrate(&pool, &directory).await?;
		Ok(Self { pool, directory })
	}
	pub async fn account(&self, uuid: &str) -> Result<Account> {
		let account = sqlx::query_as("SELECT * FROM accounts WHERE uuid = ?")
			.bind(uuid)
			.fetch_optional(&self.pool)
			.await
			.map_err(db_error)?
			.ok_or("Аккаунт не найден".to_string())?;
		hydrate(account)
	}
	pub async fn snapshot(&self) -> Result<Snapshot> {
		let accounts: Vec<Account> =
			sqlx::query_as("SELECT * FROM accounts ORDER BY active DESC, nickname COLLATE NOCASE")
				.fetch_all(&self.pool)
				.await
				.map_err(db_error)?;
		let accounts = accounts
			.into_iter()
			.map(hydrate)
			.collect::<Result<Vec<_>>>()?;
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
		sqlx::query("INSERT INTO accounts(uuid,nickname,kind,active,skin_provider,account_provider) VALUES(?,?,'offline',?,'fallback','offline')").bind(&uuid).bind(nickname).bind(count == 0).execute(&mut *tx).await.map_err(db_error)?;
		tx.commit().await.map_err(db_error)?;
		self.snapshot().await
	}
	pub async fn rename(&self, uuid: &str, nickname: &str) -> Result<Snapshot> {
		validate_nickname(nickname)?;
		let new_uuid = offline_uuid(nickname).to_string();
		let mut tx = self.pool.begin().await.map_err(db_error)?;
		let kind: Option<String> = sqlx::query_scalar("SELECT kind FROM accounts WHERE uuid = ?")
			.bind(uuid)
			.fetch_optional(&mut *tx)
			.await
			.map_err(db_error)?;
		match kind.as_deref() {
			Some("offline") => (),
			Some(_) => return Err("Имя меняется у провайдера аккаунта".into()),
			None => return Err("Аккаунт не найден".into()),
		}
		if new_uuid != uuid
			&& sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM accounts WHERE uuid = ?")
				.bind(&new_uuid)
				.fetch_one(&mut *tx)
				.await
				.map_err(db_error)?
				> 0
		{
			return Err("Этот офлайн аккаунт уже добавлен".into());
		}
		sqlx::query("UPDATE accounts SET uuid = ?, nickname = ?, skin_provider = 'fallback', skin_url = NULL WHERE uuid = ?").bind(new_uuid).bind(nickname).bind(uuid).execute(&mut *tx).await.map_err(db_error)?;
		tx.commit().await.map_err(db_error)?;
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
		let existing: Option<String> =
			sqlx::query_scalar("SELECT kind FROM accounts WHERE uuid = ?")
				.bind(profile.uuid.to_string())
				.fetch_optional(&mut *tx)
				.await
				.map_err(db_error)?;
		if existing.is_some_and(|kind| kind != "microsoft") {
			return Err("Этот UUID уже принадлежит аккаунту другого типа".into());
		}
		if make_active {
			sqlx::query("UPDATE accounts SET active = 0")
				.execute(&mut *tx)
				.await
				.map_err(db_error)?;
		}
		sqlx::query("INSERT INTO accounts(uuid,nickname,kind,active,skin_provider,skin_url,account_provider,credential_reference) VALUES(?,?,'microsoft',?,'mojang',?,'microsoft',?) ON CONFLICT(uuid) DO UPDATE SET nickname = excluded.nickname, skin_url = excluded.skin_url, account_provider = excluded.account_provider, credential_reference = excluded.credential_reference, kind = excluded.kind, active = CASE WHEN excluded.active THEN 1 ELSE accounts.active END")
			.bind(profile.uuid.to_string()).bind(profile.nickname).bind(make_active).bind(profile.skin_url).bind(serde_json::to_string(&CredentialReference::new(AccountProvider::Microsoft,profile.uuid)).map_err(db_error)?).execute(&mut *tx).await.map_err(db_error)?;
		tx.commit().await.map_err(db_error)?;
		self.snapshot().await
	}
	pub async fn ely_by(
		&self,
		profile: ncreate_app_lib::ElyProfile,
		make_active: bool,
	) -> Result<Snapshot> {
		let mut tx = self.pool.begin().await.map_err(db_error)?;
		let existing: Option<String> =
			sqlx::query_scalar("SELECT kind FROM accounts WHERE uuid = ?")
				.bind(profile.uuid.to_string())
				.fetch_optional(&mut *tx)
				.await
				.map_err(db_error)?;
		if existing.is_some_and(|kind| kind != "ely_by") {
			return Err("Этот UUID уже принадлежит аккаунту другого типа".into());
		}
		if make_active {
			sqlx::query("UPDATE accounts SET active = 0")
				.execute(&mut *tx)
				.await
				.map_err(db_error)?;
		}
		sqlx::query("INSERT INTO accounts(uuid,nickname,kind,active,skin_provider,skin_url,account_provider,credential_reference) VALUES(?,?,'ely_by',?,'ely_by',?,'ely_by',?) ON CONFLICT(uuid) DO UPDATE SET nickname = excluded.nickname, kind = excluded.kind, account_provider = excluded.account_provider, credential_reference = excluded.credential_reference, skin_url = excluded.skin_url, skin_provider = excluded.skin_provider, active = CASE WHEN excluded.active THEN 1 ELSE accounts.active END")
            .bind(profile.uuid.to_string()).bind(profile.nickname).bind(make_active).bind(profile.skin_url)
            .bind(serde_json::to_string(&CredentialReference::new(AccountProvider::ElyBy,profile.uuid)).map_err(db_error)?).execute(&mut *tx).await.map_err(db_error)?;
		tx.commit().await.map_err(db_error)?;
		self.snapshot().await
	}
	pub async fn settings(&self, mut settings: Settings) -> Result<Snapshot> {
		if !["stable", "beta"].contains(&settings.release_channel.as_str())
			|| settings.locale != "ru"
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
fn stable_channel() -> String {
	"stable".into()
}
fn hydrate(mut account: Account) -> Result<Account> {
	let uuid = Uuid::parse_str(&account.uuid).map_err(db_error)?;
	account.account_provider = match account.kind.as_str() {
		"offline" => AccountProvider::Offline,
		"microsoft" => AccountProvider::Microsoft,
		"ely_by" => AccountProvider::ElyBy,
		_ => return Err("Неизвестный провайдер аккаунта".into()),
	};
	account.game_identity = GameIdentity {
		uuid,
		nickname: account.nickname.clone(),
	};
	account.credential_reference = CredentialReference::new(account.account_provider, uuid);
	Ok(account)
}
async fn migrate(pool: &SqlitePool, directory: &std::path::Path) -> Result<()> {
	let version: i64 = sqlx::query_scalar("PRAGMA user_version")
		.fetch_one(pool)
		.await
		.map_err(db_error)?;
	if version >= 2 {
		return Ok(());
	}
	let existing: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'accounts'",
	)
	.fetch_one(pool)
	.await
	.map_err(db_error)?;
	if existing > 0 {
		let backup_dir = directory.join("backups");
		tokio::fs::create_dir_all(&backup_dir)
			.await
			.map_err(db_error)?;
		let backup = backup_dir.join(format!("launcher-v0.1-{}.db", Uuid::new_v4()));
		sqlx::query("VACUUM INTO ?")
			.bind(backup.display().to_string())
			.execute(pool)
			.await
			.map_err(db_error)?;
		#[cfg(unix)]
		{
			use std::os::unix::fs::PermissionsExt;
			tokio::fs::set_permissions(&backup, std::fs::Permissions::from_mode(0o600))
				.await
				.map_err(db_error)?;
		}
	}
	let mut tx = pool.begin().await.map_err(db_error)?;
	sqlx::query("CREATE TABLE accounts_v2 (uuid TEXT PRIMARY KEY, nickname TEXT NOT NULL, kind TEXT NOT NULL CHECK(kind IN ('offline','microsoft','ely_by')), active BOOLEAN NOT NULL DEFAULT 0, skin_provider TEXT NOT NULL, skin_url TEXT, account_provider TEXT NOT NULL CHECK(account_provider IN ('offline','microsoft','ely_by')), credential_reference TEXT)").execute(&mut *tx).await.map_err(db_error)?;
	if existing > 0 {
		sqlx::query("INSERT INTO accounts_v2 (uuid,nickname,kind,active,skin_provider,skin_url,account_provider,credential_reference) SELECT uuid,nickname,kind,active,skin_provider,skin_url,kind,CASE WHEN kind='microsoft' THEN json_object('provider','microsoft','key',uuid) ELSE NULL END FROM accounts").execute(&mut *tx).await.map_err(db_error)?;
		sqlx::query("DROP TABLE accounts")
			.execute(&mut *tx)
			.await
			.map_err(db_error)?;
	}
	sqlx::query("ALTER TABLE accounts_v2 RENAME TO accounts")
		.execute(&mut *tx)
		.await
		.map_err(db_error)?;
	sqlx::query("CREATE UNIQUE INDEX one_active_account ON accounts(active) WHERE active=1")
		.execute(&mut *tx)
		.await
		.map_err(db_error)?;
	sqlx::query(
		"CREATE TABLE IF NOT EXISTS settings (id INTEGER PRIMARY KEY CHECK(id=0),value TEXT NOT NULL)",
	)
	.execute(&mut *tx)
	.await
	.map_err(db_error)?;
	sqlx::query("PRAGMA user_version=2")
		.execute(&mut *tx)
		.await
		.map_err(db_error)?;
	tx.commit().await.map_err(db_error)?;
	Ok(())
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
	async fn legacy_fixture(directory: &std::path::Path, duplicate_active: bool) -> SqlitePool {
		tokio::fs::create_dir_all(directory)
			.await
			.expect("fixture directory");
		let options = SqliteConnectOptions::new()
			.filename(directory.join("launcher.db"))
			.create_if_missing(true);
		let pool = sqlx::sqlite::SqlitePoolOptions::new()
			.max_connections(1)
			.connect_with(options)
			.await
			.expect("legacy fixture connection");
		sqlx::query("CREATE TABLE accounts (uuid TEXT PRIMARY KEY,nickname TEXT NOT NULL,kind TEXT NOT NULL CHECK(kind IN ('offline','microsoft')),active BOOLEAN NOT NULL DEFAULT 0,skin_provider TEXT NOT NULL,skin_url TEXT)").execute(&pool).await.expect("legacy accounts");
		sqlx::query("CREATE TABLE settings (id INTEGER PRIMARY KEY,value TEXT NOT NULL)")
			.execute(&pool)
			.await
			.expect("legacy settings");
		sqlx::query("INSERT INTO accounts VALUES (?, 'LegacyOffline','offline',?,'ely_by',NULL)")
			.bind(offline_uuid("LegacyOffline").to_string())
			.bind(duplicate_active)
			.execute(&pool)
			.await
			.expect("offline fixture");
		sqlx::query("INSERT INTO accounts VALUES ('12345678-1234-4234-9234-123456789abc','LegacyMicrosoft','microsoft',1,'mojang','https://textures.minecraft.net/texture/synthetic')").execute(&pool).await.expect("Microsoft fixture");
		let settings = r#"{"locale":"ru","theme":"oled","animations":false,"blur":false,"reduced_motion":true,"memory_mb":8192,"java_path":"/custom/java","game_directory":"/custom/game","auto_updates":false}"#;
		sqlx::query("INSERT INTO settings VALUES(0,?)")
			.bind(settings)
			.execute(&pool)
			.await
			.expect("settings fixture");
		pool
	}
	#[tokio::test]
	async fn v01_migration_backs_up_preserves_metadata_settings_and_future_engine_version() {
		let directory = std::env::temp_dir().join(format!("ncreate-migration-{}", Uuid::new_v4()));
		legacy_fixture(&directory, false).await.close().await;
		let store = Store::open(directory.clone())
			.await
			.expect("migrate original v0.1");
		let snapshot = store.snapshot().await.expect("migrated snapshot");
		assert_eq!(snapshot.accounts.len(), 2);
		assert_eq!(snapshot.settings.memory_mb, 8192);
		assert_eq!(snapshot.settings.java_path, "/custom/java");
		assert_eq!(snapshot.settings.theme, "oled");
		assert_eq!(snapshot.settings.release_channel, "stable");
		let microsoft = snapshot
			.accounts
			.iter()
			.find(|a| a.kind == "microsoft")
			.expect("Microsoft retained");
		assert!(microsoft.active);
		assert_eq!(microsoft.account_provider, AccountProvider::Microsoft);
		assert_eq!(microsoft.game_identity.uuid.to_string(), microsoft.uuid);
		assert_eq!(
			microsoft
				.credential_reference
				.as_ref()
				.expect("vault reference")
				.key,
			microsoft.uuid
		);
		let offline = snapshot
			.accounts
			.iter()
			.find(|a| a.kind == "offline")
			.expect("offline retained");
		assert!(offline.credential_reference.is_none());
		assert_eq!(offline.skin_provider, "ely_by");
		let mut backups = tokio::fs::read_dir(directory.join("backups"))
			.await
			.expect("backup directory");
		let backup = backups
			.next_entry()
			.await
			.expect("backup entry")
			.expect("backup exists")
			.path();
		let backup_pool = sqlx::sqlite::SqlitePoolOptions::new()
			.max_connections(1)
			.connect_with(
				SqliteConnectOptions::new()
					.filename(&backup)
					.read_only(true),
			)
			.await
			.expect("backup database readable");
		assert_eq!(
			sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM accounts")
				.fetch_one(&backup_pool)
				.await
				.expect("backup rows"),
			2
		);
		assert_eq!(
			sqlx::query_scalar::<_, i64>("PRAGMA user_version")
				.fetch_one(&backup_pool)
				.await
				.expect("backup schema"),
			0
		);
		backup_pool.close().await;
		let saved = store
			.ely_by(
				ncreate_app_lib::ElyProfile {
					uuid: Uuid::new_v4(),
					nickname: "ElyRealProfile".into(),
					skin_url: None,
				},
				true,
			)
			.await
			.expect("Ely metadata save");
		assert_eq!(saved.accounts.iter().filter(|a| a.active).count(), 1);
		assert_eq!(
			saved
				.accounts
				.iter()
				.find(|a| a.active)
				.expect("active Ely")
				.kind,
			"ely_by"
		);
		sqlx::query("PRAGMA user_version=3")
			.execute(&store.pool)
			.await
			.expect("engine migration version");
		store.pool.close().await;
		let reopened = Store::open(directory.clone())
			.await
			.expect("future engine schema still readable");
		assert_eq!(
			sqlx::query_scalar::<_, i64>("PRAGMA user_version")
				.fetch_one(&reopened.pool)
				.await
				.expect("version retained"),
			3
		);
		assert_eq!(
			reopened
				.snapshot()
				.await
				.expect("accounts preserved")
				.accounts
				.len(),
			3
		);
		reopened.pool.close().await;
		tokio::fs::remove_dir_all(directory)
			.await
			.expect("cleanup migration");
	}
	#[tokio::test]
	async fn migration_error_rolls_back_every_schema_change() {
		let directory =
			std::env::temp_dir().join(format!("ncreate-migration-rollback-{}", Uuid::new_v4()));
		legacy_fixture(&directory, true).await.close().await;
		assert!(Store::open(directory.clone()).await.is_err());
		let pool = sqlx::sqlite::SqlitePoolOptions::new()
			.max_connections(1)
			.connect_with(SqliteConnectOptions::new().filename(directory.join("launcher.db")))
			.await
			.expect("original database after rollback");
		assert_eq!(
			sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM accounts")
				.fetch_one(&pool)
				.await
				.expect("original rows"),
			2
		);
		assert_eq!(
			sqlx::query_scalar::<_, i64>("PRAGMA user_version")
				.fetch_one(&pool)
				.await
				.expect("original schema version"),
			0
		);
		assert_eq!(
			sqlx::query_scalar::<_, i64>(
				"SELECT COUNT(*) FROM pragma_table_info('accounts') WHERE name='account_provider'"
			)
			.fetch_one(&pool)
			.await
			.expect("original columns"),
			0
		);
		pool.close().await;
		tokio::fs::remove_dir_all(directory)
			.await
			.expect("cleanup rollback");
	}
	#[tokio::test]
	async fn invalid_activation_rolls_back_and_release_channel_validation_is_persisted() {
		let directory =
			std::env::temp_dir().join(format!("ncreate-active-test-{}", Uuid::new_v4()));
		let store = Store::open(directory.clone()).await.expect("store");
		store.add_offline("ActiveUser").await.expect("account");
		assert!(store.activate(&Uuid::new_v4().to_string()).await.is_err());
		assert!(store.snapshot().await.expect("snapshot").accounts[0].active);
		let mut settings = Settings {
			release_channel: "beta".into(),
			..Settings::default()
		};
		assert_eq!(
			store
				.settings(settings.clone())
				.await
				.expect("beta settings")
				.settings
				.release_channel,
			"beta"
		);
		settings.release_channel = "arbitrary".into();
		assert!(store.settings(settings).await.is_err());
		store.pool.close().await;
		tokio::fs::remove_dir_all(directory).await.expect("cleanup");
	}
	#[tokio::test]
	async fn authenticated_provider_cannot_overwrite_another_provider_identity() {
		let directory =
			std::env::temp_dir().join(format!("ncreate-provider-test-{}", Uuid::new_v4()));
		let store = Store::open(directory.clone()).await.expect("store");
		let account = store
			.add_offline("IdentityUser")
			.await
			.expect("offline")
			.accounts
			.remove(0);
		let uuid = Uuid::parse_str(&account.uuid).expect("uuid");
		assert!(
			store
				.ely_by(
					ncreate_app_lib::ElyProfile {
						uuid,
						nickname: "OtherUser".into(),
						skin_url: None
					},
					true
				)
				.await
				.is_err()
		);
		assert!(
			store
				.microsoft(
					ncreate_app_lib::MicrosoftProfile {
						uuid,
						nickname: "OtherUser".into(),
						skin_url: None
					},
					true
				)
				.await
				.is_err()
		);
		let retained = store
			.account(&account.uuid)
			.await
			.expect("unchanged identity");
		assert_eq!(retained.kind, "offline");
		assert_eq!(retained.nickname, "IdentityUser");
		assert!(retained.active);
		store.pool.close().await;
		tokio::fs::remove_dir_all(directory).await.expect("cleanup");
	}
}
