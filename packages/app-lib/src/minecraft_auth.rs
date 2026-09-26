//! Adapted from Modrinth App main, packages/app-lib/src/state/minecraft_auth.rs.
//! Copyright remains with upstream contributors; GPL-3.0-only. Changes: isolated vault
//! persistence, no global launcher state, token-safe tracing and metadata-only public API.
use crate::Error;
use crate::HTTP_CLIENT;
use base64::Engine;
use base64::prelude::{BASE64_STANDARD, BASE64_URL_SAFE_NO_PAD};
use chrono::{DateTime, Duration, Utc};
use heck::ToTitleCase;
use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey, VerifyingKey};
use rand::Rng;
use rand::rngs::OsRng;
use reqwest::header::HeaderMap;
use reqwest::{Response, StatusCode};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::json;
use sha2::Digest;
use std::borrow::Cow;
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::time::Instant;
use url::Url;
use uuid::Uuid;

#[derive(Debug, Clone, Copy)]
pub enum MinecraftAuthStep {
	GetDeviceToken,
	SisuAuthenticate,
	GetOAuthToken,
	RefreshOAuthToken,
	SisuAuthorize,
	XstsAuthorize,
	MinecraftToken,
	MinecraftEntitlements,
	MinecraftProfile,
}

#[derive(thiserror::Error, Debug)]
pub enum MinecraftAuthenticationError {
	#[error("Error reading public key during generation")]
	ReadingPublicKey,
	#[error("Failed to serialize private key to PEM: {0}")]
	PEMSerialize(#[from] p256::pkcs8::Error),
	#[error("Failed to serialize body to JSON during step {step:?}: {source}")]
	SerializeBody {
		step: MinecraftAuthStep,
		#[source]
		source: serde_json::Error,
	},
	#[error(
		"Failed to deserialize response to JSON during step {step:?}: {source}. Status Code: {status_code}"
	)]
	DeserializeResponse {
		step: MinecraftAuthStep,
		raw: String,
		#[source]
		source: serde_json::Error,
		status_code: StatusCode,
	},
	#[error("Request failed during step {step:?}: {source}")]
	Request {
		step: MinecraftAuthStep,
		#[source]
		source: reqwest::Error,
	},
	#[error("authentication request failed at {step:?} (HTTP {status_code})")]
	HttpStatus {
		step: MinecraftAuthStep,
		status_code: StatusCode,
	},
	#[error("Error reading XBOX Session ID header")]
	NoSessionId,
	#[error("Error reading user hash")]
	NoUserHash,
}

#[derive(Deserialize)]
struct OAuthErrorResponse {
	error: String,
}

pub struct MinecraftLoginFlow {
	verifier: String,
	session_id: String,
	pub auth_request_uri: String,
	pair: DeviceTokenPair,
	state: String,
}

#[tracing::instrument(skip_all)]
pub(crate) async fn login_begin() -> crate::Result<MinecraftLoginFlow> {
	let (pair, current_date) = DeviceTokenPair::refresh_and_get_device_token(Utc::now()).await?;

	let verifier = generate_oauth_challenge();
	let result = sha2::Sha256::digest(&verifier);
	let challenge = BASE64_URL_SAFE_NO_PAD.encode(result);

	match sisu_authenticate(&pair.token.token, &challenge, &pair.key, current_date).await {
		Ok((session_id, redirect_uri, state)) => {
			return Ok(MinecraftLoginFlow {
				verifier,
				session_id,
				auth_request_uri: redirect_uri.value.msa_oauth_redirect,
				pair,
				state,
			});
		}
		Err(err) => return Err(Error::from(err)),
	}
}

#[tracing::instrument(skip_all)]
pub(crate) async fn login_finish(
	code: &str,
	flow: MinecraftLoginFlow,
) -> crate::Result<Credentials> {
	let pair = flow.pair;

	let oauth_token = oauth_token(code, &flow.verifier).await?;
	let sisu_authorize = sisu_authorize(
		Some(&flow.session_id),
		&oauth_token.value.access_token,
		&pair.token.token,
		&pair.key,
		oauth_token.date,
	)
	.await?;

	let xbox_token = xsts_authorize(
		sisu_authorize.value,
		&pair.token.token,
		&pair.key,
		sisu_authorize.date,
	)
	.await?;
	let minecraft_token = minecraft_token(xbox_token.value).await?;

	minecraft_entitlements(&minecraft_token.access_token).await?;

	let mut credentials = Credentials {
		offline_profile: MinecraftProfile::default(),
		access_token: minecraft_token.access_token,
		refresh_token: oauth_token.value.refresh_token,
		expires: oauth_token.date + Duration::seconds(oauth_token.value.expires_in as i64),
		active: true,
	};

	// During login, we need to fetch the online profile at least once to get the
	// player UUID and name to use for the offline profile, in order for that offline
	// profile to make sense. It's also important to modify the returned credentials
	// object, as otherwise continued usage of it will skip the profile cache due to
	// the dummy UUID
	let online_profile = minecraft_profile(&credentials.access_token).await?;
	credentials.offline_profile = MinecraftProfile {
		id: online_profile.id,
		name: online_profile.name.clone(),
		..credentials.offline_profile
	};

	Ok(credentials)
}

#[derive(Serialize, Deserialize)]
pub(crate) struct Credentials {
	/// The offline profile of the user these credentials are for.
	///
	/// Such a profile can only be relied upon to have a proper player UUID, which is
	/// never changed. A potentially stale username may be available, but no other data
	/// such as skins or capes is available.
	#[serde(rename = "profile")]
	pub offline_profile: MinecraftProfile,
	pub access_token: String,
	pub refresh_token: String,
	pub expires: DateTime<Utc>,
	pub active: bool,
}

impl Credentials {
	pub(crate) async fn refresh(&mut self) -> crate::Result<()> {
		if self.expires > Utc::now() + Duration::minutes(5) {
			return Ok(());
		}
		let oauth = oauth_refresh(&self.refresh_token).await?;
		let (pair, date) = DeviceTokenPair::refresh_and_get_device_token(oauth.date).await?;
		let sisu = sisu_authorize(
			None,
			&oauth.value.access_token,
			&pair.token.token,
			&pair.key,
			date,
		)
		.await?;
		let xbox = xsts_authorize(sisu.value, &pair.token.token, &pair.key, sisu.date).await?;
		let minecraft = minecraft_token(xbox.value).await?;
		self.access_token = minecraft.access_token;
		self.refresh_token = oauth.value.refresh_token;
		self.expires = oauth.date + Duration::seconds(oauth.value.expires_in as i64);
		Ok(())
	}
	pub(crate) async fn profile(&self) -> crate::Result<MinecraftProfile> {
		Ok(minecraft_profile(&self.access_token).await?)
	}
}

pub(crate) struct DeviceTokenPair {
	pub token: DeviceToken,
	pub key: DeviceTokenKey,
}
impl DeviceTokenPair {
	async fn refresh_and_get_device_token(
		date: DateTime<Utc>,
	) -> crate::Result<(Self, DateTime<Utc>)> {
		let key = generate_key()?;
		let response = device_token(&key, date).await?;
		Ok((
			Self {
				key,
				token: response.value,
			},
			response.date,
		))
	}
}
impl MinecraftLoginFlow {
	/// Validates the exact upstream OAuth callback and one-time state before accepting a code.
	pub fn code_from_redirect(&self, url: &Url) -> crate::Result<Option<String>> {
		if url.scheme() != "https"
			|| url.host_str() != Some("login.live.com")
			|| url.path() != "/oauth20_desktop.srf"
		{
			return Ok(None);
		}
		let state = url
			.query_pairs()
			.find(|(name, _)| name == "state")
			.map(|(_, value)| value.into_owned());
		if state.as_deref() != Some(self.state.as_str()) {
			return Err(Error::OtherError(
				"Microsoft OAuth callback state is invalid".to_owned(),
			));
		}
		if url.query_pairs().any(|(name, _)| name == "error") {
			return Err(Error::OtherError(
				"Microsoft sign-in was cancelled or rejected".to_owned(),
			));
		}
		Ok(url
			.query_pairs()
			.find(|(name, _)| name == "code")
			.map(|(_, value)| value.into_owned()))
	}
}
fn safe_error_response(body: &str) -> String {
	match serde_json::from_str::<OAuthErrorResponse>(body) {
		Ok(response) if response.error == "invalid_grant" => {
			r#"{"error":"invalid_grant"}"#.to_owned()
		}
		_ => "[redacted authentication response]".to_owned(),
	}
}
const MICROSOFT_CLIENT_ID: &str = "00000000402b5328";
const AUTH_REPLY_URL: &str = "https://login.live.com/oauth20_desktop.srf";
const REQUESTED_SCOPE: &str = "service::user.auth.xboxlive.com::MBI_SSL";
pub const MINECRAFT_SERVICES_USER_AGENT: &str =
	"NCreate Launcher (https://github.com/Yozekkk/ncreate-launcher)";

pub struct RequestWithDate<T> {
	pub date: DateTime<Utc>,
	pub value: T,
}

// flow steps
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct DeviceToken {
	pub issue_instant: DateTime<Utc>,
	pub not_after: DateTime<Utc>,
	pub token: String,
	pub display_claims: HashMap<String, serde_json::Value>,
}

#[tracing::instrument(skip_all)]
pub async fn device_token(
	key: &DeviceTokenKey,
	current_date: DateTime<Utc>,
) -> Result<RequestWithDate<DeviceToken>, MinecraftAuthenticationError> {
	let res = send_signed_request(
		None,
		"https://device.auth.xboxlive.com/device/authenticate",
		"/device/authenticate",
		json!({
			"Properties": {
				"AuthMethod": "ProofOfPossession",
				"Id": format!("{{{}}}", key.id.to_string().to_uppercase()),
				"DeviceType": "Win32",
				"Version": "10.16.0",
				"ProofKey": {
					"kty": "EC",
					"x": key.x,
					"y": key.y,
					"crv": "P-256",
					"alg": "ES256",
					"use": "sig"
				}
			},
			"RelyingParty": "http://auth.xboxlive.com",
			"TokenType": "JWT"

		}),
		key,
		MinecraftAuthStep::GetDeviceToken,
		current_date,
	)
	.await?;

	Ok(RequestWithDate {
		date: res.current_date,
		value: res.body,
	})
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RedirectUri {
	pub msa_oauth_redirect: String,
}

#[tracing::instrument(skip_all)]
async fn sisu_authenticate(
	token: &str,
	challenge: &str,
	key: &DeviceTokenKey,
	current_date: DateTime<Utc>,
) -> Result<(String, RequestWithDate<RedirectUri>, String), MinecraftAuthenticationError> {
	let state = generate_oauth_challenge();
	let res = send_signed_request::<RedirectUri>(
		None,
		"https://sisu.xboxlive.com/authenticate",
		"/authenticate",
		json!({
		  "AppId": MICROSOFT_CLIENT_ID,
		  "DeviceToken": token,
		  "Offers": [
			REQUESTED_SCOPE
		  ],
		  "Query": {
			"code_challenge": challenge,
			"code_challenge_method": "S256",
			"state": state,
			"prompt": "select_account"
		  },
		  "RedirectUri": AUTH_REPLY_URL,
		  "Sandbox": "RETAIL",
		  "TokenType": "code",
		  "TitleId": "1794566092",
		}),
		key,
		MinecraftAuthStep::SisuAuthenticate,
		current_date,
	)
	.await?;

	let session_id = res
		.headers
		.get("X-SessionId")
		.and_then(|x| x.to_str().ok())
		.ok_or_else(|| MinecraftAuthenticationError::NoSessionId)?
		.to_string();

	Ok((
		session_id,
		RequestWithDate {
			date: res.current_date,
			value: res.body,
		},
		state,
	))
}

#[derive(Deserialize)]
struct OAuthToken {
	// pub token_type: String,
	pub expires_in: u64,
	// pub scope: String,
	pub access_token: String,
	pub refresh_token: String,
	// pub user_id: String,
	// pub foci: String,
}

#[tracing::instrument(skip_all)]
async fn oauth_token(
	code: &str,
	verifier: &str,
) -> Result<RequestWithDate<OAuthToken>, MinecraftAuthenticationError> {
	let mut query = HashMap::new();
	query.insert("client_id", MICROSOFT_CLIENT_ID);
	query.insert("code", code);
	query.insert("code_verifier", verifier);
	query.insert("grant_type", "authorization_code");
	query.insert("redirect_uri", AUTH_REPLY_URL);
	query.insert("scope", REQUESTED_SCOPE);

	let res = auth_retry(|| {
		HTTP_CLIENT
			.post("https://login.live.com/oauth20_token.srf")
			.header("Accept", "application/json")
			.form(&query)
			.send()
	})
	.await
	.map_err(|source| MinecraftAuthenticationError::Request {
		source,
		step: MinecraftAuthStep::GetOAuthToken,
	})?;

	let status = res.status();
	if !status.is_success() {
		return Err(MinecraftAuthenticationError::HttpStatus {
			step: MinecraftAuthStep::GetOAuthToken,
			status_code: status,
		});
	}
	let current_date = get_date_header(res.headers());
	let text = res
		.text()
		.await
		.map_err(|source| MinecraftAuthenticationError::Request {
			source,
			step: MinecraftAuthStep::GetOAuthToken,
		})?;

	let body = serde_json::from_str(&text).map_err(|source| {
		MinecraftAuthenticationError::DeserializeResponse {
			source,
			raw: safe_error_response(&text),
			step: MinecraftAuthStep::GetOAuthToken,
			status_code: status,
		}
	})?;

	Ok(RequestWithDate {
		date: current_date,
		value: body,
	})
}

#[tracing::instrument(skip_all)]
async fn oauth_refresh(
	refresh_token: &str,
) -> Result<RequestWithDate<OAuthToken>, MinecraftAuthenticationError> {
	let mut query = HashMap::new();
	query.insert("client_id", MICROSOFT_CLIENT_ID);
	query.insert("refresh_token", refresh_token);
	query.insert("grant_type", "refresh_token");
	query.insert("redirect_uri", AUTH_REPLY_URL);
	query.insert("scope", REQUESTED_SCOPE);

	let res = auth_retry(|| {
		HTTP_CLIENT
			.post("https://login.live.com/oauth20_token.srf")
			.header("Accept", "application/json")
			.form(&query)
			.send()
	})
	.await
	.map_err(|source| MinecraftAuthenticationError::Request {
		source,
		step: MinecraftAuthStep::RefreshOAuthToken,
	})?;

	let status = res.status();
	if !status.is_success() {
		return Err(MinecraftAuthenticationError::HttpStatus {
			step: MinecraftAuthStep::RefreshOAuthToken,
			status_code: status,
		});
	}
	let current_date = get_date_header(res.headers());
	let text = res
		.text()
		.await
		.map_err(|source| MinecraftAuthenticationError::Request {
			source,
			step: MinecraftAuthStep::RefreshOAuthToken,
		})?;

	let body = serde_json::from_str(&text).map_err(|source| {
		MinecraftAuthenticationError::DeserializeResponse {
			source,
			raw: safe_error_response(&text),
			step: MinecraftAuthStep::RefreshOAuthToken,
			status_code: status,
		}
	})?;

	Ok(RequestWithDate {
		date: current_date,
		value: body,
	})
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct SisuAuthorize {
	// pub authorization_token: DeviceToken,
	// pub device_token: String,
	// pub sandbox: String,
	pub title_token: DeviceToken,
	pub user_token: DeviceToken,
	// pub web_page: String,
}

#[tracing::instrument(skip_all)]
async fn sisu_authorize(
	session_id: Option<&str>,
	access_token: &str,
	device_token: &str,
	key: &DeviceTokenKey,
	current_date: DateTime<Utc>,
) -> Result<RequestWithDate<SisuAuthorize>, MinecraftAuthenticationError> {
	let res = send_signed_request(
		None,
		"https://sisu.xboxlive.com/authorize",
		"/authorize",
		json!({
			"AccessToken": format!("t={access_token}"),
			"AppId": MICROSOFT_CLIENT_ID,
			"DeviceToken": device_token,
			"ProofKey": {
				"kty": "EC",
				"x": key.x,
				"y": key.y,
				"crv": "P-256",
				"alg": "ES256",
				"use": "sig"
			},
			"Sandbox": "RETAIL",
			"SessionId": session_id,
			"SiteName": "user.auth.xboxlive.com",
			"RelyingParty": "http://xboxlive.com",
			"UseModernGamertag": true
		}),
		key,
		MinecraftAuthStep::SisuAuthorize,
		current_date,
	)
	.await?;

	Ok(RequestWithDate {
		date: res.current_date,
		value: res.body,
	})
}

#[tracing::instrument(skip_all)]
async fn xsts_authorize(
	authorize: SisuAuthorize,
	device_token: &str,
	key: &DeviceTokenKey,
	current_date: DateTime<Utc>,
) -> Result<RequestWithDate<DeviceToken>, MinecraftAuthenticationError> {
	let res = send_signed_request(
		None,
		"https://xsts.auth.xboxlive.com/xsts/authorize",
		"/xsts/authorize",
		json!({
			"RelyingParty": "rp://api.minecraftservices.com/",
			"TokenType": "JWT",
			"Properties": {
				"SandboxId": "RETAIL",
				"UserTokens": [authorize.user_token.token],
				"DeviceToken": device_token,
				"TitleToken": authorize.title_token.token,
			},
		}),
		key,
		MinecraftAuthStep::XstsAuthorize,
		current_date,
	)
	.await?;

	Ok(RequestWithDate {
		date: res.current_date,
		value: res.body,
	})
}

#[derive(Deserialize)]
struct MinecraftToken {
	// pub username: String,
	pub access_token: String,
	// pub token_type: String,
	// pub expires_in: u64,
}

#[tracing::instrument(skip_all)]
async fn minecraft_token(
	token: DeviceToken,
) -> Result<MinecraftToken, MinecraftAuthenticationError> {
	let uhs = token
		.display_claims
		.get("xui")
		.and_then(|x| x.get(0))
		.and_then(|x| x.get("uhs"))
		.and_then(|x| x.as_str().map(String::from))
		.ok_or_else(|| MinecraftAuthenticationError::NoUserHash)?;

	let token = token.token;

	let res = auth_retry(|| {
		HTTP_CLIENT
			.post("https://api.minecraftservices.com/launcher/login")
			.header("Accept", "application/json")
			.header("User-Agent", MINECRAFT_SERVICES_USER_AGENT)
			.json(&json!({
				"platform": "PC_LAUNCHER",
				"xtoken": format!("XBL3.0 x={uhs};{token}"),
			}))
			.send()
	})
	.await
	.map_err(|source| MinecraftAuthenticationError::Request {
		source,
		step: MinecraftAuthStep::MinecraftToken,
	})?;

	let status = res.status();
	if !status.is_success() {
		return Err(MinecraftAuthenticationError::HttpStatus {
			step: MinecraftAuthStep::MinecraftToken,
			status_code: status,
		});
	}
	let text = res
		.text()
		.await
		.map_err(|source| MinecraftAuthenticationError::Request {
			source,
			step: MinecraftAuthStep::MinecraftToken,
		})?;

	serde_json::from_str(&text).map_err(|source| {
		MinecraftAuthenticationError::DeserializeResponse {
			source,
			raw: safe_error_response(&text),
			step: MinecraftAuthStep::MinecraftToken,
			status_code: status,
		}
	})
}

#[derive(Deserialize, Serialize, Debug, Copy, Clone, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum MinecraftSkinVariant {
	/// The classic player model, with arms that are 4 pixels wide.
	Classic,
	/// The slim player model, with arms that are 3 pixels wide.
	Slim,
	/// The player model is unknown.
	#[serde(other)]
	Unknown, // Defensive handling of unexpected Mojang API return values to
	         // prevent breaking the entire profile parsing
}

#[derive(Deserialize, Serialize, Debug, Copy, Clone, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum MinecraftCharacterExpressionState {
	/// This expression is selected for being displayed ingame.
	///
	/// At the moment, at most one expression can be selected at a time.
	Active,
	/// This expression is not selected for being displayed ingame.
	Inactive,
	/// The expression selection status is unknown.
	#[serde(other)]
	Unknown, // Defensive handling of unexpected Mojang API return values to
	         // prevent breaking the entire profile parsing
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct MinecraftSkin {
	/// The UUID of this skin object.
	///
	/// As of 2025-04-08, in the production Mojang profile endpoint this UUID
	/// changes every time the player changes their skin, even if the skin
	/// texture is the same as before.
	pub id: Uuid,
	/// The selection state of the skin.
	///
	/// As of 2025-04-08, in the production Mojang profile endpoint this
	/// is always `ACTIVE`, as only a single skin representing the current
	/// skin is returned.
	pub state: MinecraftCharacterExpressionState,
	/// The URL to the skin texture.
	///
	/// As of 2025-04-08, in the production Mojang profile endpoint the file
	/// name for this URL is a hash of the skin texture, so that different
	/// players using the same skin texture will share a texture URL.
	pub url: Arc<Url>,
	/// A hash of the skin texture.
	///
	/// As of 2025-04-08, in the production Mojang profile endpoint this
	/// is always set and the same as the file name of the skin texture URL.
	#[serde(
        default, // Defensive handling of unexpected Mojang API return values to
                 // prevent breaking the entire profile parsing
        rename = "textureKey"
    )]
	pub texture_key: Option<Arc<str>>,
	/// The player model variant this skin is for.
	pub variant: MinecraftSkinVariant,
	/// User-friendly name for the skin.
	///
	/// As of 2025-04-08, in the production Mojang profile endpoint this is
	/// only set if the player has not set a custom skin, and this skin object
	/// is therefore the default skin for the player's UUID.
	#[serde(
		default,
		rename = "alias",
		deserialize_with = "normalize_skin_alias_case"
	)]
	pub name: Option<String>,
}

impl MinecraftSkin {
	/// Robustly computes the texture key for this skin, falling back to its
	/// URL file name and finally to the skin UUID when necessary.
	pub fn texture_key(&self) -> Arc<str> {
		self.texture_key.as_ref().cloned().unwrap_or_else(|| {
			self.url
				.path_segments()
				.and_then(|mut path_segments| path_segments.next_back().map(String::from))
				.unwrap_or_else(|| self.id.as_simple().to_string())
				.into()
		})
	}
}

fn normalize_skin_alias_case<'de, D: Deserializer<'de>>(
	deserializer: D,
) -> Result<Option<String>, D::Error> {
	// Skin aliases have been spotted to be returned in all caps, so make sure
	// they are normalized to a prettier title case
	Ok(<Option<Cow<'_, str>>>::deserialize(deserializer)?.map(|alias| alias.to_title_case()))
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct MinecraftCape {
	/// The UUID of the cape.
	pub id: Uuid,
	/// The selection state of the cape.
	pub state: MinecraftCharacterExpressionState,
	/// The URL to the cape texture.
	pub url: Arc<Url>,
	/// The user-friendly name for the cape.
	#[serde(rename = "alias")]
	pub name: Arc<str>,
}

#[derive(Deserialize, Serialize, Debug, Default, Clone)]
pub struct MinecraftProfile {
	/// The UUID of the player.
	#[serde(default)]
	pub id: Uuid,
	/// The username of the player.
	pub name: String,
	/// The skins the player is known to have.
	///
	/// As of 2025-04-08, in the production Mojang profile endpoint every
	/// player has a single skin.
	pub skins: Vec<MinecraftSkin>,
	/// The capes the player is known to have.
	pub capes: Vec<MinecraftCape>,
	/// The instant when the profile was fetched. See also [Self::is_fresh].
	#[serde(skip)]
	pub fetch_time: Option<Instant>,
}

impl MinecraftProfile {
	/// Returns the currently selected skin for this profile.
	pub fn current_skin(&self) -> crate::Result<&MinecraftSkin> {
		self.skins
			.iter()
			.find(|skin| skin.state == MinecraftCharacterExpressionState::Active)
			// There should always be one active skin, even when the player uses their default skin
			.ok_or_else(|| Error::OtherError("No active skin found".into()))
	}

	/// Returns the currently selected cape for this profile.
	pub fn current_cape(&self) -> Option<&MinecraftCape> {
		self.capes
			.iter()
			.find(|cape| cape.state == MinecraftCharacterExpressionState::Active)
	}
}

#[tracing::instrument(skip_all)]
async fn minecraft_profile(token: &str) -> Result<MinecraftProfile, MinecraftAuthenticationError> {
	let res = auth_retry(|| {
		HTTP_CLIENT
			.get("https://api.minecraftservices.com/minecraft/profile")
			.header("Accept", "application/json")
			.header("User-Agent", MINECRAFT_SERVICES_USER_AGENT)
			.bearer_auth(token)
			// Profiles may be refreshed periodically in response to user actions,
			// so we want each refresh to be fast
			.timeout(std::time::Duration::from_secs(10))
			.send()
	})
	.await
	.map_err(|source| MinecraftAuthenticationError::Request {
		source,
		step: MinecraftAuthStep::MinecraftProfile,
	})?;

	let status = res.status();
	if !status.is_success() {
		return Err(MinecraftAuthenticationError::HttpStatus {
			step: MinecraftAuthStep::MinecraftProfile,
			status_code: status,
		});
	}
	let text = res
		.text()
		.await
		.map_err(|source| MinecraftAuthenticationError::Request {
			source,
			step: MinecraftAuthStep::MinecraftProfile,
		})?;

	let mut profile = serde_json::from_str::<MinecraftProfile>(&text).map_err(|source| {
		MinecraftAuthenticationError::DeserializeResponse {
			source,
			raw: safe_error_response(&text),
			step: MinecraftAuthStep::MinecraftProfile,
			status_code: status,
		}
	})?;
	profile.fetch_time = Some(Instant::now());

	Ok(profile)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MinecraftEntitlements {}

#[tracing::instrument(skip_all)]
async fn minecraft_entitlements(
	token: &str,
) -> Result<MinecraftEntitlements, MinecraftAuthenticationError> {
	let res = auth_retry(|| {
		HTTP_CLIENT
			.get(format!(
				"https://api.minecraftservices.com/entitlements/license?requestId={}",
				Uuid::new_v4()
			))
			.header("Accept", "application/json")
			.header("User-Agent", MINECRAFT_SERVICES_USER_AGENT)
			.bearer_auth(token)
			.send()
	})
	.await
	.map_err(|source| MinecraftAuthenticationError::Request {
		source,
		step: MinecraftAuthStep::MinecraftEntitlements,
	})?;

	let status = res.status();
	if !status.is_success() {
		return Err(MinecraftAuthenticationError::HttpStatus {
			step: MinecraftAuthStep::MinecraftEntitlements,
			status_code: status,
		});
	}
	let text = res
		.text()
		.await
		.map_err(|source| MinecraftAuthenticationError::Request {
			source,
			step: MinecraftAuthStep::MinecraftEntitlements,
		})?;

	serde_json::from_str(&text).map_err(|source| {
		MinecraftAuthenticationError::DeserializeResponse {
			source,
			raw: safe_error_response(&text),
			step: MinecraftAuthStep::MinecraftEntitlements,
			status_code: status,
		}
	})
}

// auth utils
#[tracing::instrument(skip_all)]
async fn auth_retry<F>(reqwest_request: impl Fn() -> F) -> Result<reqwest::Response, reqwest::Error>
where
	F: Future<Output = Result<Response, reqwest::Error>>,
{
	const RETRY_COUNT: usize = 5; // Does command 9 times
	const RETRY_WAIT: std::time::Duration = std::time::Duration::from_millis(250);

	let mut resp = reqwest_request().await;
	for i in 0..RETRY_COUNT {
		match &resp {
			Ok(_) => {
				break;
			}
			Err(err) => {
				if err.is_connect() || err.is_timeout() {
					if i < RETRY_COUNT - 1 {
						tracing::debug!("Request failed with connect error, retrying...",);
						tokio::time::sleep(RETRY_WAIT).await;
						resp = reqwest_request().await;
					} else {
						break;
					}
				}
			}
		}
	}

	resp
}

pub struct DeviceTokenKey {
	pub id: Uuid,
	pub key: SigningKey,
	pub x: String,
	pub y: String,
}

#[tracing::instrument(skip_all)]
fn generate_key() -> Result<DeviceTokenKey, MinecraftAuthenticationError> {
	let uuid = Uuid::new_v4();

	let signing_key = SigningKey::random(&mut OsRng);
	let public_key = VerifyingKey::from(&signing_key);

	let encoded_point = public_key.to_encoded_point(false);

	Ok(DeviceTokenKey {
		id: uuid,
		key: signing_key,
		x: BASE64_URL_SAFE_NO_PAD.encode(
			encoded_point
				.x()
				.ok_or_else(|| MinecraftAuthenticationError::ReadingPublicKey)?,
		),
		y: BASE64_URL_SAFE_NO_PAD.encode(
			encoded_point
				.y()
				.ok_or_else(|| MinecraftAuthenticationError::ReadingPublicKey)?,
		),
	})
}

struct SignedRequestResponse<T> {
	pub headers: HeaderMap,
	pub current_date: DateTime<Utc>,
	pub body: T,
}

#[tracing::instrument(skip_all)]
async fn send_signed_request<T: DeserializeOwned>(
	authorization: Option<&str>,
	url: &str,
	url_path: &str,
	raw_body: serde_json::Value,
	key: &DeviceTokenKey,
	step: MinecraftAuthStep,
	current_date: DateTime<Utc>,
) -> Result<SignedRequestResponse<T>, MinecraftAuthenticationError> {
	let auth = authorization.map_or(Vec::new(), |v| v.as_bytes().to_vec());

	let body = serde_json::to_vec(&raw_body)
		.map_err(|source| MinecraftAuthenticationError::SerializeBody { source, step })?;
	let time: u128 = { ((current_date.timestamp() as u128) + 11644473600) * 10000000 };

	let mut buffer = Vec::new();
	buffer.extend_from_slice(&1_u32.to_be_bytes()[..]);
	buffer.push(0_u8);
	buffer.extend_from_slice(&(time as u64).to_be_bytes()[..]);
	buffer.push(0_u8);
	buffer.extend_from_slice("POST".as_bytes());
	buffer.push(0_u8);
	buffer.extend_from_slice(url_path.as_bytes());
	buffer.push(0_u8);
	buffer.extend_from_slice(&auth);
	buffer.push(0_u8);
	buffer.extend_from_slice(&body);
	buffer.push(0_u8);

	let ecdsa_sig: Signature = key.key.sign(&buffer);

	let mut sig_buffer = Vec::new();
	sig_buffer.extend_from_slice(&1_i32.to_be_bytes()[..]);
	sig_buffer.extend_from_slice(&(time as u64).to_be_bytes()[..]);
	sig_buffer.extend_from_slice(&ecdsa_sig.r().to_bytes());
	sig_buffer.extend_from_slice(&ecdsa_sig.s().to_bytes());

	let signature = BASE64_STANDARD.encode(&sig_buffer);

	let res = auth_retry(|| {
		let mut request = HTTP_CLIENT
			.post(url)
			.header("Content-Type", "application/json; charset=utf-8")
			.header("Accept", "application/json")
			.header("Signature", &signature);

		if url != "https://sisu.xboxlive.com/authorize" {
			request = request.header("x-xbl-contract-version", "1");
		}

		if let Some(auth) = authorization {
			request = request.header("Authorization", auth);
		}

		request.body(body.clone()).send()
	})
	.await
	.map_err(|source| MinecraftAuthenticationError::Request { source, step })?;

	let status = res.status();
	if !status.is_success() {
		return Err(MinecraftAuthenticationError::HttpStatus {
			step,
			status_code: status,
		});
	}
	let headers = res.headers().clone();

	let current_date = get_date_header(&headers);

	let body = res
		.text()
		.await
		.map_err(|source| MinecraftAuthenticationError::Request { source, step })?;

	let body = serde_json::from_str(&body).map_err(|source| {
		MinecraftAuthenticationError::DeserializeResponse {
			source,
			raw: safe_error_response(&body),
			step,
			status_code: status,
		}
	})?;
	Ok(SignedRequestResponse {
		headers,
		current_date,
		body,
	})
}

#[tracing::instrument(skip_all)]
fn get_date_header(headers: &HeaderMap) -> DateTime<Utc> {
	headers
		.get(reqwest::header::DATE)
		.and_then(|x| x.to_str().ok())
		.and_then(|x| DateTime::parse_from_rfc2822(x).ok())
		.map_or(Utc::now(), |x| x.with_timezone(&Utc))
}

#[tracing::instrument(skip_all)]
fn generate_oauth_challenge() -> String {
	let mut rng = rand::thread_rng();

	let bytes: Vec<u8> = (0..64).map(|_| rng.r#gen::<u8>()).collect();
	bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
	use super::*;
	fn flow() -> MinecraftLoginFlow {
		MinecraftLoginFlow {
			verifier: "private-verifier".into(),
			session_id: "private-session".into(),
			auth_request_uri: "https://login.live.com/authorize?state=expected".into(),
			state: "expected".into(),
			pair: DeviceTokenPair {
				key: generate_key().expect("test key"),
				token: DeviceToken {
					issue_instant: Utc::now(),
					not_after: Utc::now(),
					token: "private-device-token".into(),
					display_claims: HashMap::new(),
				},
			},
		}
	}
	#[test]
	fn callback_requires_exact_https_origin_and_state() {
		let flow = flow();
		for input in [
			"http://login.live.com/oauth20_desktop.srf?state=expected&code=secret",
			"https://login.live.com.attacker.example/oauth20_desktop.srf?state=expected&code=secret",
			"https://login.live.com/other?state=expected&code=secret",
		] {
			assert!(
				flow.code_from_redirect(&Url::parse(input).unwrap())
					.unwrap()
					.is_none()
			);
		}
		for input in [
			"https://login.live.com/oauth20_desktop.srf?code=secret",
			"https://login.live.com/oauth20_desktop.srf?state=wrong&code=secret",
		] {
			assert!(
				flow.code_from_redirect(&Url::parse(input).unwrap())
					.is_err()
			);
		}
		assert_eq!(
			flow.code_from_redirect(
				&Url::parse(
					"https://login.live.com/oauth20_desktop.srf?state=expected&code=test-code"
				)
				.unwrap()
			)
			.unwrap()
			.as_deref(),
			Some("test-code")
		);
	}
	#[test]
	fn malformed_auth_response_cannot_expose_tokens() {
		let raw = r#"{"access_token":"access-secret","refresh_token":"refresh-secret"}"#;
		let safe = safe_error_response(raw);
		assert!(!safe.contains("access-secret"));
		assert!(!safe.contains("refresh-secret"));
		let error = MinecraftAuthenticationError::DeserializeResponse {
			step: MinecraftAuthStep::GetOAuthToken,
			raw: safe,
			source: serde_json::from_str::<String>("invalid").unwrap_err(),
			status_code: StatusCode::BAD_REQUEST,
		};
		assert!(!format!("{error:?}").contains("access-secret"));
		assert!(!format!("{error}").contains("refresh-secret"));
	}
}
