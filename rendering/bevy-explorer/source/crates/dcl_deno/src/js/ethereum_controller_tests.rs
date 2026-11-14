use super::{context, create_runtime};
use common::rpc::RpcCall;
use serde_json::{json, Value};
use std::time::Duration;

// Load the deployed scene module and real Rust ops. The fixture is only the engine's
// identity/wallet response; signing must arrive through the scene response transport, never a provider connection.
fn legacy_signing_scene(wallet_response: Result<Value, String>) -> (Value, Vec<Value>) {
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
                RpcCall::SendAsync { body, response } => {
                    observed.push(json!({"method": body.method, "params": body.params}));
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
        "#).unwrap();
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
fn legacy_ethereum_dispatches_personal_sign_through_the_engine() {
    let (result, observed) = legacy_signing_scene(Ok(json!("0xsigned")));
    let message = "# DCL Signed message\ntitle: piñata 🦊\nnonce: 42";
    let hex = "0x232044434c205369676e6564206d6573736167650a7469746c653a207069c3b161746120f09fa68a0a6e6f6e63653a203432";
    assert_eq!(
        observed,
        vec![json!({"method": "personal_sign", "params": [hex, "0xabc"]})]
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
    assert_eq!(observed[0]["method"], "personal_sign");
}
