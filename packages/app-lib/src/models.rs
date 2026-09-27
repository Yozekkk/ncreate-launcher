//! Authentication provider, game identity, skin source and credential reference are separate concepts.
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::Zeroizing;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AccountProvider {
	Microsoft,
	ElyBy,
	#[default]
	Offline,
}
impl AccountProvider {
	pub fn as_str(self) -> &'static str {
		match self {
			Self::Microsoft => "microsoft",
			Self::ElyBy => "ely_by",
			Self::Offline => "offline",
		}
	}
}
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkinProvider {
	Mojang,
	ElyBy,
	Fallback,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct GameIdentity {
	pub uuid: Uuid,
	pub nickname: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CredentialReference {
	pub provider: AccountProvider,
	pub key: String,
}
impl CredentialReference {
	pub fn new(provider: AccountProvider, uuid: Uuid) -> Option<Self> {
		match provider {
			AccountProvider::Offline => None,
			AccountProvider::Microsoft => Some(Self {
				provider,
				key: uuid.to_string(),
			}),
			AccountProvider::ElyBy => Some(Self {
				provider,
				key: format!("ely_by:{uuid}"),
			}),
		}
	}
}
/// Backend-only session: deliberately not Serialize; Debug never displays the token.
pub struct GameSession {
	pub identity: GameIdentity,
	pub provider: AccountProvider,
	token: Zeroizing<String>,
}
impl GameSession {
	pub(crate) fn new(identity: GameIdentity, provider: AccountProvider, token: String) -> Self {
		Self {
			identity,
			provider,
			token: Zeroizing::new(token),
		}
	}
	pub fn offline(identity: GameIdentity) -> Self {
		Self::new(identity, AccountProvider::Offline, "0".into())
	}
	pub fn token(&self) -> &str {
		&self.token
	}
}
impl std::fmt::Debug for GameSession {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("GameSession")
			.field("identity", &self.identity)
			.field("provider", &self.provider)
			.field("token", &"[redacted]")
			.finish()
	}
}
