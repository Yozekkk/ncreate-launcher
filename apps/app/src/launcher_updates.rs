//! GitHub release discovery and Tauri's signed desktop updater stay behind native IPC.
use crate::{AppState, local, storage::Result};
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};
use tauri::Emitter;
use tauri_plugin_updater::{Update, UpdaterExt};

const RELEASES_API: &str =
	"https://api.github.com/repos/Yozekkk/ncreate-launcher/releases?per_page=30";
const MAX_RELEASES_BYTES: usize = 2 * 1024 * 1024;
const MAX_METADATA_BYTES: usize = 64 * 1024;
const MAX_UPDATE_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Channel {
	Stable,
	Beta,
}
impl Channel {
	fn parse(value: &str) -> Result<Self> {
		match value {
			"stable" => Ok(Self::Stable),
			"beta" => Ok(Self::Beta),
			_ => Err("Неизвестный канал обновлений".into()),
		}
	}
	fn as_str(self) -> &'static str {
		match self {
			Self::Stable => "stable",
			Self::Beta => "beta",
		}
	}
}

#[derive(Debug, Deserialize)]
struct Release {
	tag_name: String,
	name: String,
	draft: bool,
	prerelease: bool,
	assets: Vec<ReleaseAsset>,
}
#[derive(Debug, Deserialize)]
struct ReleaseAsset {
	name: String,
	browser_download_url: String,
}
struct Candidate {
	version: Version,
	channel: Channel,
	metadata_url: url::Url,
}

pub struct PendingUpdate {
	channel: String,
	update: Update,
	size_bytes: u64,
	sha256: String,
}

#[derive(Serialize)]
pub struct UpdateInfo {
	available: bool,
	current_version: String,
	version: Option<String>,
	notes: Option<String>,
	size_bytes: Option<u64>,
}
#[derive(Clone, Serialize)]
struct UpdateProgress {
	phase: &'static str,
	downloaded_bytes: u64,
	total_bytes: Option<u64>,
	bytes_per_second: Option<u64>,
	message: Option<String>,
}

fn release_channel(release: &Release) -> Channel {
	let title = release.name.to_lowercase();
	if release.prerelease || title.contains("beta") || title.contains("бета") {
		Channel::Beta
	} else {
		Channel::Stable
	}
}

fn github_asset_url(value: &str) -> Result<url::Url> {
	let parsed = url::Url::parse(value).map_err(|_| "Некорректный URL обновления".to_string())?;
	if parsed.scheme() != "https"
		|| parsed.host_str() != Some("github.com")
		|| !parsed
			.path()
			.starts_with("/Yozekkk/ncreate-launcher/releases/download/")
		|| !parsed.username().is_empty()
		|| parsed.password().is_some()
		|| parsed.query().is_some()
		|| parsed.fragment().is_some()
	{
		return Err("Недоверенный адрес обновления".into());
	}
	Ok(parsed)
}

fn newest_candidate(
	releases: Vec<Release>,
	channel: Channel,
	current: &Version,
) -> Result<Option<Candidate>> {
	let mut candidates = Vec::new();
	for release in releases {
		if release.draft {
			continue;
		}
		let Ok(version) = Version::parse(release.tag_name.trim_start_matches('v')) else {
			continue;
		};
		if version <= *current {
			continue;
		}
		let actual_channel = release_channel(&release);
		if channel == Channel::Stable && actual_channel != Channel::Stable {
			continue;
		}
		let Some(asset) = release
			.assets
			.iter()
			.find(|asset| asset.name == "latest.json")
		else {
			continue;
		};
		candidates.push(Candidate {
			version,
			channel: actual_channel,
			metadata_url: github_asset_url(&asset.browser_download_url)?,
		});
	}
	candidates.sort_by(|left, right| left.version.cmp(&right.version));
	Ok(candidates.pop())
}

fn validate_metadata(bytes: &[u8], candidate: &Candidate) -> Result<serde_json::Value> {
	let value: serde_json::Value =
		serde_json::from_slice(bytes).map_err(|_| "Некорректные данные обновления".to_string())?;
	if value.get("version").and_then(serde_json::Value::as_str)
		!= Some(candidate.version.to_string().as_str())
		|| value.get("channel").and_then(serde_json::Value::as_str)
			!= Some(candidate.channel.as_str())
	{
		return Err("Версия или канал обновления не совпадает с релизом".into());
	}
	let platforms = value
		.get("platforms")
		.and_then(serde_json::Value::as_object)
		.ok_or_else(|| "Нет списка платформ обновления".to_string())?;
	if platforms.is_empty() || platforms.len() > 6 {
		return Err("Некорректный список платформ обновления".into());
	}
	for (target, platform) in platforms {
		if !["linux-x86_64", "windows-x86_64"].contains(&target.as_str()) {
			return Err("Неизвестная платформа обновления".into());
		}
		let url = platform
			.get("url")
			.and_then(serde_json::Value::as_str)
			.ok_or_else(|| "Нет адреса файла обновления".to_string())?;
		github_asset_url(url)?;
		if platform
			.get("signature")
			.and_then(serde_json::Value::as_str)
			.is_none_or(|signature| signature.is_empty() || signature.len() > 4096)
		{
			return Err("Нет подписи обновления".into());
		}
		if platform
			.get("size")
			.and_then(serde_json::Value::as_u64)
			.is_none_or(|size| size == 0 || size > MAX_UPDATE_BYTES)
		{
			return Err("Некорректный размер обновления".into());
		}
		if platform
			.get("sha256")
			.and_then(serde_json::Value::as_str)
			.is_none_or(|hash| {
				hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit())
			}) {
			return Err("Нет контрольной суммы обновления".into());
		}
	}
	Ok(value)
}

fn verify_update_bytes(bytes: &[u8], size: u64, sha256: &str) -> Result<()> {
	if bytes.len() as u64 != size
		|| !format!("{:x}", Sha256::digest(bytes)).eq_ignore_ascii_case(sha256)
	{
		return Err("Размер или контрольная сумма обновления не совпадает".into());
	}
	Ok(())
}

fn github_client() -> Result<reqwest::Client> {
	reqwest::Client::builder()
		.user_agent(concat!("NCreate-Launcher/", env!("CARGO_PKG_VERSION")))
		.https_only(true)
		.timeout(Duration::from_secs(15))
		.redirect(reqwest::redirect::Policy::custom(|attempt| {
			let url = attempt.url();
			if attempt.previous().len() >= 5
				|| url.scheme() != "https"
				|| !matches!(
					url.host_str(),
					Some("github.com" | "api.github.com" | "release-assets.githubusercontent.com")
				) {
				attempt.stop()
			} else {
				attempt.follow()
			}
		}))
		.build()
		.map_err(|_| "Не удалось подготовить проверку обновлений".into())
}

async fn fetch_limited(client: &reqwest::Client, url: &str, limit: usize) -> Result<Vec<u8>> {
	let mut response = client
		.get(url)
		.send()
		.await
		.map_err(|_| "Не удалось подключиться к GitHub для проверки обновлений".to_string())?
		.error_for_status()
		.map_err(|_| "GitHub не предоставил данные обновления".to_string())?;
	let mut bytes = Vec::new();
	while let Some(chunk) = response
		.chunk()
		.await
		.map_err(|_| "Ошибка загрузки данных обновления".to_string())?
	{
		if bytes.len().saturating_add(chunk.len()) > limit {
			return Err("Данные обновления превышают допустимый размер".into());
		}
		bytes.extend_from_slice(&chunk);
	}
	Ok(bytes)
}

async fn ensure_channel(state: &tauri::State<'_, AppState>, value: &str) -> Result<Channel> {
	let channel = Channel::parse(value)?;
	if state
		.store()
		.await?
		.snapshot()
		.await?
		.settings
		.release_channel
		!= value
	{
		return Err("Канал обновлений изменился. Проверьте обновления снова.".into());
	}
	Ok(channel)
}

#[tauri::command]
pub async fn launcher_check_update(
	window: tauri::WebviewWindow,
	app: tauri::AppHandle,
	state: tauri::State<'_, AppState>,
	channel: String,
) -> Result<UpdateInfo> {
	local(&window)?;
	let channel = ensure_channel(&state, &channel).await?;
	let _guard = state
		.update_gate
		.try_lock()
		.map_err(|_| "Проверка или установка обновления уже выполняется".to_string())?;
	*state.pending_update.lock().await = None;
	let current_version = app.package_info().version.to_string();
	let current =
		Version::parse(&current_version).map_err(|_| "Некорректная версия лаунчера".to_string())?;
	let client = github_client()?;
	let releases: Vec<Release> =
		serde_json::from_slice(&fetch_limited(&client, RELEASES_API, MAX_RELEASES_BYTES).await?)
			.map_err(|_| "Некорректный ответ GitHub Releases".to_string())?;
	let Some(candidate) = newest_candidate(releases, channel, &current)? else {
		return Ok(UpdateInfo {
			available: false,
			current_version,
			version: None,
			notes: None,
			size_bytes: None,
		});
	};
	let metadata =
		fetch_limited(&client, candidate.metadata_url.as_str(), MAX_METADATA_BYTES).await?;
	validate_metadata(&metadata, &candidate)?;
	let updater = app
		.updater_builder()
		.endpoints(vec![candidate.metadata_url.clone()])
		.map_err(|_| "Недоверенный адрес обновления".to_string())?
		.timeout(Duration::from_secs(30))
		.build()
		.map_err(|_| "Не удалось запустить проверку обновления".to_string())?;
	let Some(update) = updater
		.check()
		.await
		.map_err(|_| "Не удалось проверить подписанное обновление".to_string())?
	else {
		return Ok(UpdateInfo {
			available: false,
			current_version,
			version: None,
			notes: None,
			size_bytes: None,
		});
	};
	if update.version != candidate.version.to_string() {
		return Err("Версия обновления изменилась во время проверки".into());
	}
	let returned = serde_json::to_vec(&update.raw_json)
		.map_err(|_| "Некорректные данные обновления".to_string())?;
	let value = validate_metadata(&returned, &candidate)?;
	github_asset_url(update.download_url.as_str())?;
	let target = tauri_plugin_updater::target()
		.ok_or_else(|| "Платформа не поддерживает обновление".to_string())?;
	let platform = value
		.get("platforms")
		.and_then(|platforms| platforms.get(&target))
		.ok_or_else(|| "Для этой платформы нет обновления".to_string())?;
	let size_bytes = platform
		.get("size")
		.and_then(serde_json::Value::as_u64)
		.ok_or_else(|| "Некорректный размер обновления".to_string())?;
	let sha256 = platform
		.get("sha256")
		.and_then(serde_json::Value::as_str)
		.ok_or_else(|| "Нет контрольной суммы обновления".to_string())?
		.to_owned();
	let info = UpdateInfo {
		available: true,
		current_version,
		version: Some(update.version.clone()),
		notes: update.body.clone(),
		size_bytes: Some(size_bytes),
	};
	*state.pending_update.lock().await = Some(PendingUpdate {
		channel: channel.as_str().into(),
		update,
		size_bytes,
		sha256,
	});
	Ok(info)
}

#[tauri::command]
pub async fn launcher_install_update(
	window: tauri::WebviewWindow,
	app: tauri::AppHandle,
	state: tauri::State<'_, AppState>,
	channel: String,
) -> Result<()> {
	local(&window)?;
	ensure_channel(&state, &channel).await?;
	let _guard = state
		.update_gate
		.try_lock()
		.map_err(|_| "Установка обновления уже выполняется".to_string())?;
	let pending = state
		.pending_update
		.lock()
		.await
		.take()
		.ok_or_else(|| "Сначала проверьте доступность обновления".to_string())?;
	if pending.channel != channel {
		return Err("Канал обновлений изменился. Проверьте обновления снова.".into());
	}
	let started = Instant::now();
	let mut downloaded_bytes = 0_u64;
	let mut last_emit = Instant::now() - Duration::from_secs(1);
	let total_bytes = Some(pending.size_bytes);
	let progress_app = app.clone();
	let bytes = pending
		.update
		.download(
			move |chunk, content_length| {
				downloaded_bytes = downloaded_bytes.saturating_add(chunk as u64);
				if last_emit.elapsed() >= Duration::from_millis(150) {
					let speed = (downloaded_bytes as f64
						/ started.elapsed().as_secs_f64().max(0.001)) as u64;
					let _ = progress_app.emit_to(
						"main",
						"launcher-update-progress",
						UpdateProgress {
							phase: "downloading",
							downloaded_bytes,
							total_bytes: total_bytes.or(content_length),
							bytes_per_second: Some(speed),
							message: None,
						},
					);
					last_emit = Instant::now();
				}
			},
			|| {},
		)
		.await
		.map_err(|_| {
			let message = "Не удалось скачать или проверить подпись обновления".to_string();
			let _ = app.emit_to(
				"main",
				"launcher-update-progress",
				UpdateProgress {
					phase: "error",
					downloaded_bytes: 0,
					total_bytes,
					bytes_per_second: None,
					message: Some(message.clone()),
				},
			);
			message
		})?;
	if let Err(message) = verify_update_bytes(&bytes, pending.size_bytes, &pending.sha256) {
		let _ = app.emit_to(
			"main",
			"launcher-update-progress",
			UpdateProgress {
				phase: "error",
				downloaded_bytes: bytes.len() as u64,
				total_bytes,
				bytes_per_second: None,
				message: Some(message.clone()),
			},
		);
		return Err(message);
	}
	let _ = app.emit_to(
		"main",
		"launcher-update-progress",
		UpdateProgress {
			phase: "installing",
			downloaded_bytes: bytes.len() as u64,
			total_bytes: Some(bytes.len() as u64),
			bytes_per_second: None,
			message: None,
		},
	);
	pending.update.install(bytes).map_err(|_| {
		let message = "Не удалось установить обновление".to_string();
		let _ = app.emit_to(
			"main",
			"launcher-update-progress",
			UpdateProgress {
				phase: "error",
				downloaded_bytes: 0,
				total_bytes,
				bytes_per_second: None,
				message: Some(message.clone()),
			},
		);
		message
	})?;
	let _ = app.emit_to(
		"main",
		"launcher-update-progress",
		UpdateProgress {
			phase: "restarting",
			downloaded_bytes: 0,
			total_bytes,
			bytes_per_second: None,
			message: None,
		},
	);
	#[cfg(not(windows))]
	app.request_restart();
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use base64::Engine as _;
	use minisign_verify::{PublicKey, Signature};
	use std::{io::Write, net::TcpListener};

	fn release(tag: &str, name: &str, prerelease: bool, with_metadata: bool) -> Release {
		Release {
			tag_name: tag.into(),
			name: name.into(),
			draft: false,
			prerelease,
			assets: if with_metadata {
				vec![ReleaseAsset {
					name: "latest.json".into(),
					browser_download_url: format!(
						"https://github.com/Yozekkk/ncreate-launcher/releases/download/{tag}/latest.json"
					),
				}]
			} else {
				Vec::new()
			},
		}
	}
	#[test]
	fn stable_never_selects_beta_and_legacy_unsigned_release_is_ignored() {
		let releases = vec![
			release("v0.5.0", "NCreate Launcher v0.5.0 Beta", false, false),
			release("v0.6.0-beta.1", "NCreate Launcher Beta", true, true),
			release("v0.6.0", "NCreate Launcher v0.6.0", false, true),
		];
		let selected = newest_candidate(releases, Channel::Stable, &Version::new(0, 5, 0))
			.expect("valid releases")
			.expect("stable update");
		assert_eq!(selected.version, Version::new(0, 6, 0));
		assert_eq!(selected.channel, Channel::Stable);
	}
	#[test]
	fn beta_can_select_beta_but_no_update_for_current_version() {
		let releases = vec![release("v0.7.0-beta.1", "Beta", true, true)];
		assert!(
			newest_candidate(releases, Channel::Stable, &Version::new(0, 6, 0))
				.unwrap()
				.is_none()
		);
		let beta = newest_candidate(
			vec![release("v0.7.0-beta.1", "Beta", true, true)],
			Channel::Beta,
			&Version::new(0, 6, 0),
		)
		.unwrap();
		assert_eq!(
			beta.expect("beta candidate").version.to_string(),
			"0.7.0-beta.1"
		);
		assert!(
			newest_candidate(
				vec![release("v0.5.0", "Beta", false, true)],
				Channel::Beta,
				&Version::new(0, 5, 0)
			)
			.unwrap()
			.is_none()
		);
	}
	#[test]
	fn metadata_rejects_channel_mismatch_unsafe_url_and_oversize() {
		let candidate = newest_candidate(
			vec![release("v0.6.0", "Stable", false, true)],
			Channel::Stable,
			&Version::new(0, 5, 0),
		)
		.unwrap()
		.unwrap();
		let mut metadata = serde_json::json!({"version":"0.6.0","channel":"stable","platforms":{"linux-x86_64":{"url":"https://github.com/Yozekkk/ncreate-launcher/releases/download/v0.6.0/app.AppImage","signature":"signed","size":100,"sha256":"0000000000000000000000000000000000000000000000000000000000000000"}}});
		assert!(validate_metadata(metadata.to_string().as_bytes(), &candidate).is_ok());
		metadata["channel"] = "beta".into();
		assert!(validate_metadata(metadata.to_string().as_bytes(), &candidate).is_err());
		metadata["channel"] = "stable".into();
		metadata["platforms"]["linux-x86_64"]["url"] = "http://example.com/app".into();
		assert!(validate_metadata(metadata.to_string().as_bytes(), &candidate).is_err());
		metadata["platforms"]["linux-x86_64"]["url"] =
			"https://github.com/Yozekkk/ncreate-launcher/releases/download/v0.6.0/app.AppImage"
				.into();
		metadata["platforms"]["linux-x86_64"]["size"] = (MAX_UPDATE_BYTES + 1).into();
		assert!(validate_metadata(metadata.to_string().as_bytes(), &candidate).is_err());
		metadata["platforms"]["linux-x86_64"]["size"] = 100.into();
		metadata["platforms"]["linux-x86_64"]["sha256"] = "bad".into();
		assert!(validate_metadata(metadata.to_string().as_bytes(), &candidate).is_err());
	}
	#[test]
	fn downloaded_update_requires_exact_size_and_sha256() {
		let bytes = b"signed updater fixture";
		let hash = format!("{:x}", Sha256::digest(bytes));
		assert!(verify_update_bytes(bytes, bytes.len() as u64, &hash).is_ok());
		assert!(verify_update_bytes(bytes, bytes.len() as u64 + 1, &hash).is_err());
		assert!(verify_update_bytes(b"altered", bytes.len() as u64, &hash).is_err());
	}
	#[test]
	fn configured_public_key_accepts_signed_fixture_and_rejects_tampering() {
		let config: serde_json::Value =
			serde_json::from_str(include_str!("../tauri.conf.json")).expect("Tauri config");
		let key = config["plugins"]["updater"]["pubkey"]
			.as_str()
			.expect("public key");
		let public = base64::engine::general_purpose::STANDARD
			.decode(key)
			.expect("base64 public key");
		let public = PublicKey::decode(std::str::from_utf8(&public).expect("public key text"))
			.expect("valid public key");
		let signed = base64::engine::general_purpose::STANDARD
			.decode(include_str!("../tests/fixtures/updater-v0.6.0.sig").trim())
			.expect("base64 signature");
		let signature = Signature::decode(std::str::from_utf8(&signed).expect("signature text"))
			.expect("valid signature");
		let payload = include_bytes!("../tests/fixtures/updater-v0.6.0.txt");
		assert!(public.verify(payload, &signature, true).is_ok());
		assert!(signature.trusted_comment().contains("version:0.6.0"));
		assert!(
			public
				.verify(b"tampered package", &signature, true)
				.is_err()
		);
	}
	#[tokio::test]
	async fn metadata_fetch_reports_offline_and_interrupted_responses() {
		let client = reqwest::Client::builder()
			.timeout(Duration::from_secs(2))
			.build()
			.expect("HTTP test client");
		let offline_listener = TcpListener::bind("127.0.0.1:0").expect("unused local port");
		let offline_url = format!("http://{}", offline_listener.local_addr().unwrap());
		drop(offline_listener);
		assert!(fetch_limited(&client, &offline_url, 64).await.is_err());

		let listener = TcpListener::bind("127.0.0.1:0").expect("local test server");
		let url = format!("http://{}", listener.local_addr().unwrap());
		let server = std::thread::spawn(move || {
			let (mut stream, _) = listener.accept().expect("client connection");
			let _ = stream.write_all(
				b"HTTP/1.1 200 OK\r\nContent-Length: 20\r\nConnection: close\r\n\r\npartial",
			);
		});
		assert!(fetch_limited(&client, &url, 64).await.is_err());
		server.join().expect("server completed");
	}
	#[test]
	#[ignore = "requires a fresh signed local AppImage production build"]
	fn signed_local_appimage_matches_configured_updater_key() {
		let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
			.join("../../target/release/bundle/appimage")
			.join(format!(
				"NCreate-Launcher-{}.AppImage",
				env!("CARGO_PKG_VERSION")
			));
		let config: serde_json::Value =
			serde_json::from_str(include_str!("../tauri.conf.json")).expect("Tauri config");
		let public = base64::engine::general_purpose::STANDARD
			.decode(
				config["plugins"]["updater"]["pubkey"]
					.as_str()
					.expect("public key"),
			)
			.expect("base64 public key");
		let public = PublicKey::decode(std::str::from_utf8(&public).expect("public key text"))
			.expect("valid public key");
		let encoded = std::fs::read_to_string(format!("{}.sig", root.display()))
			.expect("signed AppImage artifact");
		let signature = base64::engine::general_purpose::STANDARD
			.decode(encoded.trim())
			.expect("base64 signature");
		let signature = Signature::decode(std::str::from_utf8(&signature).expect("signature text"))
			.expect("valid signature");
		let artifact = std::fs::read(&root).expect("AppImage artifact");
		public
			.verify(&artifact, &signature, true)
			.expect("signed AppImage matches public key");
		assert!(
			signature
				.trusted_comment()
				.contains(&format!("version:{}", env!("CARGO_PKG_VERSION")))
		);
	}
}
