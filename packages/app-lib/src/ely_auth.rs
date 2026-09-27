//! Ely.by's documented launcher protocol; no password or TOTP persistence.
//! OAuth is optional and requires a separately registered confidential application.
use crate::{AccountProvider, GameIdentity, GameSession, vault};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use url::Url;
use uuid::Uuid;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

const AUTH: &str = "https://authserver.ely.by";
const TOKEN: &str = "https://account.ely.by/api/oauth2/v1/token";
const INFO: &str = "https://account.ely.by/api/account/v1/info";
const SCOPES: &str = "account_info offline_access minecraft_server_session";

#[derive(Debug, thiserror::Error)]
pub enum ElyAuthError {
	#[error("ely_two_factor_required")]
	TwoFactorRequired,
	#[error("ely_invalid_credentials")]
	InvalidCredentials,
	#[error("ely_network")]
	Network,
	#[error("ely_invalid_response")]
	InvalidResponse,
	#[error("ely_secure_storage")]
	SecureStorage,
	#[error("ely_oauth_unconfigured")]
	OAuthUnconfigured,
	#[error("ely_invalid_callback")]
	InvalidCallback,
	#[error("ely_cancelled")]
	Cancelled,
}
type Result<T> = std::result::Result<T, ElyAuthError>;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElyProfile {
	pub uuid: Uuid,
	pub nickname: String,
	pub skin_url: Option<String>,
}

pub struct ElyOAuthConfig {
	client_id: String,
	client_secret: Zeroizing<String>,
	redirect: Url,
}
impl ElyOAuthConfig {
	pub fn new(client_id: String, client_secret: String, redirect: Url) -> Result<Self> {
		let secret = Zeroizing::new(client_secret);
		if client_id.trim().is_empty()
			|| secret.is_empty()
			|| redirect.scheme() != "https"
			|| redirect.host_str().is_none()
			|| !redirect.username().is_empty()
			|| redirect.password().is_some()
			|| redirect.query().is_some()
			|| redirect.fragment().is_some()
		{
			return Err(ElyAuthError::OAuthUnconfigured);
		}
		Ok(Self {
			client_id,
			client_secret: secret,
			redirect,
		})
	}
}
/// OAuth state, code and confidential config never cross IPC.
pub struct ElyOAuthFlow {
	pub auth_request_uri: String,
	state: String,
	redirect: Url,
	created: std::time::Instant,
}
impl ElyOAuthFlow {
	pub fn code_from_redirect(&self, url: &Url) -> Result<Option<String>> {
		if self.created.elapsed() > std::time::Duration::from_secs(600) {
			return Err(ElyAuthError::InvalidCallback);
		}
		if url.origin() != self.redirect.origin() || url.path() != self.redirect.path() {
			return Ok(None);
		}
		if url.username() != self.redirect.username()
			|| url.password().is_some()
			|| url.fragment().is_some()
		{
			return Err(ElyAuthError::InvalidCallback);
		}
		let mut state = url.query_pairs().filter(|(name, _)| name == "state");
		if state.next().map(|(_, value)| value.into_owned()).as_deref() != Some(self.state.as_str())
			|| state.next().is_some()
		{
			return Err(ElyAuthError::InvalidCallback);
		}
		if url.query_pairs().any(|(name, _)| name == "error") {
			return Err(ElyAuthError::Cancelled);
		}
		let codes: Vec<_> = url
			.query_pairs()
			.filter(|(name, _)| name == "code")
			.collect();
		if codes.len() != 1 || codes[0].1.is_empty() {
			return Err(ElyAuthError::InvalidCallback);
		}
		Ok(Some(codes[0].1.to_string()))
	}
}
#[derive(Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
struct ElyCredentials {
	access_token: String,
	refresh_token: Option<String>,
	client_token: String,
	#[zeroize(skip)]
	profile: ElyProfile,
	#[zeroize(skip)]
	expires: DateTime<Utc>,
	#[zeroize(skip)]
	oauth: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyResponse {
	access_token: String,
	client_token: String,
	selected_profile: LegacyProfile,
}
#[derive(Deserialize)]
struct LegacyProfile {
	id: Uuid,
	name: String,
}
#[derive(Deserialize)]
struct OAuthResponse {
	access_token: String,
	refresh_token: Option<String>,
	expires_in: i64,
}
#[derive(Deserialize)]
struct AccountInfo {
	uuid: Uuid,
	username: String,
}

#[derive(Default)]
pub struct ElyAuthEngine {
	oauth: Option<ElyOAuthConfig>,
}
impl ElyAuthEngine {
	pub fn new() -> Self {
		Self::default()
	}
	pub fn with_oauth(config: ElyOAuthConfig) -> Self {
		Self {
			oauth: Some(config),
		}
	}
	pub fn oauth_configured(&self) -> bool {
		self.oauth.is_some()
	}
	pub fn begin_oauth(&self) -> Result<ElyOAuthFlow> {
		let config = self.oauth.as_ref().ok_or(ElyAuthError::OAuthUnconfigured)?;
		let state = Uuid::new_v4().to_string();
		let mut uri = Url::parse("https://account.ely.by/oauth2/v1")
			.map_err(|_| ElyAuthError::OAuthUnconfigured)?;
		uri.query_pairs_mut()
			.append_pair("client_id", &config.client_id)
			.append_pair("redirect_uri", config.redirect.as_str())
			.append_pair("response_type", "code")
			.append_pair("scope", SCOPES)
			.append_pair("state", &state)
			.append_pair("prompt", "select_account");
		Ok(ElyOAuthFlow {
			auth_request_uri: uri.to_string(),
			state,
			redirect: config.redirect.clone(),
			created: std::time::Instant::now(),
		})
	}
	pub async fn finish_oauth(&self, code: &str, flow: ElyOAuthFlow) -> Result<ElyProfile> {
		let config = self.oauth.as_ref().ok_or(ElyAuthError::OAuthUnconfigured)?;
		if flow.redirect != config.redirect
			|| flow.created.elapsed() > std::time::Duration::from_secs(600)
		{
			return Err(ElyAuthError::InvalidCallback);
		}
		let response = crate::transport::post(TOKEN)
			.form(&[
				("client_id", config.client_id.as_str()),
				("client_secret", config.client_secret.as_str()),
				("redirect_uri", config.redirect.as_str()),
				("grant_type", "authorization_code"),
				("code", code),
			])
			.send()
			.await
			.map_err(|_| ElyAuthError::Network)?;
		let token: OAuthResponse = read_response(response).await?;
		let profile = oauth_profile(&token.access_token).await?;
		let credentials = ElyCredentials {
			access_token: token.access_token,
			refresh_token: token.refresh_token,
			client_token: String::new(),
			profile: profile.clone(),
			expires: expires(token.expires_in)?,
			oauth: true,
		};
		save(&credentials).await?;
		Ok(profile)
	}
	/// Strings are consumed and zeroized on every return path. No request body is logged.
	pub async fn authenticate(
		&self,
		username: String,
		password: String,
		totp: Option<String>,
	) -> Result<ElyProfile> {
		let username = Zeroizing::new(username);
		let mut password = Zeroizing::new(password);
		let totp = totp.map(Zeroizing::new);
		if username.is_empty()
			|| username.len() > 320
			|| password.is_empty()
			|| password.len() > 4096
		{
			return Err(ElyAuthError::InvalidCredentials);
		}
		if let Some(totp) = &totp {
			if totp.len() != 6 || !totp.bytes().all(|b| b.is_ascii_digit()) {
				return Err(ElyAuthError::InvalidCredentials);
			}
			password.push(':');
			password.push_str(totp);
		}
		let client_token = Uuid::new_v4().to_string();
		let mut body = json!({"username":username.as_str(),"password":password.as_str(),"clientToken":client_token,"requestUser":false});
		let request = crate::transport::post(format!("{AUTH}/auth/authenticate")).json(&body);
		if let Some(serde_json::Value::String(value)) = body.get_mut("password") {
			value.zeroize();
		}
		let response = request.send().await.map_err(|_| ElyAuthError::Network)?;
		let token: LegacyResponse = read_response(response).await?;
		if token.client_token != client_token {
			return Err(ElyAuthError::InvalidResponse);
		}
		let profile = profile(token.selected_profile)?;
		let credentials = ElyCredentials {
			access_token: token.access_token,
			refresh_token: None,
			client_token,
			profile: profile.clone(),
			expires: Utc::now() + Duration::hours(24),
			oauth: false,
		};
		save(&credentials).await?;
		Ok(profile)
	}
	pub async fn validate(&self, uuid: Uuid) -> Result<bool> {
		let credentials = load(uuid).await?;
		if credentials.oauth {
			return Ok(oauth_profile(&credentials.access_token).await.is_ok());
		}
		let response = crate::transport::post(format!("{AUTH}/auth/validate"))
			.json(&json!({"accessToken":credentials.access_token}))
			.send()
			.await
			.map_err(|_| ElyAuthError::Network)?;
		match response.status().as_u16() {
			200 | 204 => Ok(true),
			400 | 401 | 403 => Ok(false),
			_ => Err(ElyAuthError::Network),
		}
	}
	pub async fn refresh(&self, uuid: Uuid) -> Result<ElyProfile> {
		let mut credentials = load(uuid).await?;
		if credentials.oauth {
			let config = self.oauth.as_ref().ok_or(ElyAuthError::OAuthUnconfigured)?;
			let refresh = credentials
				.refresh_token
				.as_deref()
				.ok_or(ElyAuthError::InvalidCredentials)?;
			let response = crate::transport::post(TOKEN)
				.form(&[
					("client_id", config.client_id.as_str()),
					("client_secret", config.client_secret.as_str()),
					("scope", SCOPES),
					("refresh_token", refresh),
					("grant_type", "refresh_token"),
				])
				.send()
				.await
				.map_err(|_| ElyAuthError::Network)?;
			let token: OAuthResponse = read_response(response).await?;
			credentials.access_token.zeroize();
			credentials.access_token = token.access_token;
			if let Some(refresh) = token.refresh_token {
				credentials.refresh_token.zeroize();
				credentials.refresh_token = Some(refresh);
			}
			credentials.expires = expires(token.expires_in)?;
			save(&credentials).await?;
			credentials.profile = oauth_profile(&credentials.access_token).await?;
		} else {
			let response = crate::transport::post(format!("{AUTH}/auth/refresh")).json(&json!({"accessToken":credentials.access_token,"clientToken":credentials.client_token,"requestUser":false})).send().await.map_err(|_|ElyAuthError::Network)?;
			let token: LegacyResponse = read_response(response).await?;
			if token.client_token != credentials.client_token {
				return Err(ElyAuthError::InvalidResponse);
			}
			let profile = profile(token.selected_profile)?;
			if profile.uuid != uuid {
				return Err(ElyAuthError::InvalidResponse);
			}
			credentials.access_token.zeroize();
			credentials.access_token = token.access_token;
			credentials.profile = profile;
			credentials.expires = Utc::now() + Duration::hours(24);
		}
		if credentials.profile.uuid != uuid {
			return Err(ElyAuthError::InvalidResponse);
		}
		save(&credentials).await?;
		Ok(credentials.profile.clone())
	}
	pub async fn session(&self, uuid: Uuid) -> Result<GameSession> {
		let credentials = load(uuid).await?;
		if credentials.expires <= Utc::now() + Duration::minutes(5) || !self.validate(uuid).await? {
			self.refresh(uuid).await?;
		}
		let credentials = load(uuid).await?;
		Ok(GameSession::new(
			GameIdentity {
				uuid,
				nickname: credentials.profile.nickname.clone(),
			},
			AccountProvider::ElyBy,
			credentials.access_token.clone(),
		))
	}
	/// Best-effort remote invalidation; a network outage never prevents local credential deletion.
	pub async fn remove(&self, uuid: Uuid) -> Result<()> {
		if let Ok(credentials) = load(uuid).await
			&& !credentials.oauth
		{
			let _ = crate::transport::post(format!("{AUTH}/auth/invalidate"))
				.json(
					&json!({"accessToken":credentials.access_token,"clientToken":credentials.client_token}),
				)
				.send()
				.await;
		}
		vault::remove(&key(uuid))
			.await
			.map_err(|_| ElyAuthError::SecureStorage)
	}
}
fn key(uuid: Uuid) -> String {
	format!("ely_by:{uuid}")
}
async fn load(uuid: Uuid) -> Result<ElyCredentials> {
	vault::load(&key(uuid))
		.await
		.map_err(|_| ElyAuthError::SecureStorage)?
		.ok_or(ElyAuthError::InvalidCredentials)
}
async fn save(credentials: &ElyCredentials) -> Result<()> {
	if credentials.access_token.is_empty() {
		return Err(ElyAuthError::InvalidResponse);
	}
	vault::save(&key(credentials.profile.uuid), credentials)
		.await
		.map_err(|_| ElyAuthError::SecureStorage)
}
fn expires(seconds: i64) -> Result<DateTime<Utc>> {
	if !(1..=604800).contains(&seconds) {
		return Err(ElyAuthError::InvalidResponse);
	}
	Ok(Utc::now() + Duration::seconds(seconds))
}
fn profile(profile: LegacyProfile) -> Result<ElyProfile> {
	if profile.id.is_nil()
		|| profile.name.is_empty()
		|| profile.name.len() > 16
		|| !profile
			.name
			.bytes()
			.all(|b| b.is_ascii_alphanumeric() || b == b'_')
	{
		return Err(ElyAuthError::InvalidResponse);
	}
	Ok(ElyProfile {
		uuid: profile.id,
		nickname: profile.name,
		skin_url: None,
	})
}
async fn oauth_profile(token: &str) -> Result<ElyProfile> {
	let response = crate::transport::get(INFO)
		.bearer_auth(token)
		.send()
		.await
		.map_err(|_| ElyAuthError::Network)?;
	let info: AccountInfo = read_response(response).await?;
	profile(LegacyProfile {
		id: info.uuid,
		name: info.username,
	})
}
async fn read_response<T: serde::de::DeserializeOwned>(
	mut response: reqwest::Response,
) -> Result<T> {
	let status = response.status();
	if response.content_length().is_some_and(|n| n > 262144) {
		return Err(ElyAuthError::InvalidResponse);
	}
	let mut bytes = Zeroizing::new(Vec::new());
	while let Some(chunk) = response.chunk().await.map_err(|_| ElyAuthError::Network)? {
		if bytes.len() + chunk.len() > 262144 {
			return Err(ElyAuthError::InvalidResponse);
		}
		bytes.extend_from_slice(&chunk);
	}
	let text = std::str::from_utf8(&bytes).map_err(|_| ElyAuthError::InvalidResponse)?;
	if !status.is_success() {
		if status.as_u16() == 401
			&& serde_json::from_str::<serde_json::Value>(text)
				.ok()
				.and_then(|v| {
					v.get("errorMessage")
						.and_then(|v| v.as_str())
						.map(str::to_owned)
				})
				.as_deref() == Some("Account protected with two factor auth.")
		{
			return Err(ElyAuthError::TwoFactorRequired);
		}
		return Err(if status.is_server_error() {
			ElyAuthError::Network
		} else {
			ElyAuthError::InvalidCredentials
		});
	}
	serde_json::from_str(text).map_err(|_| ElyAuthError::InvalidResponse)
}
