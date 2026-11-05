/// Build an Explorer HTTP client with the caller's timeouts, redirects and TLS settings.
/// Browsers negotiate their own connection settings.
pub fn http_client_builder() -> reqwest::ClientBuilder {
    let builder = reqwest::Client::builder();
    #[cfg(not(target_arch = "wasm32"))]
    let builder = builder
        .http2_adaptive_window(true)
        .http2_keep_alive_interval(std::time::Duration::from_secs(15))
        .http2_keep_alive_timeout(std::time::Duration::from_secs(5));
    builder
}
