//! Test injection is scoped per Tokio task and absent from production binaries.
use reqwest::{Method, RequestBuilder};
pub(crate) fn get(url: impl AsRef<str>) -> RequestBuilder {
	request(Method::GET, url.as_ref())
}
pub(crate) fn post(url: impl AsRef<str>) -> RequestBuilder {
	request(Method::POST, url.as_ref())
}
fn request(method: Method, url: &str) -> RequestBuilder {
	#[cfg(test)]
	if let Ok((client, base)) = TEST_HTTP.try_with(Clone::clone) {
		let parsed = url::Url::parse(url).expect("auth endpoint URL");
		let target = format!(
			"{}/{}{}{}",
			base,
			parsed.host_str().expect("endpoint host"),
			parsed.path(),
			parsed.query().map(|q| format!("?{q}")).unwrap_or_default()
		);
		return client.request(method, target);
	}
	crate::HTTP_CLIENT.request(method, url)
}
#[cfg(test)]
tokio::task_local! { pub(crate) static TEST_HTTP: (reqwest::Client, String); }
