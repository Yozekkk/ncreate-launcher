//! Public Ely.by/Mojang skin previews; no credentials or arbitrary frontend URL fetches.
use crate::storage::{Account, Result, Store};
use base64::{Engine, prelude::BASE64_STANDARD};
use futures_util::StreamExt;
use image::{DynamicImage, ImageFormat, imageops};
use serde::Serialize;
use std::{io::Cursor, time::Duration};

#[derive(Serialize, Clone)]
pub struct Skin {
	pub provider: String,
	pub head: Option<String>,
	pub texture: Option<String>,
	pub status: String,
}
pub struct SkinService {
	client: reqwest::Client,
}
impl SkinService {
	pub fn new() -> Result<Self> {
		let client = reqwest::Client::builder()
			.https_only(true)
			.redirect(reqwest::redirect::Policy::none())
			.timeout(Duration::from_secs(12))
			.connect_timeout(Duration::from_secs(5))
			.user_agent("NCreate Launcher/0.1.0")
			.build()
			.map_err(|_| "Не удалось создать соединение".to_string())?;
		Ok(Self { client })
	}
	pub async fn load(&self, account: &Account, store: &Store, force: bool) -> Result<Skin> {
		let cache = store
			.directory
			.join("skins")
			.join(format!("{}.png", account.uuid));
		let cached = match tokio::fs::metadata(&cache).await {
			Ok(metadata) if metadata.is_file() && metadata.len() <= 1_048_576 => {
				tokio::fs::read(&cache)
					.await
					.ok()
					.filter(|bytes| bytes.len() <= 1_048_576)
			}
			_ => None,
		};
		let provider = if account.kind == "microsoft" {
			"mojang"
		} else {
			"ely_by"
		};
		let fresh = tokio::fs::metadata(&cache)
			.await
			.ok()
			.and_then(|m| m.modified().ok())
			.and_then(|t| t.elapsed().ok())
			.is_some_and(|age| age < Duration::from_secs(3600));
		if fresh
			&& !force && let Some(bytes) = &cached
			&& let Ok(skin) = render(bytes, provider, "ready")
		{
			return Ok(skin);
		}
		let url = if account.kind == "offline" || account.kind == "ely_by" {
			format!("https://skinsystem.ely.by/skins/{}.png", account.nickname)
		} else if let Some(url) = &account.skin_url {
			let mut parsed =
				url::Url::parse(url).map_err(|_| "Некорректный адрес скина".to_string())?;
			if parsed.host_str() != Some("textures.minecraft.net")
				|| !parsed.path().starts_with("/texture/")
			{
				return fallback("missing");
			}
			parsed
				.set_scheme("https")
				.map_err(|_| "Некорректный адрес скина".to_string())?;
			parsed.to_string()
		} else {
			return fallback("missing");
		};
		let result = self.download(&url).await;
		match result {
			Ok(Some(bytes)) => {
				let skin =
					render(&bytes, provider, "ready").or_else(|_| fallback("network_error"))?;
				if skin.provider != "fallback" {
					tokio::fs::create_dir_all(store.directory.join("skins"))
						.await
						.map_err(crate::storage::db_error)?;
					let staging = cache.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
					tokio::fs::write(&staging, &bytes)
						.await
						.map_err(crate::storage::db_error)?;
					if let Err(e) = tokio::fs::rename(&staging, &cache).await {
						let _ = tokio::fs::remove_file(&staging).await;
						return Err(crate::storage::db_error(e));
					}
					sqlx::query("UPDATE accounts SET skin_provider = ? WHERE uuid = ?")
						.bind(provider)
						.bind(&account.uuid)
						.execute(&store.pool)
						.await
						.map_err(crate::storage::db_error)?;
				}
				Ok(skin)
			}
			Ok(None) => {
				sqlx::query("UPDATE accounts SET skin_provider = 'fallback' WHERE uuid = ?")
					.bind(&account.uuid)
					.execute(&store.pool)
					.await
					.map_err(crate::storage::db_error)?;
				fallback("missing")
			}
			Err(_) => {
				if let Some(bytes) = cached {
					render(&bytes, provider, "network_error").or_else(|_| fallback("network_error"))
				} else {
					fallback("network_error")
				}
			}
		}
	}
	fn request(&self, url: &url::Url) -> reqwest::RequestBuilder {
		#[cfg(test)]
		if let Ok((client, base)) = TEST_SKIN_HTTP.try_with(Clone::clone) {
			return client.get(format!("{}{}", base, url.path()));
		}
		self.client.get(url.clone())
	}
	async fn download(&self, url: &str) -> Result<Option<Vec<u8>>> {
		let mut url = url::Url::parse(url).map_err(|_| "Некорректный адрес скина".to_string())?;
		for _ in 0..4 {
			if !allowed_skin_url(&url) {
				return Err("Недопустимый адрес скина".into());
			}
			let response = self
				.request(&url)
				.send()
				.await
				.map_err(|_| "Сервис скинов недоступен".to_string())?;
			if response.status().is_redirection() {
				let location = response
					.headers()
					.get(reqwest::header::LOCATION)
					.and_then(|v| v.to_str().ok())
					.ok_or("Сервис скинов не указал адрес")?;
				url = url
					.join(location)
					.map_err(|_| "Некорректный адрес скина".to_string())?;
				if url.scheme() == "http" {
					url.set_scheme("https")
						.map_err(|_| "Некорректный адрес скина".to_string())?;
				}
				continue;
			}
			if [404, 204].contains(&response.status().as_u16()) {
				return Ok(None);
			}
			if !response.status().is_success()
				|| response.content_length().is_some_and(|n| n > 1_048_576)
			{
				return Err("Не удалось получить скин".into());
			}
			let mut stream = response.bytes_stream();
			let mut bytes = Vec::new();
			while let Some(chunk) = stream.next().await {
				let chunk = chunk.map_err(|_| "Не удалось получить скин".to_string())?;
				if bytes.len() + chunk.len() > 1_048_576 {
					return Err("Скин слишком большой".into());
				}
				bytes.extend_from_slice(&chunk);
			}
			return Ok(Some(bytes));
		}
		Err("Слишком много перенаправлений скина".into())
	}
}
#[cfg(test)]
tokio::task_local! { static TEST_SKIN_HTTP: (reqwest::Client,String); }
fn allowed_skin_url(url: &url::Url) -> bool {
	url.scheme() == "https"
		&& url.port_or_known_default() == Some(443)
		&& url.username().is_empty()
		&& url.password().is_none()
		&& url.fragment().is_none()
		&& match url.host_str() {
			Some("skinsystem.ely.by") => url.path().starts_with("/skins/"),
			Some("ely.by") => url.path().starts_with("/storage/skins/"),
			Some("textures.minecraft.net") => url.path().starts_with("/texture/"),
			_ => false,
		}
}
fn fallback(status: &str) -> Result<Skin> {
	render(include_bytes!("../assets/steve.png"), "fallback", status)
}
fn render(bytes: &[u8], provider: &str, status: &str) -> Result<Skin> {
	if bytes.len() > 1_048_576 {
		return Err("Скин слишком большой".into());
	}
	let mut reader = image::ImageReader::with_format(Cursor::new(bytes), ImageFormat::Png);
	let mut limits = image::Limits::default();
	limits.max_image_width = Some(64);
	limits.max_image_height = Some(64);
	limits.max_alloc = Some(1_048_576);
	reader.limits(limits);
	let texture = reader
		.decode()
		.map_err(|_| "Некорректный скин".to_string())?;
	if texture.width() != 64 || ![32, 64].contains(&texture.height()) {
		return Err("Некорректный размер скина".into());
	}
	let mut head = texture.crop_imm(8, 8, 8, 8).to_rgba8();
	imageops::overlay(&mut head, &texture.crop_imm(40, 8, 8, 8).to_rgba8(), 0, 0);
	let head = imageops::resize(&head, 128, 128, imageops::FilterType::Nearest);
	Ok(Skin {
		provider: provider.into(),
		head: Some(data_url(&DynamicImage::ImageRgba8(head))?),
		texture: Some(data_url(&texture)?),
		status: status.into(),
	})
}
fn data_url(image: &DynamicImage) -> Result<String> {
	let mut bytes = Cursor::new(Vec::new());
	image
		.write_to(&mut bytes, ImageFormat::Png)
		.map_err(|_| "Не удалось отобразить скин".to_string())?;
	Ok(format!(
		"data:image/png;base64,{}",
		BASE64_STANDARD.encode(bytes.into_inner())
	))
}
#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn fallback_is_valid_preview() {
		let skin = fallback("missing").expect("fallback valid");
		assert_eq!(skin.provider, "fallback");
		assert!(
			skin.head
				.expect("head")
				.starts_with("data:image/png;base64,")
		);
	}
	#[test]
	fn skin_redirects_stay_on_public_texture_hosts() {
		for allowed in [
			"https://skinsystem.ely.by/skins/NCreate.png",
			"https://ely.by/storage/skins/skin.png",
			"https://textures.minecraft.net/texture/skin",
		] {
			assert!(allowed_skin_url(
				&url::Url::parse(allowed).expect("valid URL")
			));
		}
		for denied in [
			"http://ely.by/storage/skins/skin.png",
			"https://ely.by/account",
			"https://ely.by.evil.example/storage/skins/skin.png",
			"https://localhost/texture/skin",
			"https://user:secret@ely.by/storage/skins/skin.png",
			"https://ely.by:8443/storage/skins/skin.png",
		] {
			assert!(!allowed_skin_url(
				&url::Url::parse(denied).expect("valid URL")
			));
		}
	}
	#[test]
	fn invalid_skin_is_rejected() {
		assert!(render(b"not a png", "ely_by", "ready").is_err());
	}
	#[test]
	fn rejects_oversized_truncated_and_extreme_png_headers() {
		assert!(render(&vec![0; 1_048_577], "ely_by", "ready").is_err());
		let valid = include_bytes!("../assets/steve.png");
		assert!(render(&valid[..valid.len() / 2], "ely_by", "ready").is_err());
		let huge = image::DynamicImage::ImageRgba8(image::RgbaImage::new(128, 128));
		let mut bytes = Cursor::new(Vec::new());
		huge.write_to(&mut bytes, ImageFormat::Png)
			.expect("oversized image fixture");
		assert!(render(bytes.get_ref(), "ely_by", "ready").is_err());
	}
	#[test]
	fn face_crop_composites_hat_without_jumping_layout() {
		let mut texture = image::RgbaImage::new(64, 64);
		for y in 8..16 {
			for x in 8..16 {
				texture.put_pixel(x, y, image::Rgba([255, 0, 0, 255]));
			}
		}
		for y in 8..16 {
			for x in 40..48 {
				texture.put_pixel(x, y, image::Rgba([0, 255, 0, 128]));
			}
		}
		let mut bytes = Cursor::new(Vec::new());
		DynamicImage::ImageRgba8(texture)
			.write_to(&mut bytes, ImageFormat::Png)
			.expect("skin fixture");
		let skin = render(bytes.get_ref(), "ely_by", "ready").expect("render");
		let head = skin.head.expect("head");
		let png = BASE64_STANDARD
			.decode(
				head.strip_prefix("data:image/png;base64,")
					.expect("head data URL"),
			)
			.expect("base64");
		let head = image::load_from_memory(&png).expect("head PNG").to_rgba8();
		assert_eq!(head.dimensions(), (128, 128));
		let pixel = head.get_pixel(0, 0);
		assert!(pixel[0] > 100 && pixel[1] > 100);
		assert!(pixel[3] >= 254);
	}

	struct Fixture {
		base: String,
		mode: std::sync::Arc<std::sync::atomic::AtomicUsize>,
		requests: std::sync::Arc<std::sync::atomic::AtomicUsize>,
		stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
		thread: Option<std::thread::JoinHandle<()>>,
	}
	impl Fixture {
		fn new() -> Self {
			use std::sync::{
				Arc,
				atomic::{AtomicBool, AtomicUsize, Ordering},
			};
			let server = tiny_http::Server::http("127.0.0.1:0").expect("mock bind");
			let base = format!("http://{}", server.server_addr());
			let mode = Arc::new(AtomicUsize::new(0));
			let requests = Arc::new(AtomicUsize::new(0));
			let stop = Arc::new(AtomicBool::new(false));
			let (mode_worker, count_worker, stop_worker) =
				(mode.clone(), requests.clone(), stop.clone());
			let thread = std::thread::spawn(move || {
				while !stop_worker.load(Ordering::SeqCst) {
					let Some(request) = server
						.recv_timeout(Duration::from_millis(20))
						.expect("mock receive")
					else {
						continue;
					};
					count_worker.fetch_add(1, Ordering::SeqCst);
					let response = match mode_worker.load(Ordering::SeqCst) {
						1 => tiny_http::Response::from_data(Vec::new()).with_status_code(404),
						2 => tiny_http::Response::from_data(Vec::new()).with_status_code(503),
						3 => {
							std::thread::sleep(Duration::from_millis(100));
							tiny_http::Response::from_data(
								include_bytes!("../assets/steve.png").to_vec(),
							)
						}
						4 => tiny_http::Response::from_data(Vec::new())
							.with_status_code(302)
							.with_header(
								tiny_http::Header::from_bytes(
									"Location",
									"https://localhost/private",
								)
								.expect("header"),
							),
						5 => tiny_http::Response::from_data(vec![0; 1_048_577]),
						_ => tiny_http::Response::from_data(
							include_bytes!("../assets/steve.png").to_vec(),
						),
					};
					let _ = request.respond(response);
				}
			});
			Self {
				base,
				mode,
				requests,
				stop,
				thread: Some(thread),
			}
		}
		fn set(&self, mode: usize) {
			self.mode.store(mode, std::sync::atomic::Ordering::SeqCst);
		}
		fn count(&self) -> usize {
			self.requests.load(std::sync::atomic::Ordering::SeqCst)
		}
	}
	impl Drop for Fixture {
		fn drop(&mut self) {
			self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
			if let Some(thread) = self.thread.take() {
				thread.join().expect("mock thread");
			}
		}
	}
	#[tokio::test]
	async fn cached_skin_force_reload_and_network_fallback_are_bounded() {
		let fixture = Fixture::new();
		let directory =
			std::env::temp_dir().join(format!("ncreate-skin-test-{}", uuid::Uuid::new_v4()));
		let store = Store::open(directory.clone())
			.await
			.expect("isolated store");
		let account = store
			.add_offline("SkinTester")
			.await
			.expect("offline profile")
			.accounts
			.remove(0);
		let service = SkinService::new().expect("client");
		let client = reqwest::Client::builder()
			.redirect(reqwest::redirect::Policy::none())
			.timeout(Duration::from_secs(1))
			.build()
			.expect("mock client");
		TEST_SKIN_HTTP
			.scope((client, fixture.base.clone()), async {
				assert_eq!(
					service
						.load(&account, &store, false)
						.await
						.expect("network skin")
						.provider,
					"ely_by"
				);
				assert_eq!(fixture.count(), 1);
				fixture.set(2);
				assert_eq!(
					service
						.load(&account, &store, false)
						.await
						.expect("cached skin")
						.status,
					"ready"
				);
				assert_eq!(fixture.count(), 1, "cache avoids network");
				assert_eq!(
					service
						.load(&account, &store, true)
						.await
						.expect("offline cached skin")
						.status,
					"network_error"
				);
				assert_eq!(fixture.count(), 2, "force bypasses cache");
				tokio::fs::write(
					directory
						.join("skins")
						.join(format!("{}.png", account.uuid)),
					b"invalid png",
				)
				.await
				.expect("corrupt cache");
				assert_eq!(
					service
						.load(&account, &store, false)
						.await
						.expect("bad cache fallback")
						.provider,
					"fallback"
				);
				fixture.set(1);
				assert_eq!(
					service
						.load(&account, &store, true)
						.await
						.expect("missing skin")
						.status,
					"missing"
				);
				assert_eq!(
					store
						.account(&account.uuid)
						.await
						.expect("updated profile")
						.skin_provider,
					"fallback"
				);
			})
			.await;
		store.pool.close().await;
		tokio::fs::remove_dir_all(directory).await.expect("cleanup");
	}
	#[tokio::test]
	async fn skin_timeout_hostile_redirect_and_oversized_body_fail_safely() {
		let fixture = Fixture::new();
		let service = SkinService::new().expect("client");
		let client = reqwest::Client::builder()
			.redirect(reqwest::redirect::Policy::none())
			.timeout(Duration::from_millis(30))
			.build()
			.expect("mock client");
		TEST_SKIN_HTTP
			.scope((client, fixture.base.clone()), async {
				fixture.set(4);
				assert!(
					service
						.download("https://skinsystem.ely.by/skins/Test.png")
						.await
						.is_err()
				);
				assert_eq!(fixture.count(), 1, "untrusted redirect never requested");
				fixture.set(5);
				assert!(
					service
						.download("https://skinsystem.ely.by/skins/Test.png")
						.await
						.is_err()
				);
				fixture.set(3);
				let start = std::time::Instant::now();
				assert!(
					service
						.download("https://skinsystem.ely.by/skins/Test.png")
						.await
						.is_err()
				);
				assert!(
					start.elapsed() < Duration::from_millis(90),
					"configured request timeout bounds wait"
				);
			})
			.await;
	}

	#[tokio::test]
	#[ignore = "Live official Ely.by service; run explicitly for network evidence"]
	async fn live_ely_skin_is_public_and_decodes_with_current_redirect_policy() {
		let service = SkinService::new().expect("strict HTTPS client");
		let bytes = service
			.download("https://skinsystem.ely.by/skins/ErickSkrauch.png")
			.await
			.expect("official Ely lookup")
			.expect("known official example has skin");
		let skin = render(&bytes, "ely_by", "ready").expect("valid live skin");
		assert_eq!(skin.provider, "ely_by");
		assert!(skin.head.is_some());
	}
	#[tokio::test]
	#[ignore = "Live official Ely.by service; run explicitly for network evidence"]
	async fn live_ely_absent_nickname_returns_missing_and_local_fallback() {
		let nickname = format!("NC{}", &uuid::Uuid::new_v4().simple().to_string()[..14]);
		let service = SkinService::new().expect("strict HTTPS client");
		assert!(
			service
				.download(&format!("https://skinsystem.ely.by/skins/{nickname}.png"))
				.await
				.expect("official absent skin lookup")
				.is_none()
		);
		assert_eq!(
			fallback("missing").expect("local fallback").provider,
			"fallback"
		);
	}
}
