//! NCreate's isolated reuse of the Modrinth App Microsoft/Xbox/Minecraft pipeline.
mod minecraft_auth;
mod vault;

pub use minecraft_auth::MinecraftLoginFlow as LoginFlow;
use serde::Serialize;
use std::sync::LazyLock;
use uuid::Uuid;

pub(crate) static HTTP_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
	reqwest::Client::builder()
		.https_only(true)
		.connect_timeout(std::time::Duration::from_secs(15))
		.timeout(std::time::Duration::from_secs(30))
		.user_agent("NCreate Launcher (https://github.com/Yozekkk/ncreate-launcher)")
		.build()
		.expect("static HTTPS client configuration is valid")
});

#[derive(thiserror::Error, Debug)]
pub enum Error {
	#[error(transparent)]
	Authentication(#[from] minecraft_auth::MinecraftAuthenticationError),
	#[error("secure credential storage is unavailable: {0}")]
	Vault(#[from] keyring::Error),
	#[error("credential storage contains invalid data")]
	Data(#[from] serde_json::Error),
	#[error("secure credential storage task failed")]
	Task(#[from] tokio::task::JoinError),
	#[error("stored Xbox device key is invalid")]
	Key(#[from] p256::pkcs8::Error),
	#[error("{0}")]
	OtherError(String),
}
pub type Result<T> = std::result::Result<T, Error>;

/// Profile metadata safe to send through IPC; credentials never cross this boundary.
#[derive(Debug, Clone, Serialize)]
pub struct MicrosoftProfile {
	pub uuid: Uuid,
	pub nickname: String,
	pub skin_url: Option<String>,
}
impl From<minecraft_auth::MinecraftProfile> for MicrosoftProfile {
	fn from(profile: minecraft_auth::MinecraftProfile) -> Self {
		Self {
			uuid: profile.id,
			nickname: profile.name,
			skin_url: profile
				.skins
				.iter()
				.find(|skin| {
					skin.state == minecraft_auth::MinecraftCharacterExpressionState::Active
				})
				.map(|skin| skin.url.to_string()),
		}
	}
}

#[derive(Default)]
pub struct AuthEngine;
impl AuthEngine {
	pub fn new() -> Self {
		Self
	}
	pub async fn begin(&self) -> Result<LoginFlow> {
		minecraft_auth::login_begin().await
	}
	pub async fn finish(&self, code: &str, flow: LoginFlow) -> Result<MicrosoftProfile> {
		let credentials = minecraft_auth::login_finish(code, flow).await?;
		let profile = credentials.profile().await?;
		vault::save(&profile.id.to_string(), &credentials).await?;
		Ok(profile.into())
	}
	pub async fn refresh(&self, uuid: Uuid) -> Result<MicrosoftProfile> {
		let mut credentials = vault::load::<minecraft_auth::Credentials>(&uuid.to_string())
			.await?
			.ok_or_else(|| {
				Error::OtherError(
					"Microsoft account credentials are absent; sign in again".to_owned(),
				)
			})?;
		credentials.refresh().await?;
		let profile = credentials.profile().await?;
		vault::save(&uuid.to_string(), &credentials).await?;
		Ok(profile.into())
	}
	pub async fn remove(&self, uuid: Uuid) -> Result<()> {
		vault::remove(&uuid.to_string()).await
	}
}
