//! Serializable launcher domain models. Credentials deliberately have no serializer.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Loader {
	Vanilla,
	Fabric,
	Forge,
	Neoforge,
	Quilt,
}
impl Loader {
	pub fn key(self) -> &'static str {
		match self {
			Self::Vanilla => "vanilla",
			Self::Fabric => "fabric",
			Self::Forge => "forge",
			Self::Neoforge => "neoforge",
			Self::Quilt => "quilt",
		}
	}
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Instance {
	pub id: String,
	pub name: String,
	pub game_version: String,
	pub loader: Loader,
	pub loader_version: Option<String>,
	pub kind: String,
	pub edition: Option<String>,
	pub manifest_version: Option<String>,
	pub status: String,
	pub memory_mb: u32,
	pub java_path: Option<String>,
	pub directory: String,
	#[serde(default)]
	pub icon: Option<String>,
	#[serde(default)]
	pub mod_count: u32,
	#[serde(default)]
	pub last_played: Option<i64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateInstance {
	pub name: String,
	pub game_version: String,
	pub loader: Loader,
	#[serde(default)]
	pub loader_version: Option<String>,
	#[serde(default = "default_memory")]
	pub memory_mb: u32,
	#[serde(default)]
	pub java_path: Option<String>,
}
fn default_memory() -> u32 {
	4096
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstalledContent {
	pub id: String,
	pub instance_id: String,
	pub project_id: Option<String>,
	pub version_id: Option<String>,
	pub name: String,
	pub kind: String,
	pub path: String,
	pub sha512: String,
	pub enabled: bool,
	pub managed: bool,
	pub source_url: Option<String>,
	#[serde(default)]
	pub version_number: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchRequest {
	#[serde(default)]
	pub query: String,
	#[serde(default = "default_kind")]
	pub kind: String,
	#[serde(default)]
	pub game_version: Option<String>,
	#[serde(default)]
	pub loader: Option<String>,
	#[serde(default)]
	pub offset: u32,
	#[serde(default)]
	pub category: Option<String>,
	#[serde(default = "default_sort")]
	pub sort: String,
	#[serde(default = "default_limit")]
	pub limit: u32,
	#[serde(default)]
	pub channel: Option<String>,
}
fn default_sort() -> String {
	"relevance".into()
}
fn default_limit() -> u32 {
	20
}
fn default_kind() -> String {
	"mod".into()
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchResult {
	pub hits: Vec<serde_json::Value>,
	pub offset: u32,
	pub total_hits: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContentInstall {
	pub instance_id: String,
	pub project_id: String,
	#[serde(default)]
	pub version_id: Option<String>,
	pub kind: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContentUpdate {
	pub content_id: String,
	pub current_version: String,
	pub next_version: String,
	pub name: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Progress {
	pub id: String,
	pub instance_id: Option<String>,
	pub operation: String,
	pub phase: String,
	pub completed: u64,
	pub total: u64,
	pub bytes_per_second: u64,
	pub message: String,
	pub cancellable: bool,
	pub error: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JavaRuntime {
	pub path: String,
	pub major: u32,
	pub architecture: String,
}
/// Backend-only credentials are injected by the auth boundary immediately before launch.
pub struct LaunchIdentity {
	pub nickname: String,
	pub uuid: String,
	pub access_token: String,
	pub user_type: String,
	pub xuid: Option<String>,
	pub authlib_injector: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunningGame {
	pub instance_id: String,
	pub pid: u32,
	pub started_at: i64,
	pub log_path: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestFile {
	pub path: String,
	pub url: String,
	pub sha256: String,
	pub size: u64,
	#[serde(default = "default_required")]
	pub required: bool,
	#[serde(default)]
	pub update_policy: UpdatePolicy,
}
fn default_required() -> bool {
	true
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UpdatePolicy {
	#[default]
	ManagedOnly,
	Preserve,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestLoader {
	pub kind: Loader,
	pub version: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManifestJava {
	pub major: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestMemory {
	pub minimum_mb: u32,
	pub recommended_mb: u32,
	pub maximum_mb: u32,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestLaunch {
	#[serde(default)]
	pub jvm_args: Vec<String>,
	#[serde(default)]
	pub game_args: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManifestServer {
	pub name: String,
	pub address: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditionManifest {
	pub schema_version: u32,
	pub id: String,
	pub version: String,
	pub minecraft: String,
	pub loader: ManifestLoader,
	pub files: Vec<ManifestFile>,
	pub java: ManifestJava,
	pub memory: ManifestMemory,
	#[serde(default)]
	pub launch: ManifestLaunch,
	#[serde(default)]
	pub servers: Vec<ManifestServer>,
	pub release_channel: String,
	#[serde(default)]
	pub changelog: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ManifestProviders {
	pub stable: BTreeMap<String, String>,
	pub beta: BTreeMap<String, String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EditionAvailability {
	pub id: String,
	pub channel: String,
	pub available: bool,
	pub manifest: Option<EditionManifest>,
	pub error: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdatePlan {
	pub instance_id: String,
	pub from_version: Option<String>,
	pub to_version: String,
	pub added: Vec<String>,
	pub changed: Vec<String>,
	pub removed: Vec<String>,
	pub conflicts: Vec<String>,
	pub changelog: String,
	pub download_bytes: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GameVersion {
	pub id: String,
	#[serde(rename = "type")]
	pub kind: String,
	pub release_time: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LoaderVersion {
	pub id: String,
	pub stable: bool,
}

impl Drop for LaunchIdentity {
	fn drop(&mut self) {
		use zeroize::Zeroize;
		self.access_token.zeroize();
	}
}
