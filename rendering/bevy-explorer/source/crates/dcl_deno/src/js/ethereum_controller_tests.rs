use super::{context, create_runtime};
use common::rpc::{RpcCall, MAX_LEGACY_SIGN_MESSAGE_BYTES};
use serde_json::{json, Value};
use std::time::Duration;

// Load the deployed scene module and real Rust ops. The fixture is only the engine's
// identity/wallet response; signing must arrive through the scene response transport, never a provider connection.
fn legacy_signing_scene(wallet_response: Result<Value, String>) -> (Value, Vec<Value>) {
    legacy_signing_scene_with_title(wallet_response, "piñata 🦊")
}

fn legacy_signing_scene_with_title(
    wallet_response: Result<Value, String>,
    title: &str,
) -> (Value, Vec<Value>) {
    let (mut runtime, _) = create_runtime(
        false,
        false,
        "legacy-ethereum-contract-test",
        false,
        Default::default(),
    );
    let state = runtime.op_state();
    let (scene_sender, mut engine_calls) = dcl_types::scene_response_channel();
    {
        let mut state = state.borrow_mut();
        state.put(context(false));
        state.put(dcl_types::RpcCalls::default());
        state.put(dcl_types::SceneResponseSender::new(
            dcl_types::SceneId::DUMMY,
            scene_sender,
        ));
    }
    // Engine replies are synchronous and run outside the scene's Tokio runtime.
    let engine = std::thread::spawn(move || {
        let mut observed = Vec::new();
        while let Some((_, dcl_types::SceneResponse::ImmediateRpcCall(call))) =
            engine_calls.blocking_recv()
        {
            match call {
                RpcCall::GetUserAccount { response } => response.send(Some("0xabc".into())),
                RpcCall::SignLegacyMessage { message, response } => {
                    observed.push(json!({"legacyMessage": message}));
                    response.send(wallet_response.clone());
                }
                _ => panic!("unexpected scene RPC"),
            }
        }
        observed
    });
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        runtime.execute_script("<test>", r#"
            globalThis.result = null;
            (async () => {
                const api = require('~system/EthereumController');
                const account = await api.getUserAccount({});
                const signed = await api.signMessage({message: {title: 'piñata 🦊', nonce: '42'}});
                const parsed = await api.convertMessageToObject({message: signed.message});
                const payment = await api.requirePayment({toAddress: account.address, amount: 1, currency: 'ETH'});
                return {account, signed, parsed, payment};
            })().then(value => result = value, error => result = {error: String(error)});
        "#.replace("'piñata 🦊'", &serde_json::to_string(title).unwrap())).unwrap();
        tokio::time::timeout(Duration::from_secs(5), runtime.run_event_loop(Default::default())).await.unwrap().unwrap();
    });
    let value = runtime
        .execute_script("<test>", "JSON.stringify(result)")
        .unwrap();
    let result = {
        let scope = &mut runtime.handle_scope();
        serde_json::from_str(&deno_core::v8::Local::new(scope, value).to_rust_string_lossy(scope))
            .unwrap()
    };
    drop(runtime);
    drop(state);
    (result, engine.join().unwrap())
}

#[test]
fn legacy_ethereum_dispatches_structured_signing_through_the_engine() {
    let (result, observed) = legacy_signing_scene(Ok(json!("0xsigned")));
    let message = "# DCL Signed message\ntitle: piñata 🦊\nnonce: 42";
    let hex = "0x232044434c205369676e6564206d6573736167650a7469746c653a207069c3b161746120f09fa68a0a6e6f6e63653a203432";
    assert_eq!(
        observed,
        vec![json!({"legacyMessage": "title: piñata 🦊\nnonce: 42"})]
    );
    assert_eq!(
        result,
        json!({
            "account": {"address": "0xabc"},
            "signed": {"message": message, "hexEncodedMessage": hex, "signature": "0xsigned"},
            "parsed": {"dict": {"title": "piñata 🦊", "nonce": "42"}},
            "payment": {"jsonAnyResponse": "{}"}
        })
    );
}

#[test]
fn legacy_ethereum_preserves_engine_permission_denial() {
    let (result, observed) = legacy_signing_scene(Err("permission denied".into()));
    assert_eq!(observed.len(), 1);
    assert_eq!(result["signed"]["signature"], "");
    assert_eq!(observed[0]["legacyMessage"], "title: piñata 🦊\nnonce: 42");
}

#[test]
fn direct_personal_sign_op_rejects_unprefixed_bytes_without_dispatching() {
    let (mut runtime, _) = create_runtime(
        false,
        false,
        "legacy-signing-direct-op-test",
        false,
        Default::default(),
    );
    let (scene_sender, mut engine_calls) = dcl_types::scene_response_channel();
    {
        let state = runtime.op_state();
        let mut state = state.borrow_mut();
        state.put(context(false));
        state.put(dcl_types::RpcCalls::default());
        state.put(dcl_types::SceneResponseSender::new(
            dcl_types::SceneId::DUMMY,
            scene_sender,
        ));
    }
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        runtime
            .execute_script(
                "<test>",
                r#"
            globalThis.result = null;
            Deno.core.ops.op_send_async('personal_sign', '["0x617262697472617279", "0xabc"]')
                .then(value => result = {value}, error => result = {error: String(error)});
        "#,
            )
            .unwrap();
        tokio::time::timeout(
            Duration::from_secs(5),
            runtime.run_event_loop(Default::default()),
        )
        .await
        .unwrap()
        .unwrap();
    });
    let result = runtime.execute_script("<test>", "result.error").unwrap();
    let scope = &mut runtime.handle_scope();
    let error = deno_core::v8::Local::new(scope, result).to_rust_string_lossy(scope);
    assert!(error.contains("invalid method"), "{error}");
    assert!(engine_calls.try_recv().is_err());
}

#[test]
fn legacy_signing_caps_utf8_bytes_before_dispatch_and_preserves_the_rejection_envelope() {
    let title_bytes = MAX_LEGACY_SIGN_MESSAGE_BYTES - "title: \nnonce: 42".len();
    let ascii_title = "a".repeat(title_bytes);
    let unicode_title = "🦊".repeat(title_bytes / 4) + &"a".repeat(title_bytes % 4);
    for title in [ascii_title, unicode_title] {
        for extra in ["", "a"] {
            let title = title.clone() + extra;
            let fields = format!("title: {title}\nnonce: 42");
            let (result, observed) = legacy_signing_scene_with_title(Ok(json!("0xsigned")), &title);
            let message = format!("# DCL Signed message\n{fields}");
            assert_eq!(result["signed"]["message"], message);
            assert_eq!(
                result["signed"]["hexEncodedMessage"],
                format!(
                    "0x{}",
                    message
                        .as_bytes()
                        .iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect::<String>()
                )
            );
            if extra.is_empty() {
                assert_eq!(fields.len(), MAX_LEGACY_SIGN_MESSAGE_BYTES);
                assert_eq!(observed, vec![json!({"legacyMessage": fields})]);
                assert_eq!(result["signed"]["signature"], "0xsigned");
            } else {
                assert_eq!(fields.len(), MAX_LEGACY_SIGN_MESSAGE_BYTES + 1);
                assert!(
                    observed.is_empty(),
                    "oversized text must not reach the engine"
                );
                assert_eq!(result["signed"]["signature"], "");
            }
        }
    }
}
