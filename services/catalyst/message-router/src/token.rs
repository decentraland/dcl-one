//! The join token, asked of the gatekeeper on every connection: `POST
//! /internal/message-router/token/v1`, signed with `MESSAGE_ROUTER_KEY` (`shared::message_router`).
//! The gatekeeper answers the signaling URL, the token, the room and the identity it minted for
//! this replica; the router learns its own identity from the answer.

use std::time::Duration;

use crate::shared::message_router::{
    self as contract, HEADER_NONCE, HEADER_SIGNATURE, HEADER_TIMESTAMP, TOKEN_PATH, TokenRequest,
    TokenResponse,
};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
/// The answer is a few hundred bytes; anything far larger is not the gatekeeper's.
const MAX_RESPONSE: usize = 16 * 1024;

pub struct TokenClient {
    client: reqwest::Client,
    url: url::Url,
    key: [u8; 32],
    replica: u16,
}

impl TokenClient {
    /// `url` is the gatekeeper's token route ([`check_url`]).
    pub fn new(url: url::Url, key: [u8; 32], replica: u16) -> anyhow::Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self {
            client,
            url,
            key,
            replica,
        })
    }

    pub async fn fetch(&self) -> anyhow::Result<TokenResponse> {
        let body = serde_json::to_vec(&TokenRequest {
            replica: self.replica,
        })?;
        let mut nonce = [0u8; 16];
        getrandom::fill(&mut nonce).map_err(|e| anyhow::anyhow!("nonce: {e}"))?;
        let nonce = contract::nonce(nonce);
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs()
            .to_string();
        let signature = contract::sign(&self.key, &timestamp, &nonce, &body);
        let response = self
            .client
            .post(self.url.clone())
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .header(HEADER_TIMESTAMP, timestamp)
            .header(HEADER_NONCE, nonce)
            .header(HEADER_SIGNATURE, signature)
            .body(body)
            .send()
            .await?;
        let status = response.status();
        let bytes = response.bytes().await?;
        anyhow::ensure!(
            bytes.len() <= MAX_RESPONSE,
            "the token route answered {} bytes",
            bytes.len()
        );
        anyhow::ensure!(
            status == reqwest::StatusCode::OK,
            "the token route answered {status}: {}",
            String::from_utf8_lossy(&bytes)
        );
        let answer: TokenResponse = serde_json::from_slice(&bytes)?;
        anyhow::ensure!(
            !answer.token.is_empty() && !answer.url.is_empty() && !answer.identity.is_empty(),
            "the token route answered an empty token, URL or identity"
        );
        Ok(answer)
    }
}

/// The gatekeeper's token route: its exact path, HTTPS, or HTTP to a loopback or private-network
/// address (an API's private address beside the router), no credentials, query or fragment.
pub fn check_url(raw: &str) -> anyhow::Result<url::Url> {
    let url = url::Url::parse(raw.trim())
        .map_err(|e| anyhow::anyhow!("MESSAGE_ROUTER_TOKEN_URL is not a URL: {e}"))?;
    let private = match url.host() {
        Some(url::Host::Ipv4(ip)) => ip.is_loopback() || ip.is_private(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback() || ip.is_unique_local(),
        Some(url::Host::Domain(name)) => name == "localhost",
        None => false,
    };
    anyhow::ensure!(
        url.scheme() == "https" || (url.scheme() == "http" && private),
        "MESSAGE_ROUTER_TOKEN_URL must be https, or http to a loopback or private address"
    );
    anyhow::ensure!(
        url.path() == TOKEN_PATH
            && url.query().is_none()
            && url.fragment().is_none()
            && url.username().is_empty()
            && url.password().is_none(),
        "MESSAGE_ROUTER_TOKEN_URL must name {TOKEN_PATH} with no query, fragment or credentials"
    );
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_token_url_is_the_exact_route_over_https_or_a_private_http_address() {
        for good in [
            "https://node.example.org/internal/message-router/token/v1",
            "http://127.0.0.1:5210/internal/message-router/token/v1",
            "http://10.0.3.7:5210/internal/message-router/token/v1",
            "http://[::1]:5210/internal/message-router/token/v1",
            "http://localhost:5210/internal/message-router/token/v1",
        ] {
            assert!(check_url(good).is_ok(), "{good}");
        }
        for bad in [
            "http://node.example.org/internal/message-router/token/v1",
            "http://203.0.113.9/internal/message-router/token/v1",
            "https://node.example.org/internal/message-router/token",
            "https://node.example.org/internal/message-router/token/v1?x=1",
            "https://user:pw@node.example.org/internal/message-router/token/v1",
            "ftp://127.0.0.1/internal/message-router/token/v1",
            "not a url",
        ] {
            assert!(check_url(bad).is_err(), "{bad}");
        }
    }

    /// The client signs what the gatekeeper's contract verifies.
    #[tokio::test]
    async fn the_client_signs_a_replica_request_and_reads_the_answer() {
        use axum::http::HeaderMap;
        let key = [5u8; 32];
        let app = axum::Router::new().route(
            TOKEN_PATH,
            axum::routing::post(
                move |headers: HeaderMap, body: axum::body::Bytes| async move {
                    use axum::response::IntoResponse;
                    let header =
                        |name: &str| headers.get(name).unwrap().to_str().unwrap().to_owned();
                    if !contract::verify(
                        &key,
                        &header(HEADER_TIMESTAMP),
                        &header(HEADER_NONCE),
                        &body,
                        &header(HEADER_SIGNATURE),
                    ) {
                        return axum::http::StatusCode::UNAUTHORIZED.into_response();
                    }
                    let request: TokenRequest = serde_json::from_slice(&body).unwrap();
                    axum::Json(TokenResponse {
                        url: "ws://127.0.0.1:7880".into(),
                        token: "jwt".into(),
                        room: "private-messages".into(),
                        identity: contract::identity("message-router-dev", request.replica),
                    })
                    .into_response()
                },
            ),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let url = check_url(&format!("http://{address}{TOKEN_PATH}")).unwrap();
        let answer = TokenClient::new(url.clone(), key, 2)
            .unwrap()
            .fetch()
            .await
            .unwrap();
        assert_eq!(answer.identity, "message-router-dev-2");
        assert_eq!(answer.room, "private-messages");
        // a request the gatekeeper refuses is an error
        assert!(
            TokenClient::new(url, [6; 32], 0)
                .unwrap()
                .fetch()
                .await
                .is_err()
        );
    }
}
