//! Synthetic HTTP responses exercise the actual production authentication functions.
use crate::{AuthEngine, ElyAuthEngine, ElyAuthError, ElyOAuthConfig, transport, vault};
use base64::{
	Engine,
	prelude::{BASE64_STANDARD, BASE64_URL_SAFE_NO_PAD},
};
use chrono::{Duration, Utc};
use p256::ecdsa::{Signature, VerifyingKey, signature::Verifier};
use serde_json::{Value, json};
use sha2::Digest;
use std::{
	collections::HashMap,
	sync::{
		Arc, Mutex,
		atomic::{AtomicBool, Ordering},
	},
	thread,
	time::Duration as StdDuration,
};
use tiny_http::{Header, Response, Server, StatusCode};
use uuid::Uuid;
const UUID: &str = "12345678-1234-4234-9234-123456789abc";

#[derive(Clone)]
struct Record {
	route: String,
	body: String,
}
struct Fixture {
	base: String,
	records: Arc<Mutex<Vec<Record>>>,
	secrets: Arc<Mutex<HashMap<String, String>>>,
	stop: Arc<AtomicBool>,
	worker: Option<thread::JoinHandle<()>>,
}
impl Fixture {
	fn new(fail: Option<(&str, u16)>) -> Self {
		Self::with_identity(fail, UUID)
	}
	fn with_identity(fail: Option<(&str, u16)>, identity: &str) -> Self {
		let identity = identity.to_owned();
		let server = Server::http("127.0.0.1:0").expect("fixture server");
		let base = format!("http://{}", server.server_addr());
		let records = Arc::new(Mutex::new(Vec::new()));
		let recorded = records.clone();
		let stop = Arc::new(AtomicBool::new(false));
		let stopped = stop.clone();
		let fail = fail.map(|(route, status)| (route.to_owned(), status));
		let worker = thread::spawn(move || {
			let mut key: Option<VerifyingKey> = None;
			let mut challenge = String::new();
			while !stopped.load(Ordering::SeqCst) {
				let Some(mut request) = server
					.recv_timeout(StdDuration::from_millis(30))
					.expect("fixture receive")
				else {
					continue;
				};
				let route = request
					.url()
					.split('?')
					.next()
					.expect("fixture route")
					.to_owned();
				let mut body = String::new();
				request
					.as_reader()
					.read_to_string(&mut body)
					.expect("fixture body");
				recorded.lock().expect("records lock").push(Record {
					route: route.clone(),
					body: body.clone(),
				});
				let json_body: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
				if route.ends_with("/device/authenticate") {
					let proof = &json_body["Properties"]["ProofKey"];
					let mut encoded = vec![4];
					encoded.extend(
						BASE64_URL_SAFE_NO_PAD
							.decode(proof["x"].as_str().expect("proof x"))
							.expect("x base64"),
					);
					encoded.extend(
						BASE64_URL_SAFE_NO_PAD
							.decode(proof["y"].as_str().expect("proof y"))
							.expect("y base64"),
					);
					key = Some(VerifyingKey::from_sec1_bytes(&encoded).expect("public device key"));
				}
				if let Some(signature) = request
					.headers()
					.iter()
					.find(|h| h.field.equiv("Signature"))
				{
					let signature = BASE64_STANDARD
						.decode(signature.value.as_str())
						.expect("signature base64");
					assert_eq!(signature.len(), 76, "upstream Xbox signature envelope");
					let path = format!("/{}", route.splitn(3, '/').nth(2).expect("signed route"));
					let mut signed = Vec::new();
					signed.extend(&signature[..4]);
					signed.push(0);
					signed.extend(&signature[4..12]);
					signed.push(0);
					signed.extend(b"POST");
					signed.push(0);
					signed.extend(path.as_bytes());
					signed.push(0);
					signed.push(0);
					signed.extend(body.as_bytes());
					signed.push(0);
					key.as_ref()
						.expect("device proof key")
						.verify(
							&signed,
							&Signature::from_slice(&signature[12..]).expect("ECDSA signature"),
						)
						.expect("upstream signed request verifies");
				}
				let date = Utc::now();
				let token = json!({"IssueInstant":date,"NotAfter":date+Duration::hours(24),"Token":"synthetic-xbox-token","DisplayClaims":{"xui":[{"uhs":"synthetic-user-hash"}]}});
				let mut status = 200;
				let mut session = false;
				let response = match route.as_str() {
					"/device.auth.xboxlive.com/device/authenticate" => token.clone(),
					"/sisu.xboxlive.com/authenticate" => {
						challenge = json_body["Query"]["code_challenge"]
							.as_str()
							.expect("PKCE challenge")
							.to_owned();
						session = true;
						json!({"MsaOauthRedirect":format!("https://login.live.com/oauth20_authorize.srf?state={}",json_body["Query"]["state"].as_str().expect("OAuth state"))})
					}
					"/login.live.com/oauth20_token.srf" => {
						let form: HashMap<String, String> =
							url::form_urlencoded::parse(body.as_bytes())
								.into_owned()
								.collect();
						if form.get("grant_type").map(String::as_str) == Some("authorization_code")
						{
							assert_eq!(
								form.get("code").map(String::as_str),
								Some("synthetic-code")
							);
							let verifier = form.get("code_verifier").expect("PKCE verifier");
							assert_eq!(
								BASE64_URL_SAFE_NO_PAD
									.encode(sha2::Sha256::digest(verifier.as_bytes())),
								challenge,
								"OAuth code exchange uses same PKCE verifier"
							);
						} else {
							assert_eq!(
								form.get("refresh_token").map(String::as_str),
								Some("synthetic-refresh")
							);
						}
						json!({"access_token":"synthetic-oauth-access","refresh_token":"synthetic-refresh","expires_in":3600})
					}
					"/sisu.xboxlive.com/authorize" => {
						json!({"TitleToken":token.clone(),"UserToken":token.clone()})
					}
					"/xsts.auth.xboxlive.com/xsts/authorize" => token,
					"/api.minecraftservices.com/launcher/login" => {
						json!({"access_token":"synthetic-minecraft-access"})
					}
					"/api.minecraftservices.com/entitlements/license" => json!({}),
					"/api.minecraftservices.com/minecraft/profile" => {
						json!({"id":identity,"name":"NCreateSynthetic","skins":[],"capes":[]})
					}
					"/authserver.ely.by/auth/authenticate" => {
						let password = json_body["password"]
							.as_str()
							.expect("password memory only");
						if password == "synthetic-2fa" {
							status = 401;
							json!({"error":"ForbiddenOperationException","errorMessage":"Account protected with two factor auth."})
						} else {
							assert!(
								password == "synthetic-password"
									|| password == "synthetic-2fa:123456"
							);
							json!({"accessToken":"synthetic-ely-access","clientToken":json_body["clientToken"],"selectedProfile":{"id":identity,"name":"NCreateSynthetic"}})
						}
					}
					"/authserver.ely.by/auth/refresh" => {
						json!({"accessToken":"synthetic-ely-refreshed","clientToken":json_body["clientToken"],"selectedProfile":{"id":identity,"name":"NCreateSynthetic"}})
					}
					"/authserver.ely.by/auth/validate" => json!({}),
					"/authserver.ely.by/auth/invalidate" => json!({}),
					"/account.ely.by/api/oauth2/v1/token" => {
						json!({"access_token":"synthetic-ely-oauth","refresh_token":"synthetic-ely-refresh","expires_in":86400})
					}
					"/account.ely.by/api/account/v1/info" => {
						json!({"uuid":identity,"username":"NCreateSynthetic"})
					}
					_ => panic!("unexpected fixture endpoint"),
				};
				let response = if fail.as_ref().is_some_and(|(failed, _)| failed == &route) {
					status = fail.as_ref().expect("failure plan").1;
					json!({"access_token":"never-log-this-token","error":"synthetic_error"})
				} else {
					response
				};
				let body = if fail
					.as_ref()
					.is_some_and(|(failed, status)| failed == &route && *status == 0)
				{
					status = 200;
					"malformed-response-containing-synthetic-secret".to_owned()
				} else {
					response.to_string()
				};
				let mut response = Response::from_string(body)
					.with_status_code(StatusCode(status))
					.with_header(
						Header::from_bytes("Content-Type", "application/json")
							.expect("JSON header"),
					)
					.with_header(
						Header::from_bytes("Date", date.to_rfc2822()).expect("date header"),
					);
				if session {
					response = response.with_header(
						Header::from_bytes("X-SessionId", "synthetic-session")
							.expect("session header"),
					);
				}
				request.respond(response).expect("fixture response");
			}
		});
		Self {
			base,
			records,
			secrets: Arc::new(Mutex::new(HashMap::new())),
			stop,
			worker: Some(worker),
		}
	}
	async fn scope<F: std::future::Future>(&self, future: F) -> F::Output {
		let client = reqwest::Client::builder()
			.timeout(StdDuration::from_secs(2))
			.redirect(reqwest::redirect::Policy::none())
			.build()
			.expect("fixture client");
		transport::TEST_HTTP
			.scope(
				(client, self.base.clone()),
				vault::TEST_VAULT.scope(self.secrets.clone(), future),
			)
			.await
	}
	fn routes(&self) -> Vec<String> {
		self.records
			.lock()
			.expect("records lock")
			.iter()
			.map(|r| r.route.clone())
			.collect()
	}
}
impl Drop for Fixture {
	fn drop(&mut self) {
		self.stop.store(true, Ordering::SeqCst);
		self.worker
			.take()
			.expect("worker")
			.join()
			.expect("fixture server assertions");
	}
}
async fn finish(engine: &AuthEngine) -> crate::Result<crate::MicrosoftProfile> {
	let flow = engine.begin().await?;
	let url = url::Url::parse(&flow.auth_request_uri).expect("mock auth URL");
	let state = url
		.query_pairs()
		.find(|(key, _)| key == "state")
		.expect("state")
		.1
		.into_owned();
	let mut callback =
		url::Url::parse("https://login.live.com/oauth20_desktop.srf").expect("callback URL");
	callback
		.query_pairs_mut()
		.append_pair("state", &state)
		.append_pair("code", "synthetic-code");
	let code = flow.code_from_redirect(&callback)?.expect("callback code");
	engine.finish(&code, flow).await
}
#[tokio::test]
async fn microsoft_actual_pipeline_persistence_refresh_session_and_remove() {
	let fixture = Fixture::new(None);
	fixture
		.scope(async {
			let engine = AuthEngine::new();
			let profile = finish(&engine).await.expect("actual pipeline finish");
			assert_eq!(profile.uuid.to_string(), UUID);
			assert!(
				!serde_json::to_string(&profile)
					.expect("IPC metadata")
					.contains("synthetic-minecraft-access")
			);
			let mut saved: Value = serde_json::from_str(
				fixture
					.secrets
					.lock()
					.expect("vault")
					.get(UUID)
					.expect("saved credentials"),
			)
			.expect("credentials JSON");
			saved["expires"] = json!(Utc::now() - Duration::hours(1));
			fixture
				.secrets
				.lock()
				.expect("vault")
				.insert(UUID.into(), saved.to_string());
			let session = engine
				.session(Uuid::parse_str(UUID).expect("uuid"))
				.await
				.expect("refresh then launch session");
			assert_eq!(session.token(), "synthetic-minecraft-access");
			assert!(!format!("{session:?}").contains("synthetic-minecraft-access"));
			engine.refresh(profile.uuid).await.expect("profile refresh");
			engine.remove(profile.uuid).await.expect("remove");
			assert!(fixture.secrets.lock().expect("vault").is_empty());
			assert!(engine.session(profile.uuid).await.is_err());
		})
		.await;
	let routes = fixture.routes();
	for route in [
		"/device.auth.xboxlive.com/device/authenticate",
		"/sisu.xboxlive.com/authenticate",
		"/login.live.com/oauth20_token.srf",
		"/sisu.xboxlive.com/authorize",
		"/xsts.auth.xboxlive.com/xsts/authorize",
		"/api.minecraftservices.com/launcher/login",
		"/api.minecraftservices.com/entitlements/license",
		"/api.minecraftservices.com/minecraft/profile",
	] {
		assert!(routes.iter().any(|r| r == route));
	}
}
#[tokio::test]
async fn microsoft_failures_at_every_real_network_step_never_persist_credentials() {
	for route in [
		"/device.auth.xboxlive.com/device/authenticate",
		"/sisu.xboxlive.com/authenticate",
		"/login.live.com/oauth20_token.srf",
		"/sisu.xboxlive.com/authorize",
		"/xsts.auth.xboxlive.com/xsts/authorize",
		"/api.minecraftservices.com/launcher/login",
		"/api.minecraftservices.com/entitlements/license",
		"/api.minecraftservices.com/minecraft/profile",
	] {
		let fixture = Fixture::new(Some((route, 401)));
		fixture
			.scope(async {
				let error = finish(&AuthEngine::new())
					.await
					.expect_err("failure injection");
				assert!(!format!("{error:?}").contains("never-log-this-token"));
				assert!(fixture.secrets.lock().expect("vault").is_empty());
			})
			.await;
		assert_eq!(
			fixture.routes().last().map(String::as_str),
			Some(route),
			"fixture must reach the injected failing production step"
		);
	}
}
#[tokio::test]
async fn cancelled_microsoft_flow_does_not_exchange_code_or_save_tokens() {
	let fixture = Fixture::new(None);
	fixture
		.scope(async {
			let flow = AuthEngine::new().begin().await.expect("begin");
			drop(flow);
			assert!(fixture.secrets.lock().expect("vault").is_empty());
		})
		.await;
	assert_eq!(fixture.routes().len(), 2);
}
#[tokio::test]
async fn ely_legacy_real_protocol_totp_validation_refresh_invalidation_and_restart() {
	let fixture = Fixture::new(None);
	fixture
		.scope(async {
			let engine = ElyAuthEngine::new();
			assert!(matches!(
				engine
					.authenticate("test".into(), "synthetic-2fa".into(), None)
					.await,
				Err(ElyAuthError::TwoFactorRequired)
			));
			let profile = engine
				.authenticate("test".into(), "synthetic-2fa".into(), Some("123456".into()))
				.await
				.expect("TOTP login");
			let saved = fixture
				.secrets
				.lock()
				.expect("vault")
				.values()
				.cloned()
				.collect::<Vec<_>>()
				.join("");
			assert!(!saved.contains("synthetic-2fa"));
			let saved_json: Value = serde_json::from_str(&saved).expect("saved credential JSON");
			assert!(saved_json.get("password").is_none());
			assert!(saved_json.get("totp").is_none());
			let restarted = ElyAuthEngine::new();
			assert!(
				restarted
					.validate(profile.uuid)
					.await
					.expect("validate after restart")
			);
			restarted.refresh(profile.uuid).await.expect("refresh");
			let session = restarted
				.session(profile.uuid)
				.await
				.expect("Ely game session");
			assert_eq!(session.token(), "synthetic-ely-refreshed");
			assert!(!format!("{session:?}").contains(session.token()));
			restarted
				.remove(profile.uuid)
				.await
				.expect("invalidate + delete");
			assert!(fixture.secrets.lock().expect("vault").is_empty());
		})
		.await;
	assert!(
		fixture
			.routes()
			.iter()
			.any(|r| r.ends_with("/auth/invalidate"))
	);
	assert!(
		fixture
			.records
			.lock()
			.expect("records")
			.iter()
			.any(|r| r.body.contains("synthetic-2fa:123456"))
	);
}
#[tokio::test]
async fn ely_oauth_configured_real_protocol_state_and_secure_persistence() {
	let fixture = Fixture::new(None);
	fixture
		.scope(async {
			assert!(matches!(
				ElyAuthEngine::new().begin_oauth(),
				Err(ElyAuthError::OAuthUnconfigured)
			));
			let config = ElyOAuthConfig::new(
				"synthetic-client".into(),
				"synthetic-client-secret".into(),
				url::Url::parse("https://ncreate.example/ely/callback").expect("redirect"),
			)
			.expect("config");
			let engine = ElyAuthEngine::with_oauth(config);
			let flow = engine.begin_oauth().expect("begin OAuth");
			let initial = url::Url::parse(&flow.auth_request_uri).expect("OAuth URL");
			let state = initial
				.query_pairs()
				.find(|(k, _)| k == "state")
				.expect("state")
				.1
				.into_owned();
			assert!(
				flow.code_from_redirect(
					&url::Url::parse(
						"https://ncreate.example/ely/callback?state=wrong&code=secret"
					)
					.expect("URL")
				)
				.is_err()
			);
			let mut callback =
				url::Url::parse("https://ncreate.example/ely/callback").expect("URL");
			callback
				.query_pairs_mut()
				.append_pair("state", &state)
				.append_pair("code", "synthetic-oauth-code");
			let code = flow
				.code_from_redirect(&callback)
				.expect("state")
				.expect("code");
			let profile = engine
				.finish_oauth(&code, flow)
				.await
				.expect("OAuth finish");
			engine.refresh(profile.uuid).await.expect("OAuth refresh");
			assert_eq!(
				engine.session(profile.uuid).await.expect("session").token(),
				"synthetic-ely-oauth"
			);
			assert!(
				!fixture
					.secrets
					.lock()
					.expect("vault")
					.values()
					.any(|v| v.contains("synthetic-client-secret"))
			);
			engine.remove(profile.uuid).await.expect("local delete");
		})
		.await;
}

#[tokio::test]
async fn microsoft_restart_and_multiple_accounts_keep_distinct_credential_references() {
	let first = Fixture::new(None);
	let mut second = Fixture::with_identity(None, "87654321-4321-4321-9321-cba987654321");
	second.secrets = first.secrets.clone();
	let first_profile = first
		.scope(finish(&AuthEngine::new()))
		.await
		.expect("first login");
	let second_profile = second
		.scope(finish(&AuthEngine::new()))
		.await
		.expect("second login");
	assert_eq!(first.secrets.lock().expect("vault").len(), 2);
	let restarted = AuthEngine::new();
	first
		.scope(async {
			assert_eq!(
				restarted
					.session(first_profile.uuid)
					.await
					.expect("first persisted session")
					.identity
					.uuid,
				first_profile.uuid
			);
			restarted
				.remove(first_profile.uuid)
				.await
				.expect("first logout");
		})
		.await;
	second
		.scope(async {
			assert_eq!(
				restarted
					.session(second_profile.uuid)
					.await
					.expect("second remains saved")
					.identity
					.uuid,
				second_profile.uuid
			);
			restarted
				.remove(second_profile.uuid)
				.await
				.expect("second logout");
		})
		.await;
	assert!(first.secrets.lock().expect("vault").is_empty());
}
#[tokio::test]
async fn malformed_response_at_each_production_step_never_exposes_or_saves_tokens() {
	for route in [
		"/device.auth.xboxlive.com/device/authenticate",
		"/sisu.xboxlive.com/authenticate",
		"/login.live.com/oauth20_token.srf",
		"/sisu.xboxlive.com/authorize",
		"/xsts.auth.xboxlive.com/xsts/authorize",
		"/api.minecraftservices.com/launcher/login",
		"/api.minecraftservices.com/entitlements/license",
		"/api.minecraftservices.com/minecraft/profile",
	] {
		let fixture = Fixture::new(Some((route, 0)));
		fixture
			.scope(async {
				let error = finish(&AuthEngine::new())
					.await
					.expect_err("malformed response");
				assert!(!format!("{error} {error:?}").contains("synthetic-secret"));
				assert!(fixture.secrets.lock().expect("vault").is_empty());
			})
			.await;
		assert_eq!(fixture.routes().last().map(String::as_str), Some(route));
	}
}
#[tokio::test]
async fn ely_bad_credentials_validation_failure_and_invalidation_outage_are_safe() {
	let rejected = Fixture::new(Some(("/authserver.ely.by/auth/authenticate", 401)));
	rejected
		.scope(async {
			assert!(matches!(
				ElyAuthEngine::new()
					.authenticate("test".into(), "synthetic-password".into(), None)
					.await,
				Err(ElyAuthError::InvalidCredentials)
			));
			assert!(rejected.secrets.lock().expect("vault").is_empty());
		})
		.await;
	let fixture = Fixture::new(Some(("/authserver.ely.by/auth/validate", 401)));
	let uuid = fixture
		.scope(async {
			let engine = ElyAuthEngine::new();
			let profile = engine
				.authenticate("test".into(), "synthetic-password".into(), None)
				.await
				.expect("login");
			assert!(!engine.validate(profile.uuid).await.expect("invalid token"));
			profile.uuid
		})
		.await;
	let mut outage = Fixture::new(Some(("/authserver.ely.by/auth/invalidate", 503)));
	outage.secrets = fixture.secrets.clone();
	outage
		.scope(async {
			ElyAuthEngine::new()
				.remove(uuid)
				.await
				.expect("local logout despite service outage");
		})
		.await;
	assert!(fixture.secrets.lock().expect("vault").is_empty());
}

#[tokio::test]
async fn vault_save_failure_is_closed_for_both_authenticated_providers() {
	let fixture = Fixture::new(None);
	fixture
		.scope(vault::TEST_VAULT_DENY_WRITE.scope(true, async {
			assert!(matches!(
				finish(&AuthEngine::new()).await,
				Err(crate::Error::Vault(_))
			));
			assert!(matches!(
				ElyAuthEngine::new()
					.authenticate("test".into(), "synthetic-password".into(), None)
					.await,
				Err(ElyAuthError::SecureStorage)
			));
			assert!(fixture.secrets.lock().expect("vault").is_empty());
		}))
		.await;
}
#[tokio::test]
async fn invalid_profiles_and_failed_refresh_cannot_replace_saved_identity_or_tokens() {
	let fixture = Fixture::with_identity(None, "00000000-0000-0000-0000-000000000000");
	fixture
		.scope(async {
			assert!(finish(&AuthEngine::new()).await.is_err());
			assert!(
				ElyAuthEngine::new()
					.authenticate("test".into(), "synthetic-password".into(), None)
					.await
					.is_err()
			);
			assert!(fixture.secrets.lock().expect("vault").is_empty());
		})
		.await;
	let saved = Fixture::new(None);
	let profile = saved
		.scope(finish(&AuthEngine::new()))
		.await
		.expect("initial login");
	{
		let mut secrets = saved.secrets.lock().expect("vault");
		let mut credentials: Value = serde_json::from_str(
			secrets
				.get(&profile.uuid.to_string())
				.expect("saved credentials"),
		)
		.expect("stored JSON");
		credentials["expires"] = json!(Utc::now() - Duration::hours(1));
		secrets.insert(profile.uuid.to_string(), credentials.to_string());
	}
	let prior = saved
		.secrets
		.lock()
		.expect("vault")
		.get(&profile.uuid.to_string())
		.expect("saved entry")
		.clone();
	let mut rejected = Fixture::new(Some(("/login.live.com/oauth20_token.srf", 401)));
	rejected.secrets = saved.secrets.clone();
	rejected
		.scope(async {
			assert!(AuthEngine::new().session(profile.uuid).await.is_err());
			assert_eq!(
				rejected
					.secrets
					.lock()
					.expect("vault")
					.get(&profile.uuid.to_string()),
				Some(&prior)
			);
		})
		.await;
}

#[tokio::test]
#[ignore = "Native operating-system vault; requires an unlocked desktop keyring"]
async fn native_credential_roundtrip_and_delete_without_real_account_tokens() {
	let key = format!("stage2-verification-{}", Uuid::new_v4());
	vault::save(
		&key,
		&json!({"marker":"synthetic-nonsecret-native-vault-fixture"}),
	)
	.await
	.expect("native credential save");
	let loaded: Value = vault::load(&key)
		.await
		.expect("native credential read")
		.expect("saved marker");
	vault::remove(&key)
		.await
		.expect("native credential deletion");
	assert_eq!(loaded["marker"], "synthetic-nonsecret-native-vault-fixture");
	assert!(
		vault::load::<Value>(&key)
			.await
			.expect("absence after deletion")
			.is_none()
	);
}
