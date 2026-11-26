//! A collection's publication fee paid with credits, through the routers as
//! `serve` mounts them over a database of the test's own: the authorization
//! a creator gets for their draft, every refusal before anything is reserved
//! or signed, the relay's acceptance of exactly the authorized call, and the
//! ledger line's settlement and expiry.

use std::sync::{Arc, Mutex};

use alloy::signers::local::PrivateKeySigner;
use alloy_primitives::{Address, B256, U256};
use alloy_sol_types::SolCall;
use async_trait::async_trait;
use axum::Router;
use axum::http::StatusCode;
use serde_json::{Value, json};
use sqlx::PgPool;

use super::digest::{CreditIdentity, ExternalCallIdentity, parse_bytes32, recover_credit_signer};
use super::errors::ApiError;
use super::publication::{PublicationItem, PublicationQuote, Publications, createCollectionCall};
use super::settlement::{FixedGate, IssuanceGate};
use super::tests::{
    FakeMarket, MockBroker, OwnDatabase, Wallet, app, available, call, scratch_wallet, seed_wallet,
    send, signed_headers,
};
use super::{RawSettings, Settings, SharedState, State};
use crate::guards::relay::guard_meta_transaction;
use crate::http::Outbound;
use crate::policy_entry::{DCL_ONE_AMOY_MANAGER, DCL_ONE_POLYGON_MANAGER, credits_abi};
use crate::shared::signed_fetch::Signer;
use crate::types::EvmAddress;

const LEGACY: &str = "/credits/credits/authorize-publication";
const CANONICAL: &str = "/shop-api/v1/credit-authorizations/publication";
const COLLECTION_MANAGER: &str = "0x9d32aac179153a991e832550d9f96441ea27763a";
const AMOY_COLLECTION_MANAGER: &str = "0x5309ae874fc4eb21adcd63f8b6c3f766cc3b1849";
const COLLECTION_STORE: &str = "0x214ffc0f0103735728dc66b61a22e4f163e275ae";
const COLLECTION: &str = "0x00000000000000000000000000000000000c011e";
const FEE_CENTS: i64 = 2_005;
const CHARGED_CENTS: i64 = 2_010;

fn manager() -> String {
    format!("{DCL_ONE_POLYGON_MANAGER:#x}")
}

fn fresh_key() -> (String, Address) {
    let signer = PrivateKeySigner::random();
    (hex::encode(signer.to_bytes()), signer.address())
}

/// The builder's answer: `creator`'s draft costs [`FEE_CENTS`]; any other
/// signer is refused.
struct OneDraft {
    creator: String,
    asked: Mutex<Vec<Vec<u8>>>,
}

#[async_trait]
impl Publications for OneDraft {
    async fn quote(&self, signer: &Signer, data: &[u8]) -> Result<PublicationQuote, ApiError> {
        self.asked.lock().unwrap().push(data.to_vec());
        if signer.as_str() != self.creator {
            return Err(ApiError::forbidden(
                "The publication names another wallet as its creator",
            ));
        }
        Ok(PublicationQuote {
            contract: EvmAddress::parse(COLLECTION).unwrap(),
            usd_cents: FEE_CENTS,
        })
    }
}

/// A manager that allows no publication while it issues purchase credits.
struct PurchasesOnly;

#[async_trait]
impl IssuanceGate for PurchasesOnly {
    async fn check(&self, _manager: Address, _signer: Address) -> Result<(), String> {
        Ok(())
    }

    async fn check_publication(
        &self,
        _manager: Address,
        _signer: Address,
        collection_manager: Address,
    ) -> Result<(), String> {
        Err(format!(
            "the manager does not allow createCollection on {collection_manager:#x} as a custom external call"
        ))
    }
}

struct Lane {
    _db: OwnDatabase,
    pool: PgPool,
    state: SharedState,
    app: Router,
    creator: Wallet,
    signer: Address,
    co_signer: Address,
    quotes: Arc<OneDraft>,
}

#[derive(Default)]
struct Without {
    co_signer: bool,
    builder: bool,
    writes: bool,
    publication_grants: bool,
}

async fn lane(without: Without) -> Option<Lane> {
    let db = OwnDatabase::create("publication").await?;
    let pool = db.pool.clone();
    let (signer_key, signer) = fresh_key();
    let (co_signer_key, co_signer) = fresh_key();
    let mut settings = Settings::parse(RawSettings {
        writes_enabled: !without.writes,
        credits_onchain_authorize_enabled: true,
        credits_signer_key: Some(signer_key),
        credits_external_call_signer_key: (!without.co_signer).then_some(co_signer_key),
        credits_manager_contract: Some(manager()),
        mana_price_max_staleness_secs: 3600,
        ..RawSettings::default()
    })
    .unwrap();
    settings.usage = super::usage::Settings {
        enabled: true,
        rpc_url: Some("http://127.0.0.1:9/".into()),
        chain_id: 137,
        contract: Some(manager()),
        ..super::usage::Settings::default()
    };
    sqlx::query("INSERT INTO credits.usage_index_cursor (chain_id, contract, last_block, folded_block, folded_at) VALUES (137, $1, 100, 100, timezone('UTC', now()))")
        .bind(manager())
        .execute(&pool)
        .await
        .unwrap();
    let gate: Arc<dyn IssuanceGate> = if without.publication_grants {
        Arc::new(PurchasesOnly)
    } else {
        Arc::new(FixedGate(Ok(())))
    };
    let state: SharedState = Arc::new(
        State::with_sources(
            pool.clone(),
            Outbound::for_tests(),
            settings,
            Arc::new(FakeMarket::default().with_mana_usd("0.5")),
            Arc::new(MockBroker::configured()),
            None,
        )
        .with_gate(gate),
    );
    let creator = scratch_wallet();
    let quotes = Arc::new(OneDraft {
        creator: creator.address.clone(),
        asked: Mutex::default(),
    });
    if !without.builder {
        assert!(state.attach_publications(quotes.clone()));
    }
    seed_wallet(&pool, &creator.address, "0", "3000").await;
    let app = app(state.clone()).nest("/shop-api", super::shop_router(state.clone()));
    Some(Lane {
        _db: db,
        pool,
        state,
        app,
        creator,
        signer,
        co_signer,
        quotes,
    })
}

fn create_collection(creator: &str, name: &str) -> Vec<u8> {
    createCollectionCall {
        forwarder: Address::repeat_byte(0xf0),
        factory: Address::repeat_byte(0xfa),
        salt: B256::repeat_byte(3),
        name: name.into(),
        symbol: "DCL-PBL".into(),
        baseURI: "https://peer.decentraland.org/lambdas/collections/standard/erc721/".into(),
        creator: creator.parse().unwrap(),
        items: vec![PublicationItem {
            rarity: "common".into(),
            price: U256::ZERO,
            beneficiary: Address::ZERO,
            metadata: "1:w:Hat::hat:BaseMale".into(),
        }],
    }
    .abi_encode()[4..]
        .to_vec()
}

fn in_a_day() -> u64 {
    chrono::Utc::now().timestamp() as u64 + 86_400
}

fn body(creator: &str) -> Value {
    json!({
        "usdPriceCents": FEE_CENTS,
        "chainId": 137,
        "creditsManagerAddress": manager(),
        "externalCall": {
            "target": COLLECTION_MANAGER,
            "selector": "0x35a629c2",
            "data": format!("0x{}", hex::encode(create_collection(creator, "Publication"))),
            "expiresAt": in_a_day(),
            "salt": format!("0x{}", "07".repeat(32)),
        },
    })
}

fn with(mut body: Value, path: &[&str], value: Value) -> Value {
    let (last, parents) = path.split_last().unwrap();
    let mut at = &mut body;
    for key in parents {
        at = &mut at[*key];
    }
    at[*last] = value;
    body
}

async fn authorizations(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM credits.authorizations")
        .fetch_one(pool)
        .await
        .unwrap()
}

/// The `externalCall` of `body` as `useCredits` carries it, for `sender` on
/// `chain_id` and `manager`.
fn sent_call(body: &Value, sender: &str, chain_id: u64, manager: Address) -> ExternalCallIdentity {
    let call = &body["externalCall"];
    let bytes = |key: &str| hex::decode(&call[key].as_str().unwrap()[2..]).unwrap();
    ExternalCallIdentity {
        sender: sender.parse().unwrap(),
        chain_id,
        manager,
        target: Address::from_slice(&bytes("target")),
        selector: bytes("selector").try_into().unwrap(),
        data: bytes("data"),
        expires_at: U256::from(call["expiresAt"].as_u64().unwrap()),
        salt: B256::from_slice(&bytes("salt")),
    }
}

#[tokio::test]
async fn a_creator_gets_one_credit_and_a_co_signature_for_the_fee_of_their_draft() {
    let Some(lane) = lane(Without::default()).await else {
        return;
    };
    let request = body(&lane.creator.address);
    let headers = signed_headers(&lane.creator, "post", LEGACY).await;
    let (status, _, v) = send(
        &lane.app,
        "POST",
        LEGACY,
        headers.clone(),
        Some(request.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");

    let mut keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        ["credit", "externalCallSignature", "oracleRate", "usdCents"],
        "credits-server's answer"
    );
    assert_eq!(v["usdCents"], CHARGED_CENTS, "rounded up to whole credits");
    assert_eq!(v["oracleRate"], "500000000000000000");
    assert_eq!(
        v["credit"]["amount"], "40902000000000000000",
        "20.05 USD at 0.5 USD/MANA is 40.1 MANA, plus the 2% buffer"
    );
    assert_eq!(v["credit"]["availableAmount"], v["credit"]["amount"]);
    assert_eq!(v["credit"]["contract"], manager());
    let expires = v["credit"]["expiresAt"].as_u64().unwrap();
    let now = chrono::Utc::now().timestamp() as u64;
    assert!(
        (now + 290..=now + 300).contains(&expires),
        "300 s: {expires}"
    );

    let salt = parse_bytes32(v["credit"]["id"].as_str().unwrap()).unwrap();
    let credit = CreditIdentity {
        sender: lane.creator.address.parse().unwrap(),
        chain_id: 137,
        manager: DCL_ONE_POLYGON_MANAGER,
        value: U256::from_str_radix(v["credit"]["amount"].as_str().unwrap(), 10).unwrap(),
        expires_at: expires,
        salt,
    };
    assert_eq!(
        recover_credit_signer(
            credit.credit_hash(),
            v["credit"]["signature"].as_str().unwrap()
        ),
        Some(lane.signer),
        "the credit is the configured signer's"
    );
    let sent = sent_call(
        &request,
        &lane.creator.address,
        137,
        DCL_ONE_POLYGON_MANAGER,
    );
    assert_eq!(
        recover_credit_signer(sent.hash(), v["externalCallSignature"].as_str().unwrap()),
        Some(lane.co_signer),
        "the call is co-signed by the configured external-call signer"
    );
    assert_eq!(
        lane.quotes.asked.lock().unwrap().as_slice(),
        std::slice::from_ref(&sent.data),
        "the builder priced the call's own arguments"
    );

    let row: (String, i64, i64, String, String, String) = sqlx::query_as(
        "SELECT a.status, a.usd_cents, a.quoted_cents, a.request_digest, l.sale_kind || '/' || l.asset_kind, l.asset_contract \
         FROM credits.authorizations a JOIN credits.authorization_lines l ON l.authorization_id = a.id WHERE a.id = $1",
    )
    .bind(v["credit"]["id"].as_str().unwrap())
    .fetch_one(&lane.pool)
    .await
    .unwrap();
    assert_eq!(
        row,
        (
            "authorized".to_string(),
            CHARGED_CENTS,
            FEE_CENTS,
            format!("{:#x}", sent.hash()),
            "publication_fee/collection".to_string(),
            COLLECTION.to_string()
        )
    );

    let (status, _, again) = send(&lane.app, "POST", LEGACY, headers, Some(request.clone())).await;
    assert_eq!(status, StatusCode::OK, "{again}");
    assert_eq!(again, v, "the same signed request replays its credit");
    assert_eq!(authorizations(&lane.pool).await, 1);

    let (status, canonical) = call(
        &lane.app,
        &lane.creator,
        "post",
        CANONICAL,
        &[],
        Some(request),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{canonical}");
    assert_ne!(canonical["credit"]["id"], v["credit"]["id"]);
    assert_eq!(authorizations(&lane.pool).await, 2);
}

#[tokio::test]
async fn a_request_the_rules_refuse_reserves_and_signs_nothing() {
    let Some(lane) = lane(Without::default()).await else {
        return;
    };
    let own = || body(&lane.creator.address);
    let refusals = [
        (
            "another fee",
            with(own(), &["usdPriceCents"], json!(FEE_CENTS - 5)),
            409,
            "Price changed",
        ),
        (
            "another chain",
            with(own(), &["chainId"], json!(80_002)),
            400,
            "Unsupported chain",
        ),
        (
            "a shared manager",
            with(
                own(),
                &["creditsManagerAddress"],
                json!("0x8b3a40ca1b6f5cafc99d112a4d02e897d1fd8cc5"),
            ),
            400,
            "Invalid credits manager address",
        ),
        (
            "another target",
            with(own(), &["externalCall", "target"], json!(COLLECTION_STORE)),
            400,
            "Invalid external call target: must be CollectionManager",
        ),
        (
            "another selector",
            with(own(), &["externalCall", "selector"], json!("0xa9059cbb")),
            400,
            "Invalid function selector: must be createCollection",
        ),
        (
            "an expired call",
            with(own(), &["externalCall", "expiresAt"], json!(1)),
            400,
            "The external call has expired",
        ),
        (
            "a property the schema lacks",
            with(own(), &["operationId"], json!("x")),
            400,
            "",
        ),
        (
            "a salt that is not 32 bytes",
            with(own(), &["externalCall", "salt"], json!("0x07")),
            400,
            "\"externalCall.salt\" must be 32 bytes of hex",
        ),
    ];
    for (case, request, status, error) in refusals {
        let (answered, v) =
            call(&lane.app, &lane.creator, "post", LEGACY, &[], Some(request)).await;
        assert_eq!(answered.as_u16(), status, "{case}: {v}");
        if !error.is_empty() {
            assert_eq!(v["error"], error, "{case}");
        }
        assert!(v.get("credit").is_none() && v.get("externalCallSignature").is_none());
    }
    let (_, v) = call(
        &lane.app,
        &lane.creator,
        "post",
        LEGACY,
        &[],
        Some(with(own(), &["usdPriceCents"], json!(FEE_CENTS - 5))),
    )
    .await;
    assert_eq!(
        v["lines"][0],
        json!({ "index": 0, "quotedCents": FEE_CENTS, "requestedCents": FEE_CENTS - 5 })
    );

    let stranger = scratch_wallet();
    seed_wallet(&lane.pool, &stranger.address, "0", "1000").await;
    let (status, v) = call(
        &lane.app,
        &stranger,
        "post",
        LEGACY,
        &[],
        Some(body(&lane.creator.address)),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "someone else's draft: {v}");

    let (status, _, v) = send(&lane.app, "POST", LEGACY, Default::default(), Some(own())).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "unsigned: {v}");

    let broke = scratch_wallet();
    seed_wallet(&lane.pool, &broke.address, "0", "200").await;
    let poor = Arc::new(
        State::with_sources(
            lane.pool.clone(),
            Outbound::for_tests(),
            lane.state.settings.clone(),
            Arc::new(FakeMarket::default().with_mana_usd("0.5")),
            Arc::new(MockBroker::configured()),
            None,
        )
        .with_gate(Arc::new(FixedGate(Ok(())))),
    );
    poor.attach_publications(Arc::new(OneDraft {
        creator: broke.address.clone(),
        asked: Mutex::default(),
    }));
    let (status, v) = call(
        &app(poor),
        &broke,
        "post",
        LEGACY,
        &[],
        Some(body(&broke.address)),
    )
    .await;
    assert_eq!(status, StatusCode::PAYMENT_REQUIRED, "{v}");
    assert_eq!(
        (v["balanceCents"].as_i64(), v["requiredCents"].as_i64()),
        (Some(2_000), Some(CHARGED_CENTS)),
        "200 credits are 20.00 USD, a cent short of the charge"
    );

    assert_eq!(
        authorizations(&lane.pool).await,
        0,
        "no refusal reserved anything"
    );
    assert_eq!(available(&lane.pool, &lane.creator.address).await, "3000");
}

#[tokio::test]
async fn a_wallet_holds_at_most_ten_live_credits() {
    let Some(lane) = lane(Without::default()).await else {
        return;
    };
    for n in 0..10 {
        let request = with(
            body(&lane.creator.address),
            &["externalCall", "salt"],
            json!(format!("0x{n:064x}")),
        );
        let (status, v) = call(&lane.app, &lane.creator, "post", LEGACY, &[], Some(request)).await;
        assert_eq!(status, StatusCode::OK, "{n}: {v}");
    }
    let eleventh = with(
        body(&lane.creator.address),
        &["externalCall", "salt"],
        json!(format!("0x{:064x}", 11)),
    );
    let (status, v) = call(
        &lane.app,
        &lane.creator,
        "post",
        LEGACY,
        &[],
        Some(eleventh),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{v}");
    assert!(v.get("credit").is_none());
    assert_eq!(authorizations(&lane.pool).await, 10);
}

#[tokio::test]
async fn a_node_that_cannot_co_sign_or_settle_a_publication_answers_unavailable() {
    for (case, without, configure) in [
        (
            "no external-call signer key",
            Without {
                co_signer: true,
                ..Without::default()
            },
            Some("CREDITS_EXTERNAL_CALL_SIGNER_KEY_FILE"),
        ),
        (
            "no builder",
            Without {
                builder: true,
                ..Without::default()
            },
            None,
        ),
        (
            "a manager without the publication grants",
            Without {
                publication_grants: true,
                ..Without::default()
            },
            Some("CREDITS_MANAGER_CONTRACT"),
        ),
        (
            "writes closed",
            Without {
                writes: true,
                ..Without::default()
            },
            None,
        ),
    ] {
        let Some(lane) = lane(without).await else {
            return;
        };
        let (status, v) = call(
            &lane.app,
            &lane.creator,
            "post",
            LEGACY,
            &[],
            Some(body(&lane.creator.address)),
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{case}: {v}");
        assert_eq!(v["configure"].as_str(), configure, "{case}: {v}");
        assert!(
            v.get("credit").is_none() && v.get("externalCallSignature").is_none(),
            "{case}"
        );
        assert_eq!(authorizations(&lane.pool).await, 0, "{case}");
        if case == "writes closed" {
            let (closed, _, purchase) = send(
                &lane.app,
                "POST",
                "/credits/credits/authorize",
                Default::default(),
                Some(json!({})),
            )
            .await;
            assert_eq!(
                (closed, &purchase),
                (status, &v),
                "the purchase authorization's answer with writes closed"
            );
        }
    }
}

/// `useCredits` as the creators app sends it for `authorization`, relayed
/// as `sender`'s meta-transaction.
fn relayed(sender: &str, authorization: &Value, sent: &ExternalCallIdentity) -> UseCredits {
    let credit = &authorization["credit"];
    let value = U256::from_str_radix(credit["amount"].as_str().unwrap(), 10).unwrap();
    UseCredits {
        sender: sender.to_string(),
        args: credits_abi::UseCreditsArgs {
            credits: vec![credits_abi::Credit {
                value,
                expiresAt: U256::from(credit["expiresAt"].as_u64().unwrap()),
                salt: parse_bytes32(credit["id"].as_str().unwrap()).unwrap(),
            }],
            creditsSignatures: vec![Default::default()],
            externalCall: credits_abi::ExternalCall {
                target: sent.target,
                selector: sent.selector.into(),
                data: sent.data.clone().into(),
                expiresAt: sent.expires_at,
                salt: sent.salt,
            },
            customExternalCallSignature: Default::default(),
            maxUncreditedValue: U256::ZERO,
            maxCreditedValue: value,
        },
    }
}

#[derive(Clone)]
struct UseCredits {
    sender: String,
    args: credits_abi::UseCreditsArgs,
}

impl UseCredits {
    fn calldata(&self) -> String {
        let inner = credits_abi::useCreditsCall {
            _args: self.args.clone(),
        }
        .abi_encode();
        let meta = crate::evm::abi::executeMetaTransactionCall {
            userAddress: self.sender.parse().unwrap(),
            functionSignature: inner.into(),
            sigR: B256::ZERO,
            sigS: B256::ZERO,
            sigV: 27,
        };
        format!("0x{}", hex::encode(meta.abi_encode()))
    }

    fn with(&self, change: impl FnOnce(&mut credits_abi::UseCreditsArgs)) -> Self {
        let mut changed = self.clone();
        change(&mut changed.args);
        changed
    }
}

async fn relay(pool: &PgPool, chain_id: u64, manager: Address, call: &UseCredits) -> bool {
    guard_meta_transaction(
        pool,
        chain_id,
        &call.sender,
        &format!("{manager:#x}"),
        &call.calldata(),
    )
    .await
    .is_ok()
}

#[tokio::test]
async fn the_relay_accepts_exactly_the_authorized_publication() {
    let Some(lane) = lane(Without::default()).await else {
        return;
    };
    let creator = lane.creator.address.clone();
    let request = body(&creator);
    let (status, authorization) = call(
        &lane.app,
        &lane.creator,
        "post",
        LEGACY,
        &[],
        Some(request.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{authorization}");
    let sent = sent_call(&request, &creator, 137, DCL_ONE_POLYGON_MANAGER);
    let own = relayed(&creator, &authorization, &sent);
    assert!(
        relay(&lane.pool, 137, DCL_ONE_POLYGON_MANAGER, &own).await,
        "the authorized call"
    );

    let stranger = scratch_wallet().address;
    let refused = [
        (
            "another selector",
            own.with(|a| a.externalCall.selector = [0xa9, 0x05, 0x9c, 0xbb].into()),
        ),
        (
            "another target",
            own.with(|a| a.externalCall.target = COLLECTION_STORE.parse().unwrap()),
        ),
        (
            "another collection",
            own.with(|a| a.externalCall.data = create_collection(&creator, "Another").into()),
        ),
        (
            "another creator",
            own.with(|a| a.externalCall.data = create_collection(&stranger, "Publication").into()),
        ),
        (
            "another call salt",
            own.with(|a| a.externalCall.salt = B256::repeat_byte(8)),
        ),
        (
            "another call expiry",
            own.with(|a| a.externalCall.expiresAt += U256::from(1)),
        ),
        (
            "another credit value",
            own.with(|a| {
                a.credits[0].value += U256::from(1);
                a.maxCreditedValue += U256::from(1);
            }),
        ),
        (
            "another credit expiry",
            own.with(|a| a.credits[0].expiresAt += U256::from(60)),
        ),
        (
            "another credit",
            own.with(|a| a.credits[0].salt = B256::repeat_byte(9)),
        ),
        (
            "a second credit",
            own.with(|a| a.credits.push(a.credits[0].clone())),
        ),
        (
            "MANA of the sender's beside the credit",
            own.with(|a| a.maxUncreditedValue = U256::from(1)),
        ),
        (
            "more credited than the credit",
            own.with(|a| a.maxCreditedValue += U256::from(1)),
        ),
        (
            "another sender",
            UseCredits {
                sender: stranger.clone(),
                ..own.clone()
            },
        ),
    ];
    for (case, call) in &refused {
        assert!(
            !relay(&lane.pool, 137, DCL_ONE_POLYGON_MANAGER, call).await,
            "{case}"
        );
    }
    assert!(
        !relay(
            &lane.pool,
            137,
            "0x8b3a40ca1b6f5cafc99d112a4d02e897d1fd8cc5"
                .parse()
                .unwrap(),
            &own
        )
        .await,
        "the shared manager"
    );
    assert!(
        !relay(&lane.pool, 80_002, DCL_ONE_AMOY_MANAGER, &own).await,
        "another chain, where the target is no collection manager"
    );
    let on_amoy = own.with(|a| a.externalCall.target = AMOY_COLLECTION_MANAGER.parse().unwrap());
    assert!(
        !relay(&lane.pool, 80_002, DCL_ONE_AMOY_MANAGER, &on_amoy).await,
        "another chain's collection manager and credits manager"
    );

    let id = authorization["credit"]["id"].as_str().unwrap();
    for status in ["retired", "consumed", "released", "expired"] {
        sqlx::query("UPDATE credits.authorizations SET status = $2 WHERE id = $1")
            .bind(id)
            .bind(status)
            .execute(&lane.pool)
            .await
            .unwrap();
        assert!(
            !relay(&lane.pool, 137, DCL_ONE_POLYGON_MANAGER, &own).await,
            "a {status} authorization"
        );
    }
    sqlx::query("UPDATE credits.authorizations SET status = 'authorized' WHERE id = $1")
        .bind(id)
        .execute(&lane.pool)
        .await
        .unwrap();
    assert!(relay(&lane.pool, 137, DCL_ONE_POLYGON_MANAGER, &own).await);
    sqlx::query("UPDATE credits.authorizations SET address = $2 WHERE id = $1")
        .bind(id)
        .bind(&stranger)
        .execute(&lane.pool)
        .await
        .unwrap();
    assert!(
        !relay(&lane.pool, 137, DCL_ONE_POLYGON_MANAGER, &own).await,
        "an authorization issued to another wallet"
    );
    sqlx::query("UPDATE credits.authorizations SET address = $2 WHERE id = $1")
        .bind(id)
        .bind(&creator)
        .execute(&lane.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE credits.authorization_lines SET sale_kind = 'primary_mint' WHERE authorization_id = $1")
        .bind(id)
        .execute(&lane.pool)
        .await
        .unwrap();
    assert!(
        !relay(&lane.pool, 137, DCL_ONE_POLYGON_MANAGER, &own).await,
        "a credit issued for a purchase"
    );
}

#[tokio::test]
async fn a_publication_credit_settles_and_expires_as_a_purchase_credit_does() {
    let Some(lane) = lane(Without::default()).await else {
        return;
    };
    let authorize = |salt: u8| {
        let request = with(
            body(&lane.creator.address),
            &["externalCall", "salt"],
            json!(format!("0x{}", hex::encode([salt; 32]))),
        );
        let (app, creator) = (&lane.app, &lane.creator);
        async move {
            let (status, v) = call(app, creator, "post", LEGACY, &[], Some(request)).await;
            assert_eq!(status, StatusCode::OK, "{v}");
            v["credit"]["id"].as_str().unwrap().to_string()
        }
    };
    let status_of = |id: String| {
        let pool = &lane.pool;
        async move {
            sqlx::query_scalar::<_, String>(
                "SELECT status FROM credits.authorizations WHERE id = $1",
            )
            .bind(id)
            .fetch_one(pool)
            .await
            .unwrap()
        }
    };

    let used = authorize(1).await;
    sqlx::query(
        "INSERT INTO credits.usages (chain_id, block_number, log_index, contract, tx_hash, sender, credit_id, salt, \
           credit_value, credit_expires_at, value_wei) \
         SELECT chain_id, 1, 1, contract, $2, address, credit_hash, id, amount_wei, extract(epoch FROM expires_at), amount_wei \
         FROM credits.authorizations WHERE id = $1",
    )
    .bind(&used)
    .bind(format!("0x{}", "12".repeat(32)))
    .execute(&lane.pool)
    .await
    .unwrap();
    let settled = super::settlement::settle(&lane.state.credits, 137, &manager(), 0, 1)
        .await
        .unwrap();
    assert_eq!(settled.len(), 1, "{settled:?}");
    assert_eq!(status_of(used).await, "consumed");
    assert_eq!(
        available(&lane.pool, &lane.creator.address).await,
        "2799",
        "3000 credits less the 201 the fee was charged"
    );

    let lapsed = authorize(2).await;
    sqlx::query("UPDATE credits.authorizations SET expires_at = timezone('UTC', now()) - interval '1 hour' WHERE id = $1")
        .bind(&lapsed)
        .execute(&lane.pool)
        .await
        .unwrap();
    assert_eq!(
        super::store::Credits::new(lane.pool.clone())
            .expire_stale_authorizations()
            .await
            .unwrap(),
        1
    );
    assert_eq!(status_of(lapsed).await, "expired");
    assert_eq!(available(&lane.pool, &lane.creator.address).await, "2799");
}
