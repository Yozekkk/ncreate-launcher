//! Round-trip a nonsecret temporary item in NCreate's native vault namespace.
fn main() -> Result<(), Box<dyn std::error::Error>> {
	let entry = keyring::Entry::new("com.ncreate.launcher", "verification-probe")?;
	entry.set_password("ncreate-vault-smoke-test")?;
	let result = entry.get_password();
	entry.delete_credential()?;
	if result? != "ncreate-vault-smoke-test" {
		return Err("vault round-trip mismatch".into());
	}
	println!("Native secure vault round-trip: PASS; temporary item deleted.");
	Ok(())
}
