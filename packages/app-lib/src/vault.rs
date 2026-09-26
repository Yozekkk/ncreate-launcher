//! Native OS vault only: no fallback to plaintext files or WebView storage.
use crate::Result;
use serde::{Serialize, de::DeserializeOwned};
const SERVICE: &str = "com.ncreate.launcher";

pub(crate) async fn load<T: DeserializeOwned + Send + 'static>(name: &str) -> Result<Option<T>> {
	let name = name.to_owned();
	tokio::task::spawn_blocking(move || {
		let entry = keyring::Entry::new(SERVICE, &name)?;
		match entry.get_password() {
			Ok(value) => Ok(Some(serde_json::from_str(&value)?)),
			Err(keyring::Error::NoEntry) => Ok(None),
			Err(error) => Err(error.into()),
		}
	})
	.await?
}
pub(crate) async fn save<T: Serialize>(name: &str, value: &T) -> Result<()> {
	let name = name.to_owned();
	let value = serde_json::to_string(value)?;
	tokio::task::spawn_blocking(move || {
		keyring::Entry::new(SERVICE, &name)?.set_password(&value)?;
		Ok(())
	})
	.await?
}
pub(crate) async fn remove(name: &str) -> Result<()> {
	let name = name.to_owned();
	tokio::task::spawn_blocking(move || {
		match keyring::Entry::new(SERVICE, &name)?.delete_credential() {
			Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
			Err(error) => Err(error.into()),
		}
	})
	.await?
}
