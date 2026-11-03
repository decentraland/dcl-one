/// Build an Explorer HTTP client with the caller's timeouts, redirects and TLS settings.
/// Browsers negotiate their own connection settings.
pub fn http_client_builder() -> reqwest::ClientBuilder {
    let builder = reqwest::Client::builder();
    #[cfg(not(target_arch = "wasm32"))]
    let builder = builder
        .http2_keep_alive_interval(std::time::Duration::from_secs(15))
        .http2_keep_alive_timeout(std::time::Duration::from_secs(5));
    builder
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    #[tokio::test]
    #[ignore = "run node tooling/bevy-http2-test.mjs to start both local TLS servers"]
    async fn loopback_negotiates_http2_and_http11() {
        let client = super::http_client_builder()
            .danger_accept_invalid_certs(true) // Ephemeral, self-signed loopback servers.
            .no_proxy()
            .build()
            .unwrap();
        for (port_var, version) in [
            ("DCL_TEST_H2_PORT", reqwest::Version::HTTP_2),
            ("DCL_TEST_H1_PORT", reqwest::Version::HTTP_11),
        ] {
            let port: u16 = std::env::var(port_var).unwrap().parse().unwrap();
            let response = client
                .get(format!("https://127.0.0.1:{port}/"))
                .send()
                .await
                .unwrap();
            assert_eq!(response.version(), version);
            assert_eq!(response.text().await.unwrap(), "ready");
        }
    }
}
