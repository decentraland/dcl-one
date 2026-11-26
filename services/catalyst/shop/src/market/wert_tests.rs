use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode};
use ed25519_dalek::{Signature, SigningKey, Verifier as _};
use serde_json::{Value, json};
use tower::ServiceExt as _;

use super::*;
use crate::market::tests::support::state_without_db_settings;
use crate::market::{Settings, market_router};
use crate::shared::signed_fetch::test_support::{address, signed_headers};

const MARKETPLACE: &str = r#"{"signer":"dcl:marketplace"}"#;

const TEST_CONTRACT: &str = "0x0000000000000000000000000000000000000003";
const PUBLICATION_MANAGER: &str = "0x0000000000000000000000000000000000000004";
const CREDITS_MANAGER: &str = "0x0000000000000000000000000000000000000005";
const AMOY_PUBLICATION_MANAGER: &str = "0x00000000000000000000000000000000000000a6";

fn test_policy() -> anyhow::Result<WertPolicy> {
    let extras =
        format!("ethereum:{TEST_CONTRACT}:default,ethereum:{TEST_CONTRACT}:publicationFees");
    WertPolicy::new(
        None,
        EvmAddress::parse(PUBLICATION_MANAGER).unwrap(),
        Some(&extras),
    )
}

fn settings_with_policy(api_url: &str, policy: WertPolicy) -> WertSettings {
    WertSettings {
        default_key: Arc::new(SigningKey::from_bytes(&[0x11; 32])),
        publication_fees_key: Arc::new(SigningKey::from_bytes(&[0x22; 32])),
        api_url: api_url.to_owned(),
        api_key: "wert-api-key-example".to_owned(),
        policy: Arc::new(policy),
    }
}

fn test_settings(api_url: &str) -> WertSettings {
    settings_with_policy(api_url, test_policy().unwrap())
}

fn message(amount: Value) -> Message {
    serde_json::from_value(json!({
        "address": "0xAbC0000000000000000000000000000000000001",
        "commodity": "MATIC",
        "commodity_amount": amount,
        "network": "Amoy",
        "sc_address": "0x0000000000000000000000000000000000000002",
        "sc_input_data": "0xdeadbeef",
    }))
    .unwrap()
}

/// The upstream library's own signatures over the message above, made with
/// the fake seeds `0x11…` (default) and `0x22…` (publication fees):
/// `signSmartContractData(message, key).signature` from
/// `@wert-io/widget-sc-signer` 2.0.0, as marketplace-server calls it.
#[test]
fn the_signature_is_the_upstream_librarys() {
    let default_key = SigningKey::from_bytes(&[0x11; 32]);
    let fees_key = SigningKey::from_bytes(&[0x22; 32]);
    let sign = |key: &SigningKey, message: Message| sign_text(key, &message.signed_text().unwrap());
    let one_and_a_half = "1AFE06DF64B61C125D7E56B2EF21203430DCAC80CD500D7D0B998EDEEF79A353BBEE3CE974B25051D522723671519C2A8057092F46BA53F4D39B07D07B3D8602";
    assert_eq!(sign(&default_key, message(json!(1.5))), one_and_a_half);
    assert_eq!(
        sign(&default_key, message(json!("1.5"))),
        one_and_a_half,
        "a numeric string signs as the number"
    );
    assert_eq!(
        sign(&fees_key, message(json!(1.5))),
        "1B8F76F73F4781FFED1F15CB1648EC785A34983983F3E2FF3A1D7450758C2A6E71A6FA77A4D2803E0823C0FD99FE863628766CED45E314F21F5DE9686825CC0F"
    );
    assert_eq!(
        sign(&default_key, message(json!("1.50"))),
        "758B34A9974E50F9231F7D015C176B4B7AC94DF13F2CE63AA2E7D248168BA7D36DA43763001C68E2CB77585007BB27388A2DAB2EB2BF2FD3030CBCD0AD24AD01",
        "a string amount is signed as sent"
    );
    assert_eq!(
        sign(&default_key, message(json!(0.000001))),
        "1E3BFE23A489314C801C7F82D07E6A71F1460F9DDF7FD26E5F19D1F6F089669248C719F20AC9A74D5778C5235D3229F4EF3DB59EFED8B3D0EFCFCFEDCDC6AC00"
    );
    assert_eq!(
        sign(&default_key, message(json!(123456789.123))),
        "75736EBEFD02FD09BD3D848623C3FBF29B49B55C4F08A265133C083058037317485ABB8660670A55E389D44C22281890C585F543C7A33938B8371DB4351B5907"
    );
    let mut mana = message(json!(100));
    mana.commodity = "MANA".into();
    mana.network = "ethereum".into();
    assert_eq!(
        sign(&default_key, mana),
        "884CB2B10D74376C9D7AD3D510E30A556C77CF3ACB3C4ADE85B28C1D9944EC79799AD331F8AB8A5BD246B23BA56E75E94320A4B39721D712A0619A4F38D78B01"
    );
}

#[test]
fn the_signed_text_lowercases_and_writes_numbers_as_javascript() {
    assert_eq!(
        message(json!(1.5)).signed_text().unwrap(),
        "address:0xAbC0000000000000000000000000000000000001\ncommodity:matic\ncommodity_amount:1.5\nnetwork:amoy\nsc_address:0x0000000000000000000000000000000000000002\nsc_input_data:0xdeadbeef"
    );
    let amount = |v: Value| {
        message(v)
            .signed_text()
            .map(|t| t.lines().nth(2).unwrap().to_string())
    };
    let parsed: Value = serde_json::from_str("1.0").unwrap();
    assert_eq!(amount(parsed).as_deref(), Some("commodity_amount:1"));
    assert_eq!(
        amount(json!(1e21)).as_deref(),
        Some("commodity_amount:1e+21")
    );
    assert_eq!(
        amount(json!(1e-7)).as_deref(),
        Some("commodity_amount:1e-7")
    );
    assert_eq!(amount(json!(true)), None);
    assert_eq!(amount(json!(null)), None);
}

#[test]
fn targets_follow_upstreams_truthiness() {
    for falsy in [
        None,
        Some(json!(null)),
        Some(json!(false)),
        Some(json!(0)),
        Some(json!("")),
        Some(json!("default")),
    ] {
        assert_eq!(
            Target::from_body(falsy.as_ref()),
            Some(Target::Default),
            "{falsy:?}"
        );
    }
    assert_eq!(
        Target::from_body(Some(&json!("publicationFees"))),
        Some(Target::PublicationFees)
    );
    for invalid in [
        json!("PublicationFees"),
        json!("fees"),
        json!(1),
        json!(true),
        json!({}),
    ] {
        assert_eq!(Target::from_body(Some(&invalid)), None, "{invalid}");
    }
}

#[test]
fn settings_need_all_four_and_never_echo_a_value() {
    let key = format!("0x{}", "11".repeat(32));
    let url = Some("https://partner.example.com/api/v3/");
    assert!(
        WertSettings::parse(None, Some(" "), None, None, &[], test_policy)
            .unwrap()
            .is_none()
    );
    let partial = WertSettings::parse(Some(&key), None, url, None, &[], test_policy)
        .unwrap_err()
        .to_string();
    assert!(
        partial.contains("missing: WERT_PUBLICATION_FEES_PRIVATE_KEY_FILE, WERT_API_KEY")
            && !partial.contains(&key),
        "{partial}"
    );
    let short = "11".repeat(31);
    let bad_key = WertSettings::parse(
        Some(&short),
        Some(&key),
        url,
        Some("api-key-example"),
        &[],
        test_policy,
    )
    .unwrap_err()
    .to_string();
    assert!(
        bad_key.contains("WERT_PRIVATE_KEY") && !bad_key.contains(&short),
        "{bad_key}"
    );
    let plaintext = WertSettings::parse(
        Some(&key),
        Some(&key),
        Some("http://partner.example.com"),
        Some("api-key-example"),
        &[],
        test_policy,
    )
    .unwrap_err()
    .to_string();
    assert!(plaintext.contains("WERT_API_URL"), "{plaintext}");
    let settings = WertSettings::parse(
        Some(&key),
        Some(&"22".repeat(32)),
        url,
        Some("api-key-example"),
        &[],
        test_policy,
    )
    .unwrap()
    .unwrap();
    assert_eq!(settings.api_url, "https://partner.example.com/api/v3");
    let debug = format!("{settings:?}");
    assert!(
        !debug.contains("api-key-example") && !debug.contains("1111") && !debug.contains("2222"),
        "{debug}"
    );
}

#[test]
fn wert_extra_contract_config_rejects_malformed_tuples() {
    let manager = EvmAddress::parse(PUBLICATION_MANAGER).unwrap();
    for (input, refusal) in [
        (format!("polygon:{TEST_CONTRACT}"), "network:address:target"),
        (
            format!("matic:{TEST_CONTRACT}:default"),
            "unsupported network",
        ),
        (format!("polygon:{TEST_CONTRACT}:fees"), "invalid target"),
        ("polygon:invalid:default".to_string(), "invalid EVM address"),
        (
            format!("polygon:{TEST_CONTRACT}:default,"),
            "network:address:target",
        ),
    ] {
        let error = WertPolicy::new(None, manager, Some(&input))
            .err()
            .expect("invalid startup configuration")
            .to_string();
        assert!(error.contains("WERT_EXTRA_CONTRACTS entry"), "{error}");
        assert!(error.contains(refusal), "{error}");
        assert!(!error.contains(&input), "{error}");
    }
}

/// What the fake Wert API answers, and the requests it received.
#[derive(Clone)]
struct FakeWert {
    status: StatusCode,
    body: Value,
    seen: Arc<Mutex<Vec<(HeaderMap, Value)>>>,
}

async fn fake_wert(status: StatusCode, body: Value) -> (String, FakeWert) {
    let fake = FakeWert {
        status,
        body,
        seen: Arc::default(),
    };
    let handler = {
        let fake = fake.clone();
        move |headers: HeaderMap, axum::Json(body): axum::Json<Value>| {
            let fake = fake.clone();
            async move {
                fake.seen.lock().unwrap().push((headers, body));
                (fake.status, axum::Json(fake.body.clone()))
            }
        }
    };
    let app = Router::new().route("/create-session", axum::routing::post(handler));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (url, fake)
}

fn app_with(wert: Option<WertSettings>) -> Router {
    let mut settings = Settings::for_tests();
    settings.market.wert = wert;
    assert!(
        !settings.data_writes_enabled,
        "the route answers while writes are off"
    );
    market_router(state_without_db_settings(settings))
}

async fn post(app: Router, path: &str, headers: HeaderMap, body: Value) -> (StatusCode, Value) {
    let mut request = Request::post(path).header("content-type", "application/json");
    for (name, value) in &headers {
        request = request.header(name, value);
    }
    let response = app
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn request_body(target: Option<&str>) -> Value {
    let me = address();
    let mut body = json!({
        "message": {
            "address": me.to_uppercase().replacen("0X", "0x", 1),
            "commodity": "MANA",
            "commodity_amount": 12.5,
            "network": "Ethereum",
            "sc_address": TEST_CONTRACT,
            "sc_input_data": "0xc0ffee",
        },
        "session": { "flow_type": "simple_full_restrict", "wallet_address": me, "commodity": "MANA", "phone": "+15550100" },
    });
    if let Some(target) = target {
        body["target"] = json!(target);
    }
    body
}

fn verifies(seed: u8, signature: &str, body: &Value) -> bool {
    let m = &body["message"];
    let text = format!(
        "address:{}\ncommodity:mana\ncommodity_amount:12.5\nnetwork:{}\nsc_address:{}\nsc_input_data:{}",
        m["address"].as_str().unwrap(),
        m["network"].as_str().unwrap().to_lowercase(),
        m["sc_address"].as_str().unwrap(),
        m["sc_input_data"].as_str().unwrap(),
    );
    assert_eq!(signature, signature.to_uppercase());
    let bytes: [u8; 64] = hex::decode(signature).unwrap().try_into().unwrap();
    SigningKey::from_bytes(&[seed; 32])
        .verifying_key()
        .verify(text.as_bytes(), &Signature::from_bytes(&bytes))
        .is_ok()
}

#[tokio::test]
async fn an_unconfigured_node_names_the_four_settings() {
    let headers = signed_headers("post", PATH, MARKETPLACE, true).await;
    let (status, body) = post(app_with(None), PATH, headers, request_body(None)).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(body["source"], "wert");
    assert_eq!(
        body["configure"],
        "WERT_PRIVATE_KEY_FILE, WERT_PUBLICATION_FEES_PRIVATE_KEY_FILE, WERT_API_URL, WERT_API_KEY"
    );
}

#[tokio::test]
async fn it_signs_with_the_targets_key_and_creates_the_session() {
    let (url, fake) = fake_wert(
        StatusCode::OK,
        json!({ "sessionId": "session-example", "requestId": "r" }),
    )
    .await;
    let app = app_with(Some(test_settings(&url)));
    for (target, seed, other) in [
        (None, 0x11, 0x22),
        (Some("default"), 0x11, 0x22),
        (Some("publicationFees"), 0x22, 0x11),
    ] {
        let sent = request_body(target);
        let headers = signed_headers("post", PATH, MARKETPLACE, true).await;
        let (status, body) = post(app.clone(), PATH, headers, sent.clone()).await;
        assert_eq!(status, StatusCode::OK, "{target:?}: {body}");
        assert_eq!(body["ok"], true);
        assert_eq!(body["data"]["sessionId"], "session-example");
        let signature = body["data"]["signature"].as_str().unwrap();
        assert!(verifies(seed, signature, &sent), "{target:?}");
        assert!(!verifies(other, signature, &sent), "{target:?}");
        let (headers, session) = fake.seen.lock().unwrap().pop().unwrap();
        assert_eq!(headers["x-api-key"], "wert-api-key-example");
        assert_eq!(session, sent["session"], "the session reaches Wert as sent");
    }
}

fn request_for(network: &str, contract: &str, target: Target) -> Value {
    let mut request = request_body(Some(target.name()));
    request["message"]["network"] = json!(network);
    request["message"]["sc_address"] = json!(contract);
    request
}

#[tokio::test]
async fn wert_contract_policy_signs_each_seeded_tuple_and_the_configured_extra() {
    let (url, fake) = fake_wert(StatusCode::OK, json!({ "sessionId": "session-example" })).await;
    let credits = EvmAddress::parse(CREDITS_MANAGER).unwrap();
    let publication = EvmAddress::parse(PUBLICATION_MANAGER).unwrap();
    let amoy_publication = EvmAddress::parse(AMOY_PUBLICATION_MANAGER).unwrap();
    let extras = format!(
        "amoy:{AMOY_PUBLICATION_MANAGER}:publicationFees, AMOY:{}:publicationFees",
        AMOY_PUBLICATION_MANAGER.replace("a6", "A6"),
    );
    let policy = WertPolicy::new(
        Some((ChainId::POLYGON, credits)),
        publication,
        Some(&extras),
    )
    .unwrap();

    let polygon = DclContracts::for_chain(137).unwrap();
    let ethereum = NameContracts::for_chain(1).unwrap();
    let expected = [
        (ChainId::POLYGON, credits, Target::Default),
        (ChainId::POLYGON, publication, Target::PublicationFees),
        (
            ChainId::POLYGON,
            polygon.marketplace_v2.into(),
            Target::Default,
        ),
        (
            ChainId::POLYGON,
            polygon.offchain_marketplace.into(),
            Target::Default,
        ),
        (
            ChainId::POLYGON,
            collection_store(ChainId::POLYGON).unwrap(),
            Target::Default,
        ),
        (
            ChainId::ETHEREUM,
            ethereum.marketplace.into(),
            Target::Default,
        ),
        (
            ChainId::AMOY,
            collection_store(ChainId::AMOY).unwrap(),
            Target::Default,
        ),
        (ChainId::AMOY, amoy_publication, Target::PublicationFees),
    ];
    let offchain = NETWORKS.into_iter().flat_map(|(chain, _)| {
        listings::offchain_marketplaces(chain.0 as i64)
            .into_iter()
            .map(move |market| {
                (
                    chain,
                    EvmAddress::parse(market.address).unwrap(),
                    Target::Default,
                )
            })
    });
    assert_eq!(
        policy.contracts,
        expected.into_iter().chain(offchain).collect()
    );

    let contracts = policy.contracts.clone();
    let app = app_with(Some(settings_with_policy(&url, policy)));
    let headers = signed_headers("post", PATH, MARKETPLACE, true).await;
    for (chain, address, target) in &contracts {
        let (_, network) = NETWORKS.into_iter().find(|(id, _)| id == chain).unwrap();
        let sent = request_for(
            &network.to_uppercase(),
            &address.to_string().to_uppercase().replacen("0X", "0x", 1),
            *target,
        );
        let (status, response) = post(app.clone(), PATH, headers.clone(), sent.clone()).await;
        assert_eq!(
            status,
            StatusCode::OK,
            "{network} {address} {target:?}: {response}"
        );
        let seed = match target {
            Target::Default => 0x11,
            Target::PublicationFees => 0x22,
        };
        assert!(verifies(
            seed,
            response["data"]["signature"].as_str().unwrap(),
            &sent
        ));
    }
    assert_eq!(fake.seen.lock().unwrap().len(), contracts.len());
}

#[tokio::test]
async fn wert_contract_policy_refuses_unknown_wrong_chain_and_wrong_target() {
    let (url, fake) = fake_wert(StatusCode::OK, json!({ "sessionId": "session-example" })).await;
    let app = app_with(Some(test_settings(&url)));
    let polygon_store = collection_store(ChainId::POLYGON).unwrap().to_string();
    let headers = signed_headers("post", PATH, MARKETPLACE, true).await;
    for (network, address, target) in [
        (
            "polygon",
            "0x0000000000000000000000000000000000000099",
            Target::Default,
        ),
        ("amoy", polygon_store.as_str(), Target::Default),
        ("polygon", polygon_store.as_str(), Target::PublicationFees),
        ("matic", polygon_store.as_str(), Target::Default),
        ("polygon", "malformed", Target::Default),
    ] {
        let (status, body) = post(
            app.clone(),
            PATH,
            headers.clone(),
            request_for(network, address, target),
        )
        .await;
        assert_eq!(
            (status, body["message"].as_str()),
            (StatusCode::BAD_REQUEST, Some("Invalid contract")),
            "{network} {address} {target:?}: {body}",
        );
    }
    assert!(fake.seen.lock().unwrap().is_empty());
}

#[tokio::test]
async fn it_refuses_as_upstream_before_calling_wert() {
    let (url, fake) = fake_wert(StatusCode::OK, json!({ "sessionId": "session-example" })).await;
    let app = app_with(Some(test_settings(&url)));
    let signed = || signed_headers("post", PATH, MARKETPLACE, true);

    let (status, body) = post(app.clone(), PATH, HeaderMap::new(), request_body(None)).await;
    assert_eq!(
        (status, body["message"].as_str()),
        (StatusCode::UNAUTHORIZED, Some("Unauthorized")),
        "unsigned"
    );
    let scene = signed_headers(
        "post",
        PATH,
        r#"{"signer":"decentraland-kernel-scene"}"#,
        true,
    )
    .await;
    let (status, _) = post(app.clone(), PATH, scene, request_body(None)).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "a scene-signed chain counts as unsigned"
    );
    let elsewhere = signed_headers("post", "/v1/other", MARKETPLACE, true).await;
    let (status, _) = post(app.clone(), PATH, elsewhere, request_body(None)).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "a signature over another path"
    );

    let other = "0x00000000000000000000000000000000000000ff";
    for (field, pointer) in [
        ("message", "/message/address"),
        ("session", "/session/wallet_address"),
    ] {
        let mut sent = request_body(None);
        *sent.pointer_mut(pointer).unwrap() = json!(other);
        let (status, body) = post(app.clone(), PATH, signed().await, sent).await;
        assert_eq!(
            (status, body["message"].as_str()),
            (StatusCode::UNAUTHORIZED, Some("Unauthorized")),
            "{field}"
        );
    }
    let mut no_wallet = request_body(None);
    no_wallet["session"]
        .as_object_mut()
        .unwrap()
        .remove("wallet_address");
    let (status, _) = post(app.clone(), PATH, signed().await, no_wallet).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "a session without the wallet"
    );

    let (status, body) = post(
        app.clone(),
        PATH,
        signed().await,
        request_body(Some("PublicationFees")),
    )
    .await;
    assert_eq!(
        (status, body["message"].as_str()),
        (StatusCode::BAD_REQUEST, Some("Invalid target"))
    );
    let (status, _) = post(
        app.clone(),
        PATH,
        HeaderMap::new(),
        request_body(Some("PublicationFees")),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "auth is checked before the target"
    );

    let mut extra = request_body(None);
    extra["message"]["pk_id"] = json!("key_1");
    let mut mistyped = request_body(None);
    mistyped["message"]["commodity_amount"] = json!(true);
    let mut no_session = request_body(None);
    no_session.as_object_mut().unwrap().remove("session");
    for (case, sent) in [
        ("extra key", extra),
        ("mistyped amount", mistyped),
        ("no session", no_session),
    ] {
        let (status, body) = post(app.clone(), PATH, signed().await, sent).await;
        assert_eq!(
            (status, body["message"].as_str()),
            (StatusCode::BAD_REQUEST, Some("Invalid message")),
            "{case}"
        );
    }
    assert!(
        fake.seen.lock().unwrap().is_empty(),
        "no refused request reached Wert"
    );
}

#[tokio::test]
async fn a_session_wert_does_not_create_is_a_502_without_its_reply() {
    for (status, reply) in [
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({ "error": "wert-internal-detail" }),
        ),
        (
            StatusCode::OK,
            json!({ "requestId": "wert-internal-detail" }),
        ),
    ] {
        let (url, _fake) = fake_wert(status, reply).await;
        let headers = signed_headers("post", PATH, MARKETPLACE, true).await;
        let (code, body) = post(
            app_with(Some(test_settings(&url))),
            PATH,
            headers,
            request_body(None),
        )
        .await;
        assert_eq!(
            (code, body["message"].as_str()),
            (
                StatusCode::BAD_GATEWAY,
                Some("Wert session could not be created")
            ),
            "{status}"
        );
        assert!(!body.to_string().contains("wert-internal-detail"), "{body}");
    }
}

#[tokio::test]
async fn both_mounts_serve_it_and_bind_the_signature_to_the_incoming_path() {
    let (url, _fake) = fake_wert(StatusCode::OK, json!({ "sessionId": "session-example" })).await;
    for mount in ["/shop-api", "/marketplace-api"] {
        let app = Router::new().nest(mount, app_with(Some(test_settings(&url))));
        let path = format!("{mount}{PATH}");
        let headers = signed_headers("post", &path, MARKETPLACE, true).await;
        let (status, body) = post(app.clone(), &path, headers, request_body(None)).await;
        assert_eq!(status, StatusCode::OK, "{mount}: {body}");
        let headers = signed_headers("post", "/shop-api/v1/other", MARKETPLACE, true).await;
        let (status, _) = post(app, &path, headers, request_body(None)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{mount}");
    }
}

#[test]
fn the_open_paths_keep_every_historical_read() {
    for path in crate::market::historical::READ_PATHS {
        assert!(crate::market::OPEN_PATHS.contains(path), "{path}");
    }
    assert!(crate::market::OPEN_PATHS.contains(&PATH));
}

#[tokio::test]
async fn the_pending_router_no_longer_claims_the_route() {
    // `shop::router` merges both: two routers owning one path panic there.
    let app = Router::new()
        .merge(app_with(None))
        .merge(crate::pending_router());
    let (status, body) = post(app, PATH, HeaderMap::new(), request_body(None)).await;
    assert_eq!(
        (status, body["source"].as_str()),
        (StatusCode::SERVICE_UNAVAILABLE, Some("wert"))
    );
}
