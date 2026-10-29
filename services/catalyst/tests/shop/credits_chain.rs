// from: offchain-marketplace-contract@87350c63e:test/credits/CreditsManagerPolygon.useCredits.{collectionStore,legacyMarketplace}.t.sol (the calls, against our own deployment instead of a Polygon fork), docs/shop/CREDIT-HASH.md, docs/shop/CONTRACTS.md
//! The controlled-contract test (SHOP.md slice 4, "contract exclusion
//! verified"): the PINNED `CreditsManagerPolygon` deployed on a local anvil
//! with `secondarySalesAllowed = false` and our treasury signer holding
//! `CREDITS_SIGNER_ROLE`, beside the test doubles of
//! `tests/shop/anvil/Mocks.sol` (compiled by `tests/shop/anvil/build.sh`
//! into `tests/fixtures/shop/anvil/artifacts.json`). It proves, on the real
//! bytecode:
//!
//! - a credit signed by `shop::credits::digest` is accepted: `useCredits`
//!   over `CollectionStore.buy` succeeds, `CreditUsed` carries our hash as
//!   `_creditId`, and `usage::decode_log` reads the salt and value back;
//! - the same credit over the legacy marketplace's `executeOrder` (a
//!   resale) reverts `SecondarySalesNotAllowed()` before decoding the order;
//! - the old v2 text digest's signature reverts `InvalidSignature`;
//! - `settlement::ChainGate` opens on this deployment and closes on a twin
//!   deployed with `secondarySalesAllowed = true`;
//! - `settlement::Revoker` sends `revokeCredits` for the hash and the
//!   contract then refuses the credit (`RevokedCredit`).
//!
//! Needs `anvil` (foundry) on PATH or `ANVIL_BIN`; otherwise the test says so
//! and passes vacuously. No network, no live key: anvil's unlocked accounts
//! and the upstream test key ("mock, safe to use") only.

use std::process::{Command, Stdio};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use alloy_primitives::{Address, B256, U256, keccak256};
use alloy_sol_types::{SolCall, SolValue, sol};
use catalyst::http::Outbound;
use catalyst::shop::credits::digest::{CreditIdentity, TreasuryWallet, bytes32_hex};
use catalyst::shop::credits::settlement::{
    ChainGate, IssuanceGate, credits_signer_role, revoke_calldata,
};
use catalyst::shop::credits::usage::{CREDIT_USED_TOPIC, RpcLog, decode_log};
use catalyst::shop::evm::chain::ChainComponent;
use serde_json::{Value, json};

sol! {
    struct Roles {
        address owner;
        address creditsSigner;
        address pauser;
        address userDenier;
        address creditsRevoker;
        address customExternalCallSigner;
        address customExternalCallRevoker;
    }
    struct Credit {
        uint256 value;
        uint256 expiresAt;
        bytes32 salt;
    }
    struct ExternalCall {
        address target;
        bytes4 selector;
        bytes data;
        uint256 expiresAt;
        bytes32 salt;
    }
    struct UseCreditsArgs {
        Credit[] credits;
        bytes[] creditsSignatures;
        ExternalCall externalCall;
        bytes customExternalCallSignature;
        uint256 maxUncreditedValue;
        uint256 maxCreditedValue;
    }
    struct ItemToBuy {
        address collection;
        uint256[] ids;
        uint256[] prices;
        address[] beneficiaries;
    }
    function useCredits(UseCreditsArgs args);
    function buy(ItemToBuy[] items);
    function executeOrder(address contractAddress, uint256 tokenId, uint256 price);
    function setCollection(address collection, bool ok);
    function transfer(address to, uint256 amount) returns (bool);
    function balanceOf(address owner) returns (uint256);
    function bought() returns (uint256);
    function isRevoked(bytes32 creditHash) returns (bool);
    function secondarySalesAllowed() returns (bool);
    function hasRole(bytes32 role, address account) returns (bool);
}

const ARTIFACTS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/shop/anvil/artifacts.json"
);
/// credits-server's committed test key (`test/unit/logic/signer.spec.ts`, "mock, safe to use"): 0xb22c...0c58.
const SIGNER_KEY: &str = "0x057b5accbdf706b07a6515dd14de77dd7ab1972ac90c897d7c24ae68848a05e2";
/// A second throwaway key for the revoker role.
const REVOKER_KEY: &str = "0x4c0883a69102937d6231471b5dbb6204fe512961708279f2e3e8a5d4b8e3e3e3";
const CHAIN_ID: u64 = 31337;
const MANA: u128 = 1_000_000_000_000_000_000;

fn anvil_bin() -> Option<String> {
    if let Ok(bin) = std::env::var("ANVIL_BIN") {
        return Some(bin);
    }
    let ok = Command::new("anvil")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    ok.then(|| "anvil".to_string())
}

struct Anvil {
    child: std::process::Child,
    url: String,
    http: reqwest::Client,
}

impl Drop for Anvil {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Anvil {
    async fn start(bin: &str) -> Self {
        let port = {
            let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            l.local_addr().unwrap().port()
        };
        let child = Command::new(bin)
            .args([
                "--port",
                &port.to_string(),
                "--chain-id",
                &CHAIN_ID.to_string(),
                "--silent",
                "--host",
                "127.0.0.1",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("anvil spawns");
        let anvil = Self {
            child,
            url: format!("http://127.0.0.1:{port}"),
            http: reqwest::Client::new(),
        };
        for _ in 0..100 {
            if anvil.try_rpc("eth_chainId", json!([])).await.is_ok() {
                return anvil;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        panic!("anvil did not come up on {}", anvil.url);
    }

    async fn try_rpc(&self, method: &str, params: Value) -> Result<Value, String> {
        let body = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
        let v: Value = self
            .http
            .post(&self.url)
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())?;
        if let Some(err) = v.get("error") {
            return Err(err.to_string());
        }
        Ok(v["result"].clone())
    }

    async fn rpc(&self, method: &str, params: Value) -> Value {
        self.try_rpc(method, params)
            .await
            .unwrap_or_else(|e| panic!("{method}: {e}"))
    }

    /// `eth_sendTransaction` from an unlocked account, mined at once; the receipt.
    async fn send(&self, from: Address, to: Option<Address>, data: &[u8]) -> Value {
        let mut tx = json!({ "from": format!("{from:#x}"), "data": format!("0x{}", hex::encode(data)), "gas": "0x1c9c380" });
        if let Some(to) = to {
            tx["to"] = json!(format!("{to:#x}"));
        }
        let hash = self.rpc("eth_sendTransaction", json!([tx])).await;
        for _ in 0..100 {
            let receipt = self.rpc("eth_getTransactionReceipt", json!([hash])).await;
            if !receipt.is_null() {
                return receipt;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!("no receipt for {hash}");
    }

    async fn deploy(&self, from: Address, bytecode: &str, args: &[u8]) -> Address {
        let mut code = hex::decode(bytecode.trim_start_matches("0x")).unwrap();
        code.extend_from_slice(args);
        let receipt = self.send(from, None, &code).await;
        assert_eq!(receipt["status"], "0x1", "deploy reverted: {receipt}");
        Address::from_str(receipt["contractAddress"].as_str().unwrap()).unwrap()
    }

    /// `eth_call`'s revert data (the 4-byte error selector first) or `Ok(bytes)`.
    async fn call(&self, from: Address, to: Address, data: &[u8]) -> Result<Vec<u8>, Vec<u8>> {
        let body = json!({ "jsonrpc": "2.0", "id": 1, "method": "eth_call", "params": [{ "from": format!("{from:#x}"), "to": format!("{to:#x}"), "data": format!("0x{}", hex::encode(data)) }, "latest"] });
        let v: Value = self
            .http
            .post(&self.url)
            .json(&body)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        if let Some(err) = v.get("error") {
            let data = err
                .get("data")
                .and_then(|d| {
                    d.as_str()
                        .map(str::to_string)
                        .or_else(|| d.get("data").and_then(|x| x.as_str()).map(str::to_string))
                })
                .unwrap_or_default();
            return Err(hex::decode(data.trim_start_matches("0x")).unwrap_or_default());
        }
        Ok(hex::decode(v["result"].as_str().unwrap().trim_start_matches("0x")).unwrap())
    }

    async fn call_ok(&self, from: Address, to: Address, data: &[u8]) -> Vec<u8> {
        self.call(from, to, data)
            .await
            .unwrap_or_else(|e| panic!("call reverted: 0x{}", hex::encode(e)))
    }
}

fn selector(sig: &str) -> [u8; 4] {
    keccak256(sig.as_bytes())[..4].try_into().unwrap()
}

fn artifacts() -> Option<Value> {
    let text = std::fs::read_to_string(ARTIFACTS).ok()?;
    Some(serde_json::from_str(&text).expect("artifacts.json parses"))
}

fn bytecode<'a>(arts: &'a Value, name: &str) -> &'a str {
    arts["contracts"][name]["bytecode"]
        .as_str()
        .unwrap_or_else(|| panic!("{name} bytecode"))
}

struct Deployment {
    manager: Address,
    mana: Address,
    store: Address,
    legacy: Address,
    collection: Address,
    deployer: Address,
    buyer: Address,
    seller: Address,
}

async fn deploy_all(
    anvil: &Anvil,
    arts: &Value,
    secondary_sales_allowed: bool,
    signer: Address,
    revoker: Address,
) -> Deployment {
    let accounts: Vec<Address> = anvil
        .rpc("eth_accounts", json!([]))
        .await
        .as_array()
        .unwrap()
        .iter()
        .map(|a| Address::from_str(a.as_str().unwrap()).unwrap())
        .collect();
    let (deployer, buyer, seller, collection) =
        (accounts[0], accounts[1], accounts[2], accounts[3]);
    let mana = anvil
        .deploy(
            deployer,
            bytecode(arts, "MockMana"),
            &(U256::from(1_000_000u64 * MANA as u64)).abi_encode(),
        )
        .await;
    let factory = anvil
        .deploy(deployer, bytecode(arts, "MockCollectionFactory"), &[])
        .await;
    let store = anvil
        .deploy(
            deployer,
            bytecode(arts, "MockCollectionStore"),
            &(mana,).abi_encode_params(),
        )
        .await;
    let legacy = anvil
        .deploy(deployer, bytecode(arts, "MockLegacyMarketplace"), &[])
        .await;
    let roles = Roles {
        owner: deployer,
        creditsSigner: signer,
        pauser: deployer,
        userDenier: deployer,
        creditsRevoker: revoker,
        customExternalCallSigner: deployer,
        customExternalCallRevoker: deployer,
    };
    let args = (
        roles,
        U256::from(1_000u64 * MANA as u64),
        true,
        secondary_sales_allowed,
        mana,
        legacy,
        store,
        factory,
        factory,
        Vec::<Address>::new(),
    )
        .abi_encode_params();
    let manager = anvil
        .deploy(deployer, bytecode(arts, "CreditsManagerPolygon"), &args)
        .await;
    let r = anvil
        .send(
            deployer,
            Some(factory),
            &setCollectionCall {
                collection,
                ok: true,
            }
            .abi_encode(),
        )
        .await;
    assert_eq!(r["status"], "0x1");
    let r = anvil
        .send(
            deployer,
            Some(mana),
            &transferCall {
                to: manager,
                amount: U256::from(100u64 * MANA as u64),
            }
            .abi_encode(),
        )
        .await;
    assert_eq!(r["status"], "0x1");
    Deployment {
        manager,
        mana,
        store,
        legacy,
        collection,
        deployer,
        buyer,
        seller,
    }
}

fn store_buy(d: &Deployment, price: U256) -> ExternalCall {
    let items = vec![ItemToBuy {
        collection: d.collection,
        ids: vec![U256::ZERO],
        prices: vec![price],
        beneficiaries: vec![d.seller],
    }];
    ExternalCall {
        target: d.store,
        selector: selector("buy((address,uint256[],uint256[],address[])[])").into(),
        data: items.abi_encode().into(),
        expiresAt: U256::ZERO,
        salt: B256::ZERO,
    }
}

fn use_credits(identity: &CreditIdentity, signature: &str, call: ExternalCall) -> Vec<u8> {
    let credit = Credit {
        value: identity.value,
        expiresAt: U256::from(identity.expires_at),
        salt: identity.salt,
    };
    let sig = hex::decode(signature.trim_start_matches("0x")).unwrap();
    useCreditsCall {
        args: UseCreditsArgs {
            credits: vec![credit],
            creditsSignatures: vec![sig.into()],
            externalCall: call,
            customExternalCallSignature: Default::default(),
            maxUncreditedValue: U256::ZERO,
            maxCreditedValue: identity.value,
        },
    }
    .abi_encode()
}

#[tokio::test]
async fn the_pinned_manager_accepts_our_credit_for_a_mint_and_refuses_a_resale() {
    let Some(bin) = anvil_bin() else {
        eprintln!("skipped: anvil not on PATH (nix shell nixpkgs#foundry, or ANVIL_BIN)");
        return;
    };
    let Some(arts) = artifacts() else {
        eprintln!("skipped: {ARTIFACTS} missing (tests/shop/anvil/build.sh)");
        return;
    };
    assert_eq!(
        arts["pins"]["offchain-marketplace-contract"],
        "87350c63efdc93bb2c6e5010ef07a59650c67522"
    );
    let anvil = Anvil::start(&bin).await;
    let wallet = TreasuryWallet::from_hex(SIGNER_KEY).unwrap();
    let revoker_wallet = TreasuryWallet::from_hex(REVOKER_KEY).unwrap();
    let d = deploy_all(
        &anvil,
        &arts,
        false,
        wallet.signer(),
        revoker_wallet.signer(),
    )
    .await;

    let has_role = anvil
        .call_ok(
            d.deployer,
            d.manager,
            &hasRoleCall {
                role: credits_signer_role(),
                account: wallet.signer(),
            }
            .abi_encode(),
        )
        .await;
    assert!(
        bool::abi_decode(&has_role).unwrap(),
        "our signer holds CREDITS_SIGNER_ROLE on the test deployment"
    );
    let secondary = anvil
        .call_ok(
            d.deployer,
            d.manager,
            &secondarySalesAllowedCall {}.abi_encode(),
        )
        .await;
    assert!(
        !bool::abi_decode(&secondary).unwrap(),
        "the test deployment has secondarySalesAllowed = false, as CONTRACTS.md requires of a manager we issue for"
    );

    // ONE credit for the group, sized above the price like the batch route sizes it.
    let now = anvil
        .rpc("eth_getBlockByNumber", json!(["latest", false]))
        .await["timestamp"]
        .as_str()
        .map(|t| u64::from_str_radix(t.trim_start_matches("0x"), 16).unwrap())
        .unwrap();
    let salt = B256::from_slice(&keccak256(b"controlled-contract-test-salt")[..]);
    let identity = CreditIdentity {
        sender: d.buyer,
        chain_id: CHAIN_ID,
        manager: d.manager,
        value: U256::from(10u64 * MANA as u64),
        expires_at: now + 300,
        salt,
    };
    let signature = wallet.sign_credit(&identity).unwrap();
    let price = U256::from(5u64 * MANA as u64);

    // A resale first: refused by the flag, before any order is decoded.
    let resale = ExternalCall {
        target: d.legacy,
        selector: selector("executeOrder(address,uint256,uint256)").into(),
        data: (d.collection, U256::from(1), price)
            .abi_encode_params()
            .into(),
        expiresAt: U256::ZERO,
        salt: B256::ZERO,
    };
    let err = anvil
        .call(
            d.buyer,
            d.manager,
            &use_credits(&identity, &signature, resale),
        )
        .await
        .expect_err("a resale with credits must revert");
    assert_eq!(
        &err[..4],
        &selector("SecondarySalesNotAllowed()"),
        "revert data 0x{}",
        hex::encode(&err)
    );

    // The old v2 text digest: a signature the contract does not recover to the signer.
    let old_text = format!(
        "{}{:#x}{:#x}{}{}",
        bytes32_hex(salt),
        d.manager,
        d.buyer,
        identity.value,
        identity.expires_at * 1000
    );
    let old_signature = wallet
        .sign_message(keccak256(old_text.as_bytes()).as_slice())
        .unwrap();
    let err = anvil
        .call(
            d.buyer,
            d.manager,
            &use_credits(&identity, &old_signature, store_buy(&d, price)),
        )
        .await
        .expect_err("the v2 digest never verified");
    assert_eq!(
        &err[..4],
        &selector("InvalidSignature(bytes32,address)"),
        "revert data 0x{}",
        hex::encode(&err)
    );

    // The mint: accepted, MANA leaves the manager, CreditUsed names our hash.
    let receipt = anvil
        .send(
            d.buyer,
            Some(d.manager),
            &use_credits(&identity, &signature, store_buy(&d, price)),
        )
        .await;
    assert_eq!(
        receipt["status"], "0x1",
        "useCredits over CollectionStore.buy: {receipt}"
    );
    let logs = receipt["logs"].as_array().unwrap();
    let used: Vec<&Value> = logs
        .iter()
        .filter(|l| {
            l["topics"][0]
                .as_str()
                .map(str::to_ascii_lowercase)
                .as_deref()
                == Some(CREDIT_USED_TOPIC)
        })
        .collect();
    assert_eq!(used.len(), 1, "one CreditUsed: {logs:?}");
    let log = used[0];
    assert_eq!(
        log["topics"][2].as_str().unwrap().to_ascii_lowercase(),
        bytes32_hex(identity.credit_hash()),
        "the indexed _creditId is the digest we sign"
    );
    let rpc_log: RpcLog = serde_json::from_value(json!({
        "address": log["address"], "topics": log["topics"], "data": log["data"], "blockNumber": log["blockNumber"], "logIndex": log["logIndex"],
        "transactionHash": log["transactionHash"], "blockTimestamp": null, "removed": false
    }))
    .unwrap();
    let usage = decode_log(&rpc_log).expect("the index decodes the real log");
    assert_eq!(
        usage.salt,
        bytes32_hex(salt),
        "the index keys on the salt the client submits"
    );
    assert_eq!(usage.credit_id, bytes32_hex(identity.credit_hash()));
    assert_eq!(
        usage.value_wei,
        price.to_string(),
        "the partial use is the price, not the credit's value"
    );
    assert_eq!(
        usage.credit_expires_at,
        identity.expires_at.to_string(),
        "seconds, as signed"
    );
    assert_eq!(usage.sender, format!("{:#x}", d.buyer));
    let bought = anvil
        .call_ok(d.deployer, d.store, &boughtCall {}.abi_encode())
        .await;
    assert_eq!(U256::abi_decode(&bought).unwrap(), U256::from(1));
    let seller_balance = anvil
        .call_ok(
            d.deployer,
            d.mana,
            &balanceOfCall { owner: d.seller }.abi_encode(),
        )
        .await;
    assert_eq!(
        U256::abi_decode(&seller_balance).unwrap(),
        price,
        "the beneficiary received the price from the manager's MANA"
    );

    // The remainder (5 MANA of the 10 signed) is still spendable by the buyer: a second direct use goes through.
    let receipt = anvil
        .send(
            d.buyer,
            Some(d.manager),
            &use_credits(
                &identity,
                &signature,
                store_buy(&d, U256::from(2u64 * MANA as u64)),
            ),
        )
        .await;
    assert_eq!(
        receipt["status"], "0x1",
        "the contract lets the buyer spend the remainder (docs/shop/CONTRACTS.md): settlement must charge it and revocation must close it"
    );

    // The gate: open on this deployment, closed on a twin with secondary spending on, closed for a stranger signer.
    let http = Outbound::for_tests();
    let chain = Arc::new(ChainComponent::new(
        http.clone(),
        &anvil.url,
        Duration::from_secs(10),
    ));
    let gate = ChainGate::new(chain.clone(), CHAIN_ID);
    assert_eq!(gate.check(d.manager, wallet.signer()).await, Ok(()));
    let stranger = ChainGate::new(chain.clone(), CHAIN_ID);
    let why = stranger
        .check(d.manager, revoker_wallet.signer())
        .await
        .unwrap_err();
    assert!(why.contains("CREDITS_SIGNER_ROLE"), "{why}");
    let twin = deploy_all(
        &anvil,
        &arts,
        true,
        wallet.signer(),
        revoker_wallet.signer(),
    )
    .await;
    let why = ChainGate::new(chain.clone(), CHAIN_ID)
        .check(twin.manager, wallet.signer())
        .await
        .unwrap_err();
    assert!(why.contains("secondarySalesAllowed() is true"), "{why}");
    let why = ChainGate::new(chain.clone(), 137)
        .check(d.manager, wallet.signer())
        .await
        .unwrap_err();
    assert!(why.contains("serves chain 31337"), "{why}");
    let shared = Address::from_str("0x8b3a40ca1b6f5cafc99d112a4d02e897d1fd8cc5").unwrap();
    let why = ChainGate::new(chain.clone(), CHAIN_ID)
        .check(shared, wallet.signer())
        .await
        .unwrap_err();
    assert!(why.contains("shared upstream manager"), "{why}");

    // Revocation: revokeCredits from the revoker role, then the credit is refused.
    anvil
        .rpc(
            "anvil_setBalance",
            json!([
                format!("{:#x}", revoker_wallet.signer()),
                "0x56bc75e2d63100000"
            ]),
        )
        .await;
    let signer = catalyst::shop::evm::signer::DirectSigner::new(
        chain.clone(),
        REVOKER_KEY,
        CHAIN_ID,
        300_000,
        Duration::from_millis(200),
        Duration::from_secs(30),
    )
    .unwrap();
    let tx = signer
        .send_direct_call(d.manager, revoke_calldata(identity.credit_hash()))
        .await
        .expect("revokeCredits broadcast");
    let receipt = signer.await_receipt(&tx).await.unwrap();
    assert_eq!(
        receipt,
        catalyst::shop::evm::signer::ReceiptOutcome::Confirmed
    );
    let revoked = anvil
        .call_ok(
            d.deployer,
            d.manager,
            &isRevokedCall {
                creditHash: identity.credit_hash(),
            }
            .abi_encode(),
        )
        .await;
    assert!(bool::abi_decode(&revoked).unwrap(), "isRevoked[creditHash]");
    let err = anvil
        .call(
            d.buyer,
            d.manager,
            &use_credits(&identity, &signature, store_buy(&d, U256::from(MANA))),
        )
        .await
        .expect_err("a revoked credit is refused");
    assert_eq!(
        &err[..4],
        &selector("RevokedCredit(bytes32)"),
        "revert data 0x{}",
        hex::encode(&err)
    );
    // Re-sending the revocation (the crash-between-broadcast-and-write shape) is a no-op that succeeds.
    let tx = signer
        .send_direct_call(d.manager, revoke_calldata(identity.credit_hash()))
        .await
        .expect("a second revokeCredits is accepted");
    assert_eq!(
        signer.await_receipt(&tx).await.unwrap(),
        catalyst::shop::evm::signer::ReceiptOutcome::Confirmed
    );
}
