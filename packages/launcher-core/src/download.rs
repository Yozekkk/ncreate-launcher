//! Bounded HTTPS downloads shared by content, game files and edition updates.
use crate::{Error, Progress, Result};
use futures_util::StreamExt;
use sha2::{Digest, Sha512};
use std::{
	collections::HashMap,
	path::Path,
	sync::{
		Arc, Mutex,
		atomic::{AtomicBool, Ordering},
	},
	time::Duration,
};
use tokio::{io::AsyncWriteExt, sync::Semaphore};
#[derive(Default)]
struct TransferStatistics {
	started: Option<std::time::Instant>,
	received: u64,
	files: HashMap<String, (u64, u64)>,
}
#[derive(Clone)]
pub struct Operation {
	state: Arc<Mutex<Progress>>,
	cancelled: Arc<AtomicBool>,
	notify: Arc<tokio::sync::Notify>,
	transfers: Arc<Mutex<TransferStatistics>>,
}
impl Operation {
	pub fn id(&self) -> String {
		self.state
			.lock()
			.unwrap_or_else(|e| e.into_inner())
			.id
			.clone()
	}
	pub fn check(&self) -> Result<()> {
		if self.cancelled.load(Ordering::Relaxed) {
			Err(Error::Cancelled)
		} else {
			Ok(())
		}
	}
	async fn cancelled(&self) {
		loop {
			let notified = self.notify.notified();
			tokio::pin!(notified);
			notified.as_mut().enable();
			if self.cancelled.load(Ordering::Relaxed) {
				return;
			}
			notified.await;
		}
	}
	async fn wait<T>(&self, future: impl std::future::Future<Output = Result<T>>) -> Result<T> {
		tokio::select! { biased; _ = self.cancelled() => Err(Error::Cancelled), result = future => result }
	}
	pub fn progress(&self, phase: &str, completed: u64, total: u64, message: &str) {
		let mut s = self.state.lock().unwrap_or_else(|e| e.into_inner());
		s.phase = phase.into();
		s.completed = completed;
		s.total = total;
		s.bytes_per_second = 0;
		s.message = message.into();
	}
	pub(crate) fn edition_files_remaining(&self, remaining: usize, total: usize) {
		self.state.lock().unwrap_or_else(|e| e.into_inner()).message =
			format!("Осталось файлов: {remaining} из {total}");
	}
	fn register_transfer(&self, key: &str, size: u64) {
		let mut stats = self.transfers.lock().unwrap_or_else(|e| e.into_inner());
		stats.started.get_or_insert_with(std::time::Instant::now);
		stats.files.entry(key.into()).or_insert((0, size));
	}
	fn transferred(&self, key: &str, count: u64, received: u64, total: u64) {
		let mut stats = self.transfers.lock().unwrap_or_else(|e| e.into_inner());
		stats.received = stats.received.saturating_add(received);
		let file = stats.files.entry(key.into()).or_default();
		file.0 = file.0.max(count);
		file.1 = file.1.max(total).max(file.0);
		let completed = stats
			.files
			.values()
			.fold(0u64, |sum, (n, _)| sum.saturating_add(*n));
		let total = stats
			.files
			.values()
			.fold(0u64, |sum, (_, n)| sum.saturating_add(*n));
		let speed = stats.started.map_or(0, |start| {
			(stats.received as f64 / start.elapsed().as_secs_f64().max(0.001)) as u64
		});
		let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
		state.phase = "downloading".into();
		state.completed = completed;
		state.total = total;
		state.bytes_per_second = speed;
		if !matches!(
			state.operation.as_str(),
			"install_edition" | "update_edition"
		) {
			state.message = "Downloading verified game files".into();
		}
	}
	fn checking(&self) {
		let mut progress = self.state.lock().unwrap_or_else(|e| e.into_inner());
		if matches!(progress.phase.as_str(), "queued" | "checking") {
			progress.phase = "checking".into();
			progress.message = "Checking cached game files".into();
			progress.completed = 0;
			progress.total = 0;
			progress.bytes_per_second = 0;
		}
	}
	pub fn finish<T>(&self, result: &Result<T>) {
		let mut s = self.state.lock().unwrap_or_else(|e| e.into_inner());
		s.cancellable = false;
		s.bytes_per_second = 0;
		s.phase = match result {
			Ok(_) => "completed",
			Err(Error::Cancelled) => "cancelled",
			Err(_) => "failed",
		}
		.into();
		s.error = result.as_ref().err().map(ToString::to_string);
	}
	pub fn snapshot(&self) -> Progress {
		let mut progress = self.state.lock().unwrap_or_else(|e| e.into_inner()).clone();
		if progress.phase == "downloading" {
			let stats = self.transfers.lock().unwrap_or_else(|e| e.into_inner());
			progress.bytes_per_second = stats.started.map_or(0, |start| {
				(stats.received as f64 / start.elapsed().as_secs_f64().max(0.001)) as u64
			});
		}
		progress
	}
	pub(crate) fn committing(&self) {
		let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
		state.cancellable = false;
		state.phase = "committing".into();
		state.bytes_per_second = 0;
		state.message = "Сохраняем проверенные файлы".into();
	}
}
#[derive(Clone)]
pub struct DownloadManager {
	trusted: Arc<Mutex<std::collections::HashSet<String>>>,
	clients: Arc<Mutex<HashMap<String, reqwest::Client>>>,
	slots: Arc<Semaphore>,
	operations: Arc<Mutex<HashMap<String, Operation>>>,
	#[cfg(test)]
	fixtures: Arc<Mutex<HashMap<String, Vec<u8>>>>,
	#[cfg(test)]
	requests: Arc<Mutex<HashMap<String, u32>>>,
}
impl DownloadManager {
	pub fn new() -> Result<Self> {
		Ok(Self {
			trusted: Arc::new(Mutex::new(std::collections::HashSet::new())),
			clients: Arc::new(Mutex::new(HashMap::new())),
			slots: Arc::new(Semaphore::new(6)),
			operations: Arc::new(Mutex::new(HashMap::new())),
			#[cfg(test)]
			fixtures: Arc::new(Mutex::new(HashMap::new())),
			#[cfg(test)]
			requests: Arc::new(Mutex::new(HashMap::new())),
		})
	}
	pub fn begin(&self, kind: &str, instance: Option<&str>) -> Operation {
		let id = uuid::Uuid::new_v4().to_string();
		let op = Operation {
			state: Arc::new(Mutex::new(Progress {
				id: id.clone(),
				instance_id: instance.map(str::to_owned),
				operation: kind.into(),
				phase: "queued".into(),
				completed: 0,
				total: 0,
				bytes_per_second: 0,
				message: String::new(),
				cancellable: true,
				error: None,
			})),
			cancelled: Arc::new(AtomicBool::new(false)),
			notify: Arc::new(tokio::sync::Notify::new()),
			transfers: Arc::new(Mutex::new(TransferStatistics::default())),
		};
		let mut jobs = self.operations.lock().unwrap_or_else(|e| e.into_inner());
		if jobs.len() > 200 {
			jobs.retain(|_, v| v.snapshot().cancellable);
		}
		jobs.insert(id, op.clone());
		op
	}
	pub fn trust_manifest_origin(&self, url: &str) -> Result<()> {
		let parsed =
			reqwest::Url::parse(url).map_err(|_| Error::Invalid("invalid manifest URL".into()))?;
		validate_url(&parsed)?;
		self.trusted
			.lock()
			.unwrap_or_else(|e| e.into_inner())
			.insert(
				parsed
					.host_str()
					.ok_or_else(|| Error::Invalid("manifest URL has no host".into()))?
					.into(),
			);
		Ok(())
	}
	pub fn jobs(&self) -> Vec<Progress> {
		self.operations
			.lock()
			.unwrap_or_else(|e| e.into_inner())
			.values()
			.map(Operation::snapshot)
			.collect()
	}
	pub fn cancel(&self, id: &str) -> Result<()> {
		let jobs = self.operations.lock().unwrap_or_else(|e| e.into_inner());
		let op = jobs
			.get(id)
			.ok_or_else(|| Error::Invalid("operation not found".into()))?;
		if !op.snapshot().cancellable {
			return Err(Error::Invalid(
				"operation is committing or already finished".into(),
			));
		}
		op.cancelled.store(true, Ordering::Relaxed);
		op.notify.notify_waiters();
		Ok(())
	}
	async fn response(&self, url: &str) -> Result<reqwest::Response> {
		self.request(url, None).await
	}
	async fn request(
		&self,
		url: &str,
		body: Option<&serde_json::Value>,
	) -> Result<reqwest::Response> {
		let mut url =
			reqwest::Url::parse(url).map_err(|_| Error::Invalid("invalid download URL".into()))?;
		for _ in 0..5 {
			validate_url(&url)?;
			let host = url
				.host_str()
				.ok_or_else(|| Error::Invalid("URL has no host".into()))?;
			if !trusted_host(host)
				&& !self
					.trusted
					.lock()
					.unwrap_or_else(|e| e.into_inner())
					.contains(host)
			{
				return Err(Error::Invalid(
					"download source is not a trusted launcher origin".into(),
				));
			}
			let cached = self
				.clients
				.lock()
				.unwrap_or_else(|e| e.into_inner())
				.get(host)
				.cloned();
			let client = if let Some(client) = cached {
				client
			} else {
				let addresses: Vec<_> = tokio::net::lookup_host((host, 443)).await?.collect();
				if addresses.is_empty() || addresses.iter().any(|a| !public_ip(a.ip())) {
					return Err(Error::Invalid(
						"download origin resolved to a nonpublic address".into(),
					));
				}
				let client = reqwest::Client::builder()
					.https_only(true)
					.redirect(reqwest::redirect::Policy::none())
					.connect_timeout(Duration::from_secs(15))
					.timeout(Duration::from_secs(300))
					.user_agent(concat!("NCreate-Launcher/", env!("CARGO_PKG_VERSION")))
					.resolve_to_addrs(host, &addresses)
					.build()?;
				self.clients
					.lock()
					.unwrap_or_else(|e| e.into_inner())
					.insert(host.into(), client.clone());
				client
			};
			let response = if let Some(body) = body {
				client.post(url.clone()).json(body).send().await?
			} else {
				client.get(url.clone()).send().await?
			};
			if response.status().is_redirection() {
				if body.is_some() {
					return Err(Error::Invalid(
						"content identification cannot follow redirects".into(),
					));
				}
				let location = response
					.headers()
					.get(reqwest::header::LOCATION)
					.and_then(|v| v.to_str().ok())
					.ok_or_else(|| Error::Invalid("redirect without location".into()))?;
				url = url
					.join(location)
					.map_err(|_| Error::Invalid("invalid redirect".into()))?;
				continue;
			}
			return response.error_for_status().map_err(Into::into);
		}
		Err(Error::Invalid("too many HTTPS redirects".into()))
	}
	#[cfg(test)]
	pub(crate) fn fixture(&self, url: &str, bytes: Vec<u8>) {
		self.fixtures.lock().unwrap().insert(url.into(), bytes);
	}
	#[cfg(test)]
	pub(crate) fn fixture_count(&self, url: &str) -> u32 {
		*self.requests.lock().unwrap().get(url).unwrap_or(&0)
	}
	#[cfg(test)]
	fn fixture_bytes(&self, url: &str) -> Option<Vec<u8>> {
		let found = self.fixtures.lock().unwrap().get(url).cloned();
		if found.is_some() {
			*self.requests.lock().unwrap().entry(url.into()).or_default() += 1;
		}
		found
	}
	pub async fn bytes(&self, url: &str, limit: u64, op: &Operation) -> Result<Vec<u8>> {
		#[cfg(test)]
		if let Some(bytes) = self.fixture_bytes(url) {
			op.check()?;
			if bytes.len() as u64 > limit {
				return Err(Error::Invalid("download exceeds size limit".into()));
			}
			return Ok(bytes);
		}
		let _slot = op
			.wait(async { self.slots.acquire().await.map_err(|_| Error::Cancelled) })
			.await?;
		op.check()?;
		let response = op.wait(self.response(url)).await?;
		if response.content_length().is_some_and(|n| n > limit) {
			return Err(Error::Invalid("download exceeds size limit".into()));
		}
		let mut stream = response.bytes_stream();
		let mut bytes = Vec::new();
		while let Some(chunk) = op.wait(async { Ok(stream.next().await) }).await? {
			op.check()?;
			let chunk = chunk?;
			if bytes.len() as u64 + chunk.len() as u64 > limit {
				return Err(Error::Invalid("download exceeds size limit".into()));
			}
			bytes.extend_from_slice(&chunk);
		}
		Ok(bytes)
	}
	pub async fn json<T: serde::de::DeserializeOwned>(
		&self,
		url: &str,
		op: &Operation,
	) -> Result<T> {
		Ok(serde_json::from_slice(
			&self.bytes(url, 32 * 1024 * 1024, op).await?,
		)?)
	}
	pub(crate) async fn identify_hashes(
		&self,
		hashes: &[String],
		op: &Operation,
	) -> Result<HashMap<String, serde_json::Value>> {
		let url = "https://api.modrinth.com/v2/version_files";
		if hashes.len() > 256
			|| hashes
				.iter()
				.any(|hash| hash.len() != 128 || !hash.chars().all(|c| c.is_ascii_hexdigit()))
		{
			return Err(Error::Invalid(
				"invalid content identification batch".into(),
			));
		}
		op.check()?;
		#[cfg(test)]
		if let Some(bytes) = self.fixture_bytes(&format!("POST {url}")) {
			return Ok(serde_json::from_slice(&bytes)?);
		}
		let _slot = op
			.wait(async { self.slots.acquire().await.map_err(|_| Error::Cancelled) })
			.await?;
		let body = serde_json::json!({"hashes":hashes,"algorithm":"sha512"});
		let response = op.wait(self.request(url, Some(&body))).await?;
		let limit = 32 * 1024 * 1024u64;
		if response.content_length().is_some_and(|n| n > limit) {
			return Err(Error::Invalid(
				"identification response exceeds limit".into(),
			));
		}
		let mut stream = response.bytes_stream();
		let mut bytes = Vec::new();
		while let Some(chunk) = op.wait(async { Ok(stream.next().await) }).await? {
			let chunk = chunk?;
			if bytes.len() as u64 + chunk.len() as u64 > limit {
				return Err(Error::Invalid(
					"identification response exceeds limit".into(),
				));
			}
			bytes.extend_from_slice(&chunk);
		}
		Ok(serde_json::from_slice(&bytes)?)
	}

	pub async fn file(
		&self,
		url: &str,
		path: &Path,
		hash: &str,
		algorithm: &str,
		size: u64,
		op: &Operation,
	) -> Result<()> {
		let expected_len = match algorithm {
			"sha1" => 40,
			"sha256" => 64,
			"sha512" => 128,
			_ => return Err(Error::Invalid("unsupported checksum algorithm".into())),
		};
		if hash.len() != expected_len
			|| !hash.chars().all(|c| c.is_ascii_hexdigit())
			|| size > 2 * 1024 * 1024 * 1024
		{
			return Err(Error::Invalid("invalid checksum or file size".into()));
		}
		op.check()?;
		#[cfg(test)]
		if let Some(bytes) = self.fixture_bytes(url) {
			op.check()?;
			verify(&bytes, hash, algorithm)?;
			if size > 0 && bytes.len() as u64 != size {
				return Err(Error::Invalid("download size mismatch".into()));
			}
			if let Some(parent) = path.parent() {
				tokio::fs::create_dir_all(parent).await?;
			}
			tokio::fs::write(path, &bytes).await?;
			let key = path.to_string_lossy();
			op.register_transfer(&key, size.max(bytes.len() as u64));
			op.transferred(
				&key,
				bytes.len() as u64,
				bytes.len() as u64,
				size.max(bytes.len() as u64),
			);
			return Ok(());
		}
		if hash.is_empty() {
			return Err(Error::Invalid("download has no integrity hash".into()));
		}
		op.checking();
		match verify_file_inner(path, hash, algorithm, size, Some(op)).await {
			Ok(()) => return Ok(()),
			Err(Error::Cancelled) => return Err(Error::Cancelled),
			Err(_) => {}
		}
		if let Some(parent) = path.parent() {
			tokio::fs::create_dir_all(parent).await?;
		}
		let temporary = path.with_extension(format!("{}.part", uuid::Uuid::new_v4()));
		op.register_transfer(&temporary.to_string_lossy(), size);
		let result = self
			.stream_file(url, &temporary, hash, algorithm, size, op)
			.await;
		if result.is_ok() {
			if path.exists() {
				tokio::fs::remove_file(path).await?;
			}
			tokio::fs::rename(&temporary, path).await?;
		} else {
			let _ = tokio::fs::remove_file(&temporary).await;
		}
		result
	}
	async fn stream_file(
		&self,
		url: &str,
		path: &Path,
		hash: &str,
		algorithm: &str,
		size: u64,
		op: &Operation,
	) -> Result<()> {
		for attempt in 0..3 {
			let result = self.stream_once(url, path, hash, algorithm, size, op).await;
			match result {
				Err(Error::Network(_)) if attempt < 2 => {
					op.wait(async {
						tokio::time::sleep(Duration::from_millis(300 * (attempt + 1))).await;
						Ok(())
					})
					.await?;
				}
				other => return other,
			}
		}
		Err(Error::Invalid("download retries exhausted".into()))
	}
	async fn stream_once(
		&self,
		url: &str,
		path: &Path,
		hash: &str,
		algorithm: &str,
		size: u64,
		op: &Operation,
	) -> Result<()> {
		op.check()?;
		let _slot = op
			.wait(async { self.slots.acquire().await.map_err(|_| Error::Cancelled) })
			.await?;
		let response = op.wait(self.response(url)).await?;
		let declared_total = if size > 0 {
			size
		} else {
			response.content_length().unwrap_or(0)
		};
		let transfer_key = path.to_string_lossy();
		op.register_transfer(&transfer_key, declared_total);
		let limit = if size > 0 {
			size
		} else {
			2 * 1024 * 1024 * 1024
		};
		if response.content_length().is_some_and(|n| n > limit) {
			return Err(Error::Invalid("download larger than declared size".into()));
		}
		let mut output = tokio::fs::File::create(path).await?;
		let mut stream = response.bytes_stream();
		let mut digest = FileDigest::new(algorithm)?;
		let mut count = 0u64;
		while let Some(chunk) = op.wait(async { Ok(stream.next().await) }).await? {
			op.check()?;
			let chunk = chunk?;
			count = count
				.checked_add(chunk.len() as u64)
				.ok_or_else(|| Error::Invalid("download size overflow".into()))?;
			if count > limit {
				return Err(Error::Invalid("download larger than declared size".into()));
			}
			output.write_all(&chunk).await?;
			digest.update(&chunk);
			op.transferred(&transfer_key, count, chunk.len() as u64, declared_total);
		}
		output.sync_all().await?;
		if size > 0 && size != count {
			return Err(Error::Invalid("download size mismatch".into()));
		}
		let actual = digest.finish();
		if actual != hash.to_ascii_lowercase() {
			return Err(Error::Invalid("download hash mismatch".into()));
		}
		Ok(())
	}
}
pub(crate) fn validate_url(url: &reqwest::Url) -> Result<()> {
	let host = url
		.host_str()
		.ok_or_else(|| Error::Invalid("URL has no host".into()))?;
	if url.scheme() != "https"
		|| url.port_or_known_default() != Some(443)
		|| !url.username().is_empty()
		|| url.password().is_some()
		|| host.parse::<std::net::IpAddr>().is_ok()
		|| host == "localhost"
		|| !host.contains('.')
		|| host.ends_with(".local")
		|| host.ends_with(".internal")
	{
		return Err(Error::Invalid(
			"only public HTTPS download URLs are allowed".into(),
		));
	}
	Ok(())
}
pub(crate) fn verify(bytes: &[u8], hash: &str, algorithm: &str) -> Result<()> {
	let actual = match algorithm {
		"sha1" => hex::encode(sha1::Sha1::digest(bytes)),
		"sha256" => hex::encode(sha2::Sha256::digest(bytes)),
		"sha512" => hex::encode(Sha512::digest(bytes)),
		_ => return Err(Error::Invalid("unsupported hash algorithm".into())),
	};
	if actual == hash.to_ascii_lowercase() {
		Ok(())
	} else {
		Err(Error::Invalid("file integrity mismatch".into()))
	}
}
pub(crate) fn hash(bytes: &[u8]) -> String {
	hex::encode(Sha512::digest(bytes))
}
#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn rejects_local_and_plaintext_urls() {
		for u in [
			"http://cdn.modrinth.com/x",
			"https://127.0.0.1/x",
			"https://localhost/x",
			"https://user:secret@example.com/x",
			"https://example.com:8443/x",
		] {
			assert!(validate_url(&reqwest::Url::parse(u).unwrap()).is_err());
		}
	}
	#[test]
	fn hash_mismatch_fails() {
		assert!(verify(b"payload", "00", "sha512").is_err());
		assert!(verify(b"payload", &hash(b"payload"), "sha512").is_ok());
	}
	#[test]
	fn cancellation_is_visible() {
		let d = DownloadManager::new().unwrap();
		let op = d.begin("test", None);
		d.cancel(&op.id()).unwrap();
		assert!(matches!(op.check(), Err(Error::Cancelled)));
	}
}

pub(crate) fn trusted_host(host: &str) -> bool {
	matches!(
		host,
		"api.modrinth.com"
			| "cdn.modrinth.com"
			| "launcher-meta.modrinth.com"
			| "launchermeta.mojang.com"
			| "piston-meta.mojang.com"
			| "piston-data.mojang.com"
			| "resources.download.minecraft.net"
			| "libraries.minecraft.net"
			| "launcher.mojang.com"
			| "meta.fabricmc.net"
			| "maven.fabricmc.net"
			| "meta.quiltmc.org"
			| "maven.quiltmc.org"
			| "maven.minecraftforge.net"
			| "maven.neoforged.net"
			| "maven.ftb.dev"
			| "mediafilez.forgecdn.net"
			| "api.github.com"
			| "github.com"
			| "raw.githubusercontent.com"
			| "objects.githubusercontent.com"
			| "release-assets.githubusercontent.com"
			| "repo.maven.apache.org"
			| "repo1.maven.org"
	)
}
pub(crate) fn validate_content_url(value: &str) -> Result<()> {
	let url =
		reqwest::Url::parse(value).map_err(|_| Error::Invalid("invalid content URL".into()))?;
	validate_url(&url)?;
	if !matches!(
		url.host_str(),
		Some(
			"cdn.modrinth.com"
				| "api.github.com"
				| "github.com"
				| "raw.githubusercontent.com"
				| "objects.githubusercontent.com"
				| "release-assets.githubusercontent.com"
		)
	) {
		return Err(Error::Invalid(
			"modpack file source is not permitted".into(),
		));
	}
	Ok(())
}
fn public_ip(ip: std::net::IpAddr) -> bool {
	match ip {
		std::net::IpAddr::V4(v) => {
			!v.is_private()
				&& !v.is_loopback()
				&& !v.is_link_local()
				&& !v.is_unspecified()
				&& !v.is_broadcast()
				&& !v.is_documentation()
				&& !v.is_multicast()
				&& v.octets()[0] != 0
				&& v.octets()[0] < 224
				&& !(v.octets()[0] == 100 && (64..=127).contains(&v.octets()[1]))
				&& !(v.octets()[0] == 198 && (18..=19).contains(&v.octets()[1]))
				&& !(v.octets()[0] == 192 && v.octets()[1] == 0 && v.octets()[2] == 0)
		}
		std::net::IpAddr::V6(v) => {
			(v.segments()[0] & 0xe000) == 0x2000
				&& v.segments()[0] != 0x2002
				&& !(v.segments()[0] == 0x2001 && v.segments()[1] < 0x0200)
				&& v.segments()[0] != 0x3fff
				&& !v.is_loopback()
				&& !v.is_unspecified()
				&& !v.is_multicast()
				&& (v.segments()[0] & 0xfe00) != 0xfc00
				&& (v.segments()[0] & 0xffc0) != 0xfe80
				&& (v.segments()[0] & 0xffc0) != 0xfec0
				&& !(v.segments()[0] == 0x2001 && v.segments()[1] == 0x0db8)
				&& v.to_ipv4_mapped()
					.is_none_or(|v| public_ip(std::net::IpAddr::V4(v)))
		}
	}
}
enum FileDigest {
	Sha1(sha1::Sha1),
	Sha256(sha2::Sha256),
	Sha512(Sha512),
}
impl FileDigest {
	fn new(algorithm: &str) -> Result<Self> {
		match algorithm {
			"sha1" => Ok(Self::Sha1(sha1::Sha1::new())),
			"sha256" => Ok(Self::Sha256(sha2::Sha256::new())),
			"sha512" => Ok(Self::Sha512(Sha512::new())),
			_ => Err(Error::Invalid("unsupported checksum algorithm".into())),
		}
	}
	fn update(&mut self, bytes: &[u8]) {
		match self {
			Self::Sha1(d) => d.update(bytes),
			Self::Sha256(d) => d.update(bytes),
			Self::Sha512(d) => d.update(bytes),
		}
	}
	fn finish(self) -> String {
		match self {
			Self::Sha1(d) => hex::encode(d.finalize()),
			Self::Sha256(d) => hex::encode(d.finalize()),
			Self::Sha512(d) => hex::encode(d.finalize()),
		}
	}
}
pub(crate) async fn verify_file(path: &Path, hash: &str, algorithm: &str, size: u64) -> Result<()> {
	verify_file_inner(path, hash, algorithm, size, None).await
}
async fn verify_file_inner(
	path: &Path,
	hash: &str,
	algorithm: &str,
	size: u64,
	op: Option<&Operation>,
) -> Result<()> {
	if digest_file(path, algorithm, size, op).await? == hash.to_ascii_lowercase() {
		Ok(())
	} else {
		Err(Error::Invalid("cached file hash mismatch".into()))
	}
}
pub(crate) async fn hash_file(path: &Path) -> Result<String> {
	digest_file(path, "sha512", 0, None).await
}
async fn digest_file(
	path: &Path,
	algorithm: &str,
	size: u64,
	op: Option<&Operation>,
) -> Result<String> {
	use tokio::io::AsyncReadExt;
	let metadata = tokio::fs::metadata(path).await?;
	if (size > 0 && metadata.len() != size) || metadata.len() > 2 * 1024 * 1024 * 1024 {
		return Err(Error::Invalid("cached file size mismatch".into()));
	}
	let mut file = tokio::fs::File::open(path).await?;
	let mut buffer = vec![0u8; 65536];
	let mut digest = FileDigest::new(algorithm)?;
	loop {
		if let Some(op) = op {
			op.check()?;
		}
		let n = file.read(&mut buffer).await?;
		if n == 0 {
			break;
		}
		digest.update(&buffer[..n]);
	}
	Ok(digest.finish())
}

#[cfg(test)]
mod cancellation_tests {
	use super::*;
	#[test]
	fn aggregated_transfer_counters_do_not_reset_between_files_or_retries() {
		let manager = DownloadManager::new().unwrap();
		let op = manager.begin("downloads", None);
		op.register_transfer("a", 100);
		op.register_transfer("b", 50);
		op.transferred("a", 80, 80, 100);
		assert_eq!(op.snapshot().completed, 80);
		op.transferred("b", 20, 20, 50);
		assert_eq!(op.snapshot().completed, 100);
		assert_eq!(op.snapshot().total, 150);
		op.transferred("a", 40, 40, 100);
		assert_eq!(op.snapshot().completed, 100);
		op.transferred("a", 100, 60, 100);
		op.transferred("b", 50, 30, 50);
		assert_eq!(op.snapshot().completed, 150);
		let stats = op.transfers.lock().unwrap();
		assert_eq!(stats.received, 230);
		drop(stats);
		assert!(op.snapshot().bytes_per_second > 0);
		op.progress("loader_processors", 1, 2, "Preparing");
		assert_eq!(op.snapshot().bytes_per_second, 0);
		op.finish(&Ok(()));
		assert_eq!(op.snapshot().bytes_per_second, 0);
	}
	#[tokio::test]
	async fn pending_request_and_semaphore_wait_cancel_promptly() {
		let manager = DownloadManager::new().unwrap();
		let op = manager.begin("blocked", None);
		let cancel = manager.clone();
		let id = op.id();
		tokio::spawn(async move {
			tokio::time::sleep(Duration::from_millis(10)).await;
			cancel.cancel(&id).unwrap();
		});
		assert!(matches!(
			tokio::time::timeout(
				Duration::from_secs(1),
				op.wait(std::future::pending::<Result<()>>())
			)
			.await
			.unwrap(),
			Err(Error::Cancelled)
		));
		let permits = manager.slots.acquire_many(6).await.unwrap();
		let op = manager.begin("queued", None);
		let cancel = manager.clone();
		let id = op.id();
		tokio::spawn(async move {
			tokio::time::sleep(Duration::from_millis(10)).await;
			cancel.cancel(&id).unwrap();
		});
		assert!(matches!(
			tokio::time::timeout(
				Duration::from_secs(1),
				manager.bytes("https://cdn.modrinth.com/not-requested", 5, &op)
			)
			.await
			.unwrap(),
			Err(Error::Cancelled)
		));
		drop(permits);
	}
	#[test]
	fn nonpublic_dns_ranges_rejected() {
		for value in [
			"0.0.0.0",
			"10.0.0.1",
			"127.0.0.1",
			"100.64.0.1",
			"169.254.1.1",
			"172.16.0.1",
			"192.168.1.1",
			"198.18.0.1",
			"224.0.0.1",
			"::",
			"::1",
			"::127.0.0.1",
			"::ffff:192.168.1.1",
			"fc00::1",
			"fe80::1",
			"2001:db8::1",
			"2002:7f00:1::1",
		] {
			assert!(!public_ip(value.parse().unwrap()), "{value}");
		}
		assert!(public_ip("1.1.1.1".parse().unwrap()));
		assert!(public_ip("2606:4700:4700::1111".parse().unwrap()));
	}
}

#[cfg(test)]
mod cached_integrity_tests {
	use super::*;
	#[tokio::test]
	async fn all_declared_algorithms_verify_cached_files_and_cache_phase_has_no_speed() {
		let dir = tempfile::tempdir().unwrap();
		let path = dir.path().join("cache.bin");
		let bytes = b"representative game artifact";
		tokio::fs::write(&path, bytes).await.unwrap();
		for (algorithm, hash) in [
			("sha1", hex::encode(sha1::Sha1::digest(bytes))),
			("sha256", hex::encode(sha2::Sha256::digest(bytes))),
			("sha512", hex::encode(Sha512::digest(bytes))),
		] {
			verify_file(&path, &hash, algorithm, bytes.len() as u64)
				.await
				.unwrap();
			let manager = DownloadManager::new().unwrap();
			let op = manager.begin("install_game", None);
			manager
				.file(
					"https://cdn.modrinth.com/not-requested",
					&path,
					&hash,
					algorithm,
					bytes.len() as u64,
					&op,
				)
				.await
				.unwrap();
			assert_eq!(op.snapshot().phase, "checking");
			assert_eq!(op.snapshot().bytes_per_second, 0);
			manager.cancel(&op.id()).unwrap();
			assert!(matches!(
				verify_file_inner(&path, &hash, algorithm, bytes.len() as u64, Some(&op)).await,
				Err(Error::Cancelled)
			));
		}
	}
	#[tokio::test]
	#[ignore = "manual representative debug-cache performance probe"]
	async fn benchmark_cached_hashing() {
		use tokio::io::AsyncReadExt;
		let dir = tempfile::tempdir().unwrap();
		let path = dir.path().join("representative-client.jar");
		let bytes = vec![42u8; 16 * 1024 * 1024];
		tokio::fs::write(&path, &bytes).await.unwrap();
		let expected = hex::encode(sha1::Sha1::digest(&bytes));
		drop(bytes);
		let started = std::time::Instant::now();
		let mut file = tokio::fs::File::open(&path).await.unwrap();
		let mut buffer = vec![0u8; 65536];
		let mut one = sha1::Sha1::new();
		let mut two = sha2::Sha256::new();
		let mut five = Sha512::new();
		loop {
			let n = file.read(&mut buffer).await.unwrap();
			if n == 0 {
				break;
			}
			one.update(&buffer[..n]);
			two.update(&buffer[..n]);
			five.update(&buffer[..n]);
		}
		assert_eq!(hex::encode(one.finalize()), expected);
		let legacy = started.elapsed();
		let started = std::time::Instant::now();
		verify_file(&path, &expected, "sha1", 16 * 1024 * 1024)
			.await
			.unwrap();
		let selected = started.elapsed();
		println!(
			"CACHE HASH PROBE 16 MiB debug: previous triple-digest {:?}, requested SHA1 {:?}, ratio {:.2}",
			legacy,
			selected,
			legacy.as_secs_f64() / selected.as_secs_f64()
		);
	}
}
