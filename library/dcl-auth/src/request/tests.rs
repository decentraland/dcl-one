use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use axum::http::{HeaderMap, HeaderName, HeaderValue};
use serde::Deserialize;
use serde_json::{Value, json};

use super::*;
use crate::eip1654::Eip1654Validator;
use crate::signed_fetch::SignedPath;
use crate::wallet::Wallet;

#[derive(Deserialize)]
struct Fixture {
    now: i64,
    vectors: Vec<Vector>,
}

#[derive(Deserialize)]
struct Vector {
    name: String,
    method: String,
    path: String,
    local: String,
    headers: Vec<(String, String)>,
    options: Options,
    expected: Expected,
    rust: Option<Expected>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Options {
    canonical_metadata_keys: Option<Vec<String>>,
    service: Option<String>,
    optional: Option<bool>,
    expiration: Option<u64>,
    max_chain_length: Option<usize>,
    max_scoped_login_lifetime: Option<u64>,
    operation: Option<ResolveSpec>,
    resource: Option<ResolveSpec>,
    metadata_validator: Option<String>,
    provider: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ResolveSpec {
    Static(String),
    Function {
        #[serde(rename = "fn")]
        kind: String,
    },
}

#[derive(Deserialize, Debug)]
struct Expected {
    status: u16,
    code: Option<String>,
    auth: Option<String>,
    metadata: Option<Value>,
    scope: Option<ScopedLoginScope>,
    resource: Option<String>,
}

/// A contract wallet that accepts every signature, refuses every one, or
/// cannot be reached.
struct FakeContract(&'static str);

#[async_trait]
impl Eip1654Validator for FakeContract {
    async fn validate_signature(&self, _: &str, _: &[u8], _: &[u8]) -> Result<bool, AuthError> {
        match self.0 {
            "accept" => Ok(true),
            "reject" => Ok(false),
            _ => Err(AuthError::Eip1654ValidationFailed("down".into())),
        }
    }

    async fn validate_signature_at_block(
        &self,
        contract: &str,
        hash: &[u8],
        signature: &[u8],
        _: u64,
    ) -> Result<bool, AuthError> {
        self.validate_signature(contract, hash, signature).await
    }

    async fn verification_time_block(&self, _: i64) -> Result<u64, AuthError> {
        match self.0 {
            "down" => Err(AuthError::Eip1654ValidationFailed("down".into())),
            _ => Ok(100),
        }
    }
}

fn resolve(spec: &ResolveSpec) -> Resolve {
    match spec {
        ResolveSpec::Static(value) => Resolve::Static(value.clone()),
        ResolveSpec::Function { kind } if kind == "none" => {
            Resolve::Dynamic(Arc::new(|_| Ok(None)))
        }
        ResolveSpec::Function { .. } => Resolve::Dynamic(Arc::new(|_| Err(ResolveFailed))),
    }
}

fn options(o: &Options) -> VerifyOptions {
    let mut b = VerifyOptions::builder();
    if let Some(keys) = &o.canonical_metadata_keys {
        b = b.canonical_metadata_keys(keys.clone());
    }
    if let Some(service) = &o.service {
        b = b.service(service.clone());
    }
    if let Some(optional) = o.optional {
        b = b.optional(optional);
    }
    if let Some(ms) = o.expiration {
        b = b.expiration(Duration::from_millis(ms));
    }
    if let Some(links) = o.max_chain_length {
        b = b.max_chain_length(links);
    }
    if let Some(ms) = o.max_scoped_login_lifetime {
        b = b.max_scoped_login_lifetime(Duration::from_millis(ms));
    }
    if let Some(operation) = &o.operation {
        b = b.operation(resolve(operation));
    }
    if let Some(resource) = &o.resource {
        b = b.resource(resolve(resource));
    }
    if let Some(spec) = &o.metadata_validator {
        let (name, values) = spec.split_once(':').unwrap();
        let values: Vec<&str> = values.split(',').collect();
        b = b.metadata_validator(match name {
            "rejectIfSigner" => reject_if_signer(&values).unwrap(),
            "requireSigner" => require_signer(&values).unwrap(),
            other => panic!("validator {other}"),
        });
    }
    if let Some(provider) = &o.provider {
        let mode = match provider.as_str() {
            "accept" => "accept",
            "reject" => "reject",
            _ => "down",
        };
        b = b.eip1654(Arc::new(FakeContract(mode)));
    }
    b.build().unwrap()
}

/// The headers as the wire carries them: one byte per character.
fn header_map(pairs: &[(String, String)]) -> HeaderMap {
    let mut map = HeaderMap::new();
    for (name, value) in pairs {
        let bytes: Vec<u8> = value
            .chars()
            .map(|c| u8::try_from(u32::from(c)).expect("a header carries bytes"))
            .collect();
        map.append(
            HeaderName::try_from(name.as_str()).unwrap(),
            HeaderValue::from_bytes(&bytes).unwrap(),
        );
    }
    map
}

/// Every TS-generated vector through the Rust verifier, at the generator's `now`.
#[tokio::test]
async fn answers_as_crypto_middleware_6_5_0() {
    let fixture: Fixture = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/request-vectors.json"
    )))
    .unwrap();
    let mut mismatches = Vec::new();
    for v in &fixture.vectors {
        let parts = RequestParts::from_headers(
            &v.method,
            SignedPath::new(v.path.clone(), v.local.clone()),
            &header_map(&v.headers),
        );
        let expected = v.rust.as_ref().unwrap_or(&v.expected);
        let result = verify_request_at(&parts, &options(&v.options), fixture.now).await;
        let ok = match &result {
            // A deviation row carries no TS verification: the signer and metadata are the request's.
            Ok(Some(verified)) if v.rust.is_some() => {
                let header = |name: &str| {
                    v.headers
                        .iter()
                        .find(|(n, _)| n == name)
                        .map(|(_, value)| value.as_str())
                };
                let signer: Value =
                    serde_json::from_str(header("x-identity-auth-chain-0").unwrap()).unwrap();
                let metadata: Value =
                    serde_json::from_str(header("x-identity-metadata").unwrap_or("{}")).unwrap();
                expected.status == 200
                    && signer["payload"].as_str().unwrap().to_lowercase() == verified.signer
                    && Value::Object(verified.metadata.clone()) == metadata
            }
            Ok(Some(verified)) => {
                expected.status == 200
                    && expected.auth.as_deref() == Some(verified.signer.as_str())
                    && expected.metadata.as_ref() == Some(&Value::Object(verified.metadata.clone()))
                    && expected.scope == verified.scope
                    && expected.resource == verified.resource
            }
            Ok(None) => expected.status == 200 && expected.auth.is_none(),
            Err(e) => e.status() == expected.status && e.code() == expected.code.as_deref(),
        };
        if !ok {
            mismatches.push(format!("{}: expected {expected:?}, got {result:?}", v.name));
        }
    }
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
    assert!(fixture.vectors.len() > 100);
}

const NOW: i64 = 1_795_000_000_000;

fn wallet(seed: u8) -> Wallet {
    Wallet::from_hex(&format!("{seed:02x}").repeat(32)).unwrap()
}

fn link(link_type: &str, payload: &str, signature: &str) -> String {
    json!({ "type": link_type, "payload": payload, "signature": signature }).to_string()
}

fn delegation(root: &Wallet, to: &Wallet, expiration_ms: i64) -> String {
    let message = format!(
        "Decentraland Login\nEphemeral address: {}\nExpiration: {}",
        to.address(),
        crate::js_date::to_iso_string(expiration_ms).unwrap()
    );
    link(
        "ECDSA_EPHEMERAL",
        &message,
        &root.sign_message(message.as_bytes()).unwrap(),
    )
}

/// A GET to `path` signed by a one-delegation chain at [`NOW`].
fn signed_parts(path: &str, metadata: Option<&str>) -> RequestParts {
    let (root, ephemeral) = (wallet(1), wallet(2));
    let timestamp = NOW.to_string();
    let payload = create_payload("GET", path, &timestamp, metadata.unwrap_or(""));
    let chain = [
        link("SIGNER", &root.address(), ""),
        delegation(&root, &ephemeral, NOW + 600_000),
        link(
            "ECDSA_SIGNED_ENTITY",
            &payload,
            &ephemeral.sign_message(payload.as_bytes()).unwrap(),
        ),
    ];
    RequestParts::from_fields("GET", SignedPath::local(path), |name| match name {
        "x-identity-timestamp" => Some(timestamp.clone()),
        "x-identity-metadata" => metadata.map(str::to_string),
        _ => name
            .strip_prefix("x-identity-auth-chain-")
            .and_then(|i| chain.get(i.parse::<usize>().ok()?).cloned()),
    })
}

#[tokio::test]
async fn a_frame_verifies_like_headers() {
    let parts = signed_parts("/ws", Some(r#"{"intent":"x"}"#));
    let verified = verify_request_at(&parts, &VerifyOptions::default(), NOW)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(verified.signer, wallet(1).address().to_lowercase());
    assert_eq!(verified.metadata["intent"], "x");
    assert_eq!(verified.scope, None);

    let absent = signed_parts("/ws", None);
    let verified = verify_request_at(&absent, &VerifyOptions::default(), NOW)
        .await
        .unwrap()
        .unwrap();
    assert!(verified.metadata.is_empty());
}

#[tokio::test]
async fn stage_order_is_chain_timestamp_metadata_validator_signature() {
    let mut parts = signed_parts("/a", Some("[]"));
    parts.timestamp = Some("soon".into());
    parts.chain[1] = Some("{".into());
    let refuse = |metadata: &Metadata| metadata.is_empty();
    let options = VerifyOptions::builder()
        .metadata_validator(Arc::new(refuse))
        .build()
        .unwrap();
    let err = |parts: RequestParts| {
        let options = options.clone();
        async move { verify_request_at(&parts, &options, NOW).await.unwrap_err() }
    };
    assert!(
        err(parts.clone())
            .await
            .to_string()
            .starts_with("Invalid chain format")
    );
    let mut next = signed_parts("/a", Some("[]"));
    next.timestamp = Some("soon".into());
    assert!(
        err(next.clone())
            .await
            .to_string()
            .starts_with("Invalid chain timestamp")
    );
    next.timestamp = Some((NOW - 60_001).to_string());
    assert!(
        err(next.clone())
            .await
            .to_string()
            .starts_with("Expired signature")
    );
    next.timestamp = Some(NOW.to_string());
    assert!(
        err(next.clone())
            .await
            .to_string()
            .starts_with("Invalid chain metadata")
    );
    next.metadata = Some(r#"{"a":1}"#.into());
    assert!(
        err(next.clone())
            .await
            .to_string()
            .starts_with("Invalid metadata content")
    );
    next.metadata = Some("{}".into());
    assert!(err(next).await.to_string().starts_with("Invalid signature"));
}

/// The window bounds the past; the future side stops at 60 s whatever the window.
#[test]
fn a_wide_window_keeps_the_future_cap() {
    let five_minutes = 300_000;
    assert!(check_timestamp(Some(&(NOW - 300_000).to_string()), five_minutes, NOW).is_ok());
    assert!(check_timestamp(Some(&(NOW + 60_000).to_string()), five_minutes, NOW).is_ok());
    let err = check_timestamp(Some(&(NOW + 60_001).to_string()), five_minutes, NOW).unwrap_err();
    assert_eq!(err.status(), 401);
    assert!(check_timestamp(Some(&(NOW + 30_001).to_string()), 30_000, NOW).is_err());
}

#[test]
fn errors_answer_status_code_and_body() {
    let denied = forbidden(Code::OperationDenied, "no");
    assert_eq!(denied.status(), 403);
    assert_eq!(
        denied.body(),
        json!({ "ok": false, "message": "no", "code": "SCOPED_OPERATION_DENIED" })
    );
    assert_eq!(
        RequestAuthError::Unavailable.body(),
        json!({ "ok": false, "message": "Internal error" })
    );
    assert_eq!(malformed("x").status(), 400);
    assert_eq!(unauthorized("x").status(), 401);
    assert_eq!(safe(&"a".repeat(70)).chars().count(), 65);
}

/// A contract wallet that never answers.
struct Stalled;

#[async_trait]
impl Eip1654Validator for Stalled {
    async fn validate_signature(&self, _: &str, _: &[u8], _: &[u8]) -> Result<bool, AuthError> {
        std::future::pending().await
    }
}

fn contract_parts() -> RequestParts {
    let ephemeral = wallet(2);
    let message = format!(
        "Decentraland Login\nEphemeral address: {}\nExpiration: {}",
        ephemeral.address(),
        crate::js_date::to_iso_string(NOW + 600_000).unwrap()
    );
    let timestamp = NOW.to_string();
    let payload = create_payload("GET", "/a", &timestamp, "{}");
    let chain = [
        link("SIGNER", "0x1234567890123456789012345678901234567890", ""),
        link("ECDSA_EIP_1654_EPHEMERAL", &message, "0x00"),
        link(
            "ECDSA_SIGNED_ENTITY",
            &payload,
            &ephemeral.sign_message(payload.as_bytes()).unwrap(),
        ),
    ];
    RequestParts::from_fields("GET", SignedPath::local("/a"), |name| match name {
        "x-identity-timestamp" => Some(timestamp.clone()),
        "x-identity-metadata" => Some("{}".into()),
        _ => name
            .strip_prefix("x-identity-auth-chain-")
            .and_then(|i| chain.get(i.parse::<usize>().ok()?).cloned()),
    })
}

#[tokio::test(start_paused = true)]
async fn a_stalled_contract_check_is_unavailable_after_the_deadline() {
    let options = VerifyOptions::builder()
        .eip1654(Arc::new(Stalled))
        .build()
        .unwrap();
    let started = tokio::time::Instant::now();
    let err = verify_request_at(&contract_parts(), &options, NOW)
        .await
        .unwrap_err();
    assert_eq!(err, RequestAuthError::Unavailable);
    assert_eq!(started.elapsed(), SIGNATURE_VALIDATION_TIMEOUT);
}

#[tokio::test]
async fn a_contract_link_without_a_validator_is_invalid() {
    let err = verify_request_at(&contract_parts(), &VerifyOptions::default(), NOW)
        .await
        .unwrap_err();
    assert_eq!(err.status(), 401);
}

mod rpc {
    use super::*;
    use crate::rpc_validator::RpcEip1654Validator;
    use crate::validation_cache::ValidationCache;
    use axum::Json;
    use axum::routing::post;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A JSON-RPC node answering the `n`th call (from 1) with `answer`; an
    /// `httpStatus` field sets the response status.
    async fn node(answer: fn(&Value, usize) -> Value, calls: Arc<AtomicUsize>) -> String {
        let app = axum::Router::new().route(
            "/",
            post(move |Json(body): Json<Value>| {
                let calls = calls.clone();
                async move {
                    let n = calls.fetch_add(1, Ordering::SeqCst) + 1;
                    let mut answer = answer(&body, n);
                    let status = answer
                        .as_object_mut()
                        .and_then(|o| o.remove("httpStatus"))
                        .and_then(|s| s.as_u64())
                        .unwrap_or(200);
                    (
                        axum::http::StatusCode::from_u16(status as u16).unwrap(),
                        Json(answer),
                    )
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        url
    }

    /// Contracts that refuse the raw hash and accept the EIP-191 one: two calls a link.
    fn second_hash_node(body: &Value, n: usize) -> Value {
        if n % 2 == 1 {
            return json!({ "jsonrpc": "2.0", "id": body["id"], "error": { "code": 3, "message": "execution reverted" } });
        }
        json!({ "jsonrpc": "2.0", "id": body["id"], "result": format!("0x1626ba7e{}", "0".repeat(56)) })
    }

    fn failing_node(body: &Value, _: usize) -> Value {
        json!({ "jsonrpc": "2.0", "id": body["id"], "error": { "code": -32603, "message": "internal error" } })
    }

    fn magic(body: &Value) -> Value {
        json!({ "jsonrpc": "2.0", "id": body["id"], "result": format!("0x1626ba7e{}", "0".repeat(56)) })
    }

    /// No answer for the raw hash, then acceptance of the EIP-191 one.
    fn null_then_magic(body: &Value, n: usize) -> Value {
        if n == 1 {
            return json!({ "jsonrpc": "2.0", "id": body["id"], "result": null });
        }
        magic(body)
    }

    /// An HTTP 500 carrying an accepting envelope.
    fn server_error_with_magic(body: &Value, _: usize) -> Value {
        let mut answer = magic(body);
        answer["httpStatus"] = json!(500);
        answer
    }

    fn oversized_call(body: &Value, _: usize) -> Value {
        json!({ "jsonrpc": "2.0", "id": body["id"], "result": format!("0x{}", "00".repeat(33)) })
    }

    async fn verdict(
        answer: fn(&Value, usize) -> Value,
        parts: RequestParts,
    ) -> (Result<Option<VerifiedRequest>, RequestAuthError>, usize) {
        let calls = Arc::new(AtomicUsize::new(0));
        let url = node(answer, calls.clone()).await;
        let rpc = Arc::new(RpcEip1654Validator::new(url, reqwest::Client::new()));
        let options = VerifyOptions::builder()
            .eip1654(Arc::new(ValidationCache::new(rpc)))
            .build()
            .unwrap();
        let result = verify_request_at(&parts, &options, NOW).await;
        (result, calls.load(Ordering::SeqCst))
    }

    /// SIGNER, `delegations` contract-wallet delegations and a contract-signed request.
    fn contract_chain(delegations: usize) -> RequestParts {
        let address = |i: usize| format!("0x{}", format!("{:x}", i + 1).repeat(40));
        let expiration = crate::js_date::to_iso_string(NOW + 600_000).unwrap();
        let timestamp = NOW.to_string();
        let payload = create_payload("GET", "/a", &timestamp, "{}");
        let mut chain = vec![link("SIGNER", &address(0), "")];
        for i in 1..=delegations {
            let message = format!(
                "Decentraland Login\nEphemeral address: {}\nExpiration: {expiration}",
                address(i)
            );
            chain.push(link("ECDSA_EIP_1654_EPHEMERAL", &message, "0x00"));
        }
        chain.push(link("ECDSA_EIP_1654_SIGNED_ENTITY", &payload, "0x00"));
        RequestParts::from_fields("GET", SignedPath::local("/a"), |name| match name {
            "x-identity-timestamp" => Some(timestamp.clone()),
            "x-identity-metadata" => Some("{}".into()),
            _ => name
                .strip_prefix("x-identity-auth-chain-")
                .and_then(|i| chain.get(i.parse::<usize>().ok()?).cloned()),
        })
    }

    #[tokio::test]
    async fn contract_links_within_the_budget_verify() {
        let (result, calls) = verdict(second_hash_node, contract_chain(7)).await;
        assert!(result.unwrap().is_some());
        assert_eq!(calls, 16);
    }

    #[tokio::test]
    async fn a_validation_stops_at_its_rpc_call_budget() {
        let (result, calls) = verdict(second_hash_node, contract_chain(8)).await;
        let err = result.unwrap_err();
        assert_eq!(calls, MAX_RPC_CALLS, "{err:?}");
        assert!(
            matches!(&err, RequestAuthError::Unauthorized(m) if m.contains("exceeded 16 RPC calls")),
            "{err:?}"
        );
    }

    #[tokio::test]
    async fn a_node_error_is_unavailable_and_stops_the_validation() {
        let (result, calls) = verdict(failing_node, contract_parts()).await;
        let err = result.unwrap_err();
        assert_eq!(err, RequestAuthError::Unavailable);
        assert_eq!(calls, 1);
    }

    /// A node that never answers: each call gives up after [`RPC_CALL_TIMEOUT`].
    #[tokio::test(start_paused = true)]
    async fn a_stalled_rpc_call_is_unavailable_after_its_own_timeout() {
        let app = axum::Router::new().route(
            "/",
            post(|| async {
                std::future::pending::<()>().await;
                ""
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let rpc = Arc::new(RpcEip1654Validator::new(
            url,
            crate::rpc_validator::rpc_client().unwrap(),
        ));
        let options = VerifyOptions::builder().eip1654(rpc).build().unwrap();
        let started = tokio::time::Instant::now();
        let err = verify_request_at(&contract_parts(), &options, NOW)
            .await
            .unwrap_err();
        assert_eq!(err, RequestAuthError::Unavailable);
        assert_eq!(started.elapsed(), RPC_CALL_TIMEOUT);
    }

    /// A malformed answer ends the validation: a later acceptance is never read.
    #[tokio::test]
    async fn a_null_answer_is_unavailable_even_before_an_acceptance() {
        let (result, calls) = verdict(null_then_magic, contract_parts()).await;
        assert_eq!(result.unwrap_err(), RequestAuthError::Unavailable);
        assert_eq!(calls, 1);
    }

    #[tokio::test]
    async fn a_server_error_is_unavailable_whatever_its_body() {
        let (result, calls) = verdict(server_error_with_magic, contract_parts()).await;
        assert_eq!(result.unwrap_err(), RequestAuthError::Unavailable);
        assert_eq!(calls, 1);
    }

    #[tokio::test]
    async fn an_eth_call_answer_longer_than_one_word_is_unavailable() {
        let (result, _) = verdict(oversized_call, contract_parts()).await;
        let err = result.unwrap_err();
        assert_eq!(err, RequestAuthError::Unavailable);
    }
}

mod scoped {
    use super::*;
    use crate::scoped_login::{ScopedLoginGrant, scoped_ephemeral_message};

    fn scoped_parts(grants: &[(&str, &str)], expiration_ms: i64) -> RequestParts {
        let (root, ephemeral) = (wallet(1), wallet(2));
        let grants: Vec<ScopedLoginGrant> = grants
            .iter()
            .map(|(service, operation)| ScopedLoginGrant {
                service: service.to_string(),
                operation: operation.to_string(),
                resource: None,
            })
            .collect();
        let message =
            scoped_ephemeral_message(&ephemeral.address(), expiration_ms, &grants).unwrap();
        let timestamp = NOW.to_string();
        let payload = create_payload("GET", "/a", &timestamp, "{}");
        let chain = [
            link("SIGNER", &root.address(), ""),
            link(
                "ECDSA_SCOPED_EPHEMERAL",
                &message,
                &root.sign_message(message.as_bytes()).unwrap(),
            ),
            link(
                "ECDSA_SIGNED_ENTITY",
                &payload,
                &ephemeral.sign_message(payload.as_bytes()).unwrap(),
            ),
        ];
        RequestParts::from_fields("GET", SignedPath::local("/a"), |name| match name {
            "x-identity-timestamp" => Some(timestamp.clone()),
            "x-identity-metadata" => Some("{}".into()),
            _ => name
                .strip_prefix("x-identity-auth-chain-")
                .and_then(|i| chain.get(i.parse::<usize>().ok()?).cloned()),
        })
    }

    fn route(operation: &str) -> VerifyOptions {
        VerifyOptions::builder()
            .service("insights")
            .operation(Resolve::Static(operation.into()))
            .build()
            .unwrap()
    }

    #[tokio::test]
    async fn grants_reach_only_their_service_and_operation() {
        let parts = scoped_parts(&[("insights", "logs-read"), ("other", "x")], NOW + 600_000);
        let verified = verify_request_at(&parts, &route("logs-read"), NOW)
            .await
            .unwrap()
            .unwrap();
        let scope = verified.scope.unwrap();
        assert_eq!(scope.service, "insights");
        assert_eq!(scope.expiration, NOW + 600_000);
        assert_eq!(scope.grants.len(), 1);

        let err = verify_request_at(&parts, &route("logs-write"), NOW)
            .await
            .unwrap_err();
        assert_eq!(err.code(), Some("SCOPED_OPERATION_DENIED"));

        let err = verify_request_at(&parts, &VerifyOptions::default(), NOW)
            .await
            .unwrap_err();
        assert_eq!(err.status(), 401);
    }

    #[tokio::test]
    async fn the_lifetime_cap_bounds_what_remains() {
        let at_cap = scoped_parts(&[("insights", "*")], NOW + 86_400_000);
        assert!(verify_request_at(&at_cap, &route("x"), NOW).await.is_ok());
        let over = scoped_parts(&[("insights", "*")], NOW + 86_400_001);
        assert_eq!(
            verify_request_at(&over, &route("x"), NOW)
                .await
                .unwrap_err()
                .status(),
            401
        );
    }

    #[tokio::test]
    async fn optional_lets_no_signature_through_but_not_an_ungranted_scope() {
        let options = VerifyOptions::builder()
            .service("insights")
            .operation(Resolve::Static("logs-write".into()))
            .optional(true)
            .build()
            .unwrap();
        let empty = RequestParts::from_fields("GET", SignedPath::local("/a"), |_| None);
        assert_eq!(verify_request_at(&empty, &options, NOW).await, Ok(None));
        let parts = scoped_parts(&[("insights", "logs-read")], NOW + 600_000);
        assert_eq!(
            verify_request_at(&parts, &options, NOW)
                .await
                .unwrap_err()
                .status(),
            403
        );
    }
}
