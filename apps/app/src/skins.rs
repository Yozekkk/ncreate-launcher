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
		let cached = tokio::fs::read(&cache).await.ok();
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
		let url = if account.kind == "offline" {
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
			Ok(None) => fallback("missing"),
			Err(_) => {
				if let Some(bytes) = cached {
					render(&bytes, provider, "network_error").or_else(|_| fallback("network_error"))
				} else {
					fallback("network_error")
				}
			}
		}
	}
	async fn download(&self, url: &str) -> Result<Option<Vec<u8>>> {
		let mut url = url::Url::parse(url).map_err(|_| "Некорректный адрес скина".to_string())?;
		for _ in 0..4 {
			if !allowed_skin_url(&url) {
				return Err("Недопустимый адрес скина".into());
			}
			let response = self
				.client
				.get(url.clone())
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
fn allowed_skin_url(url: &url::Url) -> bool {
	url.scheme() == "https"
		&& url.port_or_known_default() == Some(443)
		&& url.username().is_empty()
		&& url.password().is_none()
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
}
