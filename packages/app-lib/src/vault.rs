//! Native OS vault only: no fallback to plaintext files or WebView storage.
use crate::Result;
use serde::{Serialize, de::DeserializeOwned};
use zeroize::Zeroizing;
const SERVICE: &str = "com.ncreate.launcher";

pub(crate) async fn load<T: DeserializeOwned + Send + 'static>(name: &str) -> Result<Option<T>> {
	#[cfg(test)]
	if let Ok(store) = TEST_VAULT.try_with(Clone::clone) {
		return store
			.lock()
			.expect("test vault lock")
			.get(name)
			.map(|v| serde_json::from_str(v))
			.transpose()
			.map_err(Into::into);
	}
	let name = name.to_owned();
	tokio::task::spawn_blocking(move || {
		let entry = keyring::Entry::new(SERVICE, &name)?;
		match entry.get_password() {
			Ok(value) => Ok(Some(serde_json::from_str(&Zeroizing::new(value))?)),
			Err(keyring::Error::NoEntry) => Ok(None),
			Err(error) => Err(error.into()),
		}
	})
	.await?
}
pub(crate) async fn save<T: Serialize>(name: &str, value: &T) -> Result<()> {
	#[cfg(test)]
	if TEST_VAULT_DENY_WRITE
		.try_with(|deny| *deny)
		.unwrap_or(false)
	{
		return Err(
			keyring::Error::NoStorageAccess(Box::new(std::io::Error::new(
				std::io::ErrorKind::PermissionDenied,
				"synthetic vault unavailable",
			)))
			.into(),
		);
	}
	let name = name.to_owned();
	let value = Zeroizing::new(serde_json::to_string(value)?);
	#[cfg(test)]
	if let Ok(store) = TEST_VAULT.try_with(Clone::clone) {
		store
			.lock()
			.expect("test vault lock")
			.insert(name, value.to_string());
		return Ok(());
	}
	tokio::task::spawn_blocking(move || {
		keyring::Entry::new(SERVICE, &name)?.set_password(&value)?;
		Ok(())
	})
	.await?
}
pub(crate) async fn remove(name: &str) -> Result<()> {
	#[cfg(test)]
	if let Ok(store) = TEST_VAULT.try_with(Clone::clone) {
		store.lock().expect("test vault lock").remove(name);
		return Ok(());
	}
	let name = name.to_owned();
	tokio::task::spawn_blocking(move || {
		match keyring::Entry::new(SERVICE, &name)?.delete_credential() {
			Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
			Err(error) => Err(error.into()),
		}
	})
	.await?
}

#[cfg(test)]
tokio::task_local! { pub(crate) static TEST_VAULT: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String,String>>>; }

#[cfg(test)]
tokio::task_local! { pub(crate) static TEST_VAULT_DENY_WRITE: bool; }
