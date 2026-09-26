//! Opens no browser and prints no tokens, OAuth state, code, or verifier.
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let flow = ncreate_app_lib::AuthEngine::new().begin().await?;
	let url = url::Url::parse(&flow.auth_request_uri)?;
	if url.scheme() != "https" {
		return Err("OAuth URL must use HTTPS".into());
	}
	println!(
		"Microsoft OAuth initiation: HTTPS, host {}",
		url.host_str().unwrap_or("unknown")
	);
	println!("Interactive authentication is required to finish; no credentials were saved.");
	Ok(())
}
