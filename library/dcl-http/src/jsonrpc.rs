//! One JSON-RPC 2.0 round trip for every chain reader: the request body, the
//! timeout, the body cap and the answer's shape are decided here once. The
//! endpoint is an operator-configured URL that may carry a gateway key in
//! its path, so no error names it. A public endpoint answers a refused
//! `eth_getLogs` range with HTTP 400 and the JSON-RPC error (and its range
//! hint) in the body, so a non-2xx status is a transport failure only when
//! the body is not a JSON-RPC answer ([`parse_envelope`]).

use std::time::Duration;

use serde_json::Value;

use super::{Outbound, SendError, read_body_capped};

mod wire;

pub use wire::*;

/// The client over one endpoint through the guarded [`Outbound`].
pub struct Rpc<'a> {
    http: &'a Outbound,
    url: &'a str,
    timeout: Duration,
    body_cap: usize,
}

impl std::fmt::Debug for Rpc<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Rpc")
            .field("timeout", &self.timeout)
            .field("body_cap", &self.body_cap)
            .finish_non_exhaustive()
    }
}

impl<'a> Rpc<'a> {
    pub fn new(http: &'a Outbound, url: &'a str) -> Self {
        Self {
            http,
            url,
            timeout: DEFAULT_TIMEOUT,
            body_cap: DEFAULT_BODY_CAP,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn with_body_cap(mut self, body_cap: usize) -> Self {
        self.body_cap = body_cap;
        self
    }

    async fn post(&self, body: &Value) -> Result<(u16, Vec<u8>), RpcError> {
        let resp = self
            .http
            .post(self.url)
            .timeout(self.timeout)
            .json(body)
            .send()
            .await
            .map_err(|e| match e {
                SendError::Refused(m) => RpcError::Refused(m),
                SendError::Http(e) => RpcError::Transport(e.without_url().to_string()),
            })?;
        let status = resp.status().as_u16();
        let bytes = read_body_capped(resp, self.body_cap)
            .await
            .map_err(|e| RpcError::Transport(format!("HTTP {status}: {e}")))?;
        Ok((status, bytes))
    }

    /// The whole envelope of one call (`result` or `error` left to the caller).
    pub async fn envelope(&self, method: &str, params: Value) -> Result<Value, RpcError> {
        let (status, bytes) = self.post(&request(1, method, params)).await?;
        parse_envelope(status, &bytes)
    }

    /// The `result` of one call.
    pub async fn call(&self, method: &str, params: Value) -> Result<Value, RpcError> {
        result_of(self.envelope(method, params).await?)
    }

    /// A call whose result is a `0x` hex string.
    pub async fn call_hex(&self, method: &str, params: Value) -> Result<String, RpcError> {
        hex_of(self.call(method, params).await?)
    }

    /// One HTTP request carrying `requests` (each a [`request`]); the
    /// envelopes the node answered, in its order. An endpoint that answers a
    /// batch with anything but an array answers no element, and the caller
    /// falls back per item.
    pub async fn batch(&self, requests: &[Value]) -> Result<Vec<Value>, RpcError> {
        let (status, bytes) = self.post(&Value::Array(requests.to_vec())).await?;
        match serde_json::from_slice::<Value>(&bytes) {
            Ok(Value::Array(items)) => Ok(items),
            _ if !(200..300).contains(&status) => Err(RpcError::Status(status)),
            _ => Ok(Vec::new()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_json_rpc_body_is_an_answer_whatever_the_status() {
        let refused = br#"{"jsonrpc":"2.0","id":1,"error":{"code":-32005,"message":"query returned more than 10000 results. Try with this block range [0x1, 0x2]."}}"#;
        let v = parse_envelope(400, refused).unwrap();
        assert_eq!(v["error"]["code"], -32005);
        assert!(matches!(
            result_of(v),
            Err(RpcError::Response { code: -32005, .. })
        ));
        assert_eq!(
            parse_envelope(502, b"<html>bad gateway</html>"),
            Err(RpcError::Status(502))
        );
        assert_eq!(
            parse_envelope(429, br#"{"message":"rate limited"}"#),
            Err(RpcError::Status(429))
        );
        assert!(matches!(
            parse_envelope(200, b"not json"),
            Err(RpcError::Malformed(_))
        ));
        assert_eq!(
            parse_envelope(200, br#"{"jsonrpc":"2.0"}"#).unwrap(),
            json!({"jsonrpc":"2.0"})
        );
    }

    #[test]
    fn results_and_their_absence() {
        assert_eq!(result_of(json!({"result": "0x89"})).unwrap(), json!("0x89"));
        assert_eq!(
            result_of(json!({"result": null})).unwrap(),
            Value::Null,
            "an unknown transaction is a null result, not a failure"
        );
        assert_eq!(result_of(json!({"jsonrpc": "2.0"})).unwrap(), Value::Null);
        assert_eq!(hex_of(json!("0x1")).unwrap(), "0x1");
        assert!(matches!(hex_of(json!(7)), Err(RpcError::Malformed(_))));
        assert!(matches!(hex_of(Value::Null), Err(RpcError::Malformed(_))));
        assert_eq!(
            request(1, "eth_chainId", json!([])),
            json!({"jsonrpc":"2.0","id":1,"method":"eth_chainId","params":[]})
        );
        assert!(RpcError::Status(500).is_transport());
        assert!(
            !RpcError::Response {
                code: 3,
                message: "execution reverted".into()
            }
            .is_transport()
        );
    }

    #[tokio::test]
    async fn the_guard_refuses_a_private_endpoint_before_any_byte_leaves() {
        let http = Outbound::new(crate::HttpClientCfg::default()).unwrap();
        let rpc = Rpc::new(&http, "http://127.0.0.1:9/").with_timeout(Duration::from_secs(1));
        assert!(matches!(
            rpc.call("eth_chainId", json!([])).await.unwrap_err(),
            RpcError::Refused(_)
        ));
        assert!(matches!(
            rpc.batch(&[request(0, "eth_chainId", json!([]))])
                .await
                .unwrap_err(),
            RpcError::Refused(_)
        ));
    }

    #[test]
    fn debug_never_prints_the_endpoint() {
        let http = Outbound::for_tests();
        let rpc = Rpc::new(&http, "https://rpc.example/v3/SECRET");
        assert!(!format!("{rpc:?}").contains("SECRET"));
    }
}
