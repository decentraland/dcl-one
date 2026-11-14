use super::{context, create_runtime};
use common::{
    rpc::RpcCall,
    structs::{GlobalCrdtStateUpdate, SceneJsFile},
};
use dcl_types::{RendererResponse, SceneResponse, SceneResponseReceiver, SceneResponseSender};
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};

fn runtime() -> (
    deno_core::JsRuntime,
    tokio::sync::mpsc::UnboundedSender<RendererResponse>,
    SceneResponseReceiver,
) {
    let (mut runtime, _) = create_runtime(
        false,
        false,
        "legacy-kernel-test",
        false,
        Default::default(),
    );
    let (renderer_tx, renderer_rx) = tokio::sync::mpsc::unbounded_channel();
    let (scene_tx, scene_rx) = dcl_types::scene_response_channel();
    let (_, global_rx) = tokio::sync::broadcast::channel::<GlobalCrdtStateUpdate>(1);
    dcl_js::init_state(
        &mut *runtime.op_state().borrow_mut(),
        Default::default(),
        context(false),
        "legacy-kernel-test".into(),
        SceneJsFile(Arc::new(String::new())),
        Default::default(),
        SceneResponseSender::new(dcl_types::SceneId::DUMMY, scene_tx),
        renderer_rx,
        global_rx,
        None,
        bevy::prelude::Vec3::ZERO,
        Default::default(),
    );
    (runtime, renderer_tx, scene_rx)
}

fn result(runtime: &mut deno_core::JsRuntime) -> Value {
    let value = runtime
        .execute_script("<test>", "JSON.stringify(globalThis.result)")
        .unwrap();
    let scope = &mut runtime.handle_scope();
    serde_json::from_str(&deno_core::v8::Local::new(scope, value).to_rust_string_lossy(scope))
        .unwrap()
}

#[test]
fn legacy_kernel_identity_permissions_and_logging_use_real_native_ops() {
    let (mut runtime, _, mut outbound) = runtime();
    let state = runtime.op_state();
    let engine = std::thread::spawn(move || {
        let mut calls = Vec::new();
        for _ in 0..3 {
            let (_, SceneResponse::ImmediateRpcCall(call)) = outbound.blocking_recv().unwrap()
            else {
                panic!("expected immediate RPC")
            };
            match call {
                RpcCall::EntityDefinition { urn, response } => {
                    calls.push(json!({"scene":urn}));
                    response.send(Some(common::rpc::EntityDefinitionResponse {
                        collection: [("main.js".to_owned(), "content-hash".to_owned())]
                            .into_iter()
                            .collect(),
                        metadata: Some("{\"scene\":{\"base\":\"1,2\"}}".into()),
                        base_url: "https://realm.example/content/contents/".into(),
                    }));
                }
                RpcCall::GetPermissions {
                    permissions,
                    response,
                } => {
                    calls.push(json!({"permissions":permissions}));
                    response.send(permissions.iter().map(|value| *value == 0).collect());
                }
                _ => panic!("unexpected legacy RPC"),
            }
        }
        calls
    });
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        runtime.execute_script("<test>", r#"
            globalThis.result = null;
            (async () => {
                const tools = require('~system/DevTools');
                await tools.event({type:'Runtime.consoleAPICalled', jsonPayload:'[{"args":[{"value":"legacy message"}]}]'});
                await tools.event({type:'Runtime.exceptionThrown', jsonPayload:'[{"exceptionDetails":{"text":"legacy error"}}]'});
                const parcel = require('~system/ParcelIdentity');
                const permissions = require('~system/Permissions');
                return {id:await parcel.getSceneId(),empty:await parcel.getIsEmpty(),parcel:await parcel.getParcel(),
                    one:await permissions.hasPermission({permission:0}),many:await permissions.hasManyPermissions({permissions:[2,0,5]})};
            })().then(value => result=value, error => result={error:String(error)});
        "#).unwrap();
        tokio::time::timeout(Duration::from_secs(5), runtime.run_event_loop(Default::default())).await.unwrap().unwrap();
    });
    assert_eq!(
        engine.join().unwrap(),
        vec![
            json!({"scene":"hash"}),
            json!({"permissions":[0]}),
            json!({"permissions":[2,0,5]})
        ]
    );
    let actual = result(&mut runtime);
    assert_eq!(actual["id"], json!({"sceneId":"hash"}));
    assert_eq!(actual["empty"], json!({"isEmpty":false}));
    assert_eq!(actual["parcel"]["cid"], "hash");
    assert_eq!(
        actual["parcel"]["land"]["baseUrl"],
        "https://realm.example/content/contents/"
    );
    assert_eq!(actual["parcel"]["land"]["baseUrlBundles"], "");
    assert_eq!(
        actual["parcel"]["land"]["mappingsResponse"]["contents"],
        json!([{"file":"main.js","hash":"content-hash"}])
    );
    assert_eq!(actual["one"], json!({"hasPermission":true}));
    assert_eq!(
        actual["many"],
        json!({"hasManyPermission":[false,true,false]})
    );
    let state = state.borrow();
    let logs = state.borrow::<Vec<dcl_types::SceneLogMessage>>();
    assert!(logs.iter().any(|log| log.message.contains("legacy message")
        && matches!(log.level, dcl_types::SceneLogLevel::Log)));
    assert!(logs.iter().any(|log| log.message.contains("legacy error")
        && matches!(log.level, dcl_types::SceneLogLevel::SceneError)));
}

#[test]
fn legacy_kernel_social_subscription_drains_events_once_and_closes_with_the_scene() {
    let (mut runtime, _, mut outbound) = runtime();
    let state = runtime.op_state();
    runtime.execute_script("<test>", "globalThis.social = require('~system/SocialController'); require('~system/SocialController')").unwrap();
    let (_, SceneResponse::ImmediateRpcCall(RpcCall::SubscribeLegacyAvatarEvents { sender })) =
        outbound.try_recv().unwrap()
    else {
        panic!("expected subscription")
    };
    assert!(
        outbound.try_recv().is_err(),
        "duplicate module load does not register twice"
    );
    let events = [
        json!({"event":"USER_VISIBLE","payload":"{\"type\":\"USER_VISIBLE\",\"userId\":\"0xabc\",\"visible\":true}"}),
        json!({"event":"USER_REMOVED","payload":"{\"type\":\"USER_REMOVED\",\"userId\":\"0xabc\"}"}),
    ];
    let queued = events.iter().map(Value::to_string).collect::<Vec<_>>();
    let engine = std::thread::spawn(move || {
        let mut queued = queued;
        let mut count = 0;
        for _ in 0..2 {
            let (_, SceneResponse::ImmediateRpcCall(call)) = outbound.blocking_recv().unwrap()
            else {
                panic!("expected immediate RPC")
            };
            let RpcCall::PullLegacyAvatarEvents { response } = call else {
                panic!("expected pull")
            };
            response.send(std::mem::take(&mut queued));
            count += 1;
        }
        count
    });
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        runtime.execute_script("<test>", "(async()=>{ result=[await social.pullAvatarEvents({}),await social.pullAvatarEvents({})] })()").unwrap();
        tokio::time::timeout(Duration::from_secs(5), runtime.run_event_loop(Default::default())).await.unwrap().unwrap();
    });
    assert_eq!(engine.join().unwrap(), 2);
    assert_eq!(
        result(&mut runtime),
        json!([{"events":events},{"events":[]}])
    );
    drop(runtime);
    drop(state);
    assert!(sender.is_closed());
}

#[test]
fn legacy_kernel_crdt_receive_drains_renderer_bytes_without_a_flush() {
    use dcl_component::{DclReader, SceneComponentId, SceneCrdtTimestamp, SceneEntityId};
    let (mut runtime, renderer, mut outbound) = runtime();
    let mut updates = dcl_types::interface::CrdtStore::default();
    let component = SceneComponentId(1234);
    let entity = SceneEntityId::ROOT;
    let timestamp = SceneCrdtTimestamp(7);
    let bytes = [8, 9];
    updates.try_update(
        component,
        dcl_types::interface::CrdtType::LWW_ANY,
        entity,
        timestamp,
        Some(&mut DclReader::new(&bytes)),
    );
    renderer
        .send(RendererResponse::Ok(
            updates,
            dcl_types::SceneCensus {
                born: Default::default(),
                died: Default::default(),
            },
            0.016,
        ))
        .unwrap();
    runtime.execute_script("<test>", "require('~system/EngineApi').crdtGetMessageFromRenderer({}).then(reply => result=reply.data.map(bytes=>Array.from(bytes)))").unwrap();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(runtime.run_event_loop(Default::default()))
        .unwrap();
    assert_eq!(
        result(&mut runtime),
        json!([dcl_types::crdt::put_component(
            &entity,
            &component,
            &timestamp,
            Some(&bytes)
        )])
    );
    assert!(outbound.try_recv().is_err(), "reading never flushes a tick");
    runtime.execute_script("<repeat>", "require('~system/EngineApi').crdtGetMessageFromRenderer({}).then(reply=>result=reply.data)").unwrap();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(runtime.run_event_loop(Default::default()))
        .unwrap();
    assert_eq!(result(&mut runtime), json!([]));
    assert!(outbound.try_recv().is_err());
    assert_eq!(
        runtime
            .op_state()
            .borrow()
            .borrow::<dcl_js::EngineTickDt>()
            .0,
        0.016
    );
}

#[test]
fn legacy_kernel_startup_and_update_await_concurrent_queries_while_signing_waits() {
    let (mut runtime, renderer, mut outbound) = runtime();
    let state = runtime.op_state();
    let engine = std::thread::spawn(move || {
        let mut signing = None;
        let mut reads = 0;
        let mut rounds = 0;
        let mut registrations = 0;
        let mut lifetime = None;
        while let Some((_, message)) = outbound.blocking_recv() {
            match message {
                SceneResponse::Ok(_, _, _, _, calls) => {
                    assert!(
                        calls.is_empty(),
                        "these queries must not wait for a CRDT flush"
                    );
                    renderer
                        .send(RendererResponse::Ok(
                            Default::default(),
                            dcl_types::SceneCensus {
                                born: Default::default(),
                                died: Default::default(),
                            },
                            0.016,
                        ))
                        .unwrap();
                }
                SceneResponse::ImmediateRpcCall(call) => {
                    match call {
                        RpcCall::SubscribeLegacyAvatarEvents { sender } => {
                            registrations += 1;
                            lifetime = Some(sender);
                            continue;
                        }
                        RpcCall::SendAsync { response, .. } => {
                            assert!(signing.is_none());
                            signing = Some(response);
                            continue;
                        }
                        RpcCall::GetPermissions {
                            permissions,
                            response,
                        } => response.send(vec![true; permissions.len()]),
                        RpcCall::GetUserAccount { response } => response.send(Some("0xabc".into())),
                        RpcCall::EntityDefinition { response, .. } => {
                            response.send(Some(common::rpc::EntityDefinitionResponse {
                                collection: Default::default(),
                                metadata: Some("{}".into()),
                                base_url: "https://content.example/contents/".into(),
                            }))
                        }
                        RpcCall::PullLegacyAvatarEvents { response } => {
                            assert!(lifetime.is_some());
                            response.send(Vec::new());
                        }
                        _ => panic!("unexpected startup request"),
                    }
                    assert!(
                        signing.is_some(),
                        "query completes while the signing request is still pending"
                    );
                    reads += 1;
                    if reads == 5 {
                        signing.take().unwrap().send(Ok(json!("approved")));
                        reads = 0;
                        rounds += 1;
                    }
                }
                _ => panic!("unexpected scene response"),
            }
        }
        assert!(lifetime.unwrap().is_closed());
        (rounds, registrations)
    });
    let script = runtime
        .execute_script(
            "<startup>",
            r#"
        globalThis.result = {};
        async function queries() {
            const social = require('~system/SocialController');
            const eth = require('~system/EthereumController');
            const pending = eth.sendAsync({id:1,method:'eth_signTypedData_v4',jsonParams:'[]'});
            const values = await Promise.all([
                require('~system/Permissions').hasManyPermissions({permissions:[0,4]}),
                require('~system/ParcelIdentity').getParcel({}),
                eth.getUserAccount({}),
                social.pullAvatarEvents({}), social.pullAvatarEvents({})
            ]);
            return {values, signature:await pending};
        }
        ({
            async onStart() {
                await require('~system/EngineApi').crdtGetState({});
                result.start = await queries();
            },
            async onUpdate() {
                result.update = await queries();
                await require('~system/EngineApi').crdtSendToRenderer({data:new Uint8Array()});
            }
        })
    "#,
        )
        .unwrap();
    dcl_js::engine::crdt_send_to_renderer(state.clone(), &[]);
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        tokio::time::timeout(
            Duration::from_secs(5),
            crate::js::run_script(&mut runtime, &script, "onStart", |_| Vec::new()),
        )
        .await
        .unwrap()
        .unwrap();
        dcl_js::end_start(&mut *state.borrow_mut());
        dcl_js::begin_tick(&mut *state.borrow_mut());
        tokio::time::timeout(
            Duration::from_secs(5),
            crate::js::run_script(&mut runtime, &script, "onUpdate", |_| Vec::new()),
        )
        .await
        .unwrap()
        .unwrap();
    });
    let actual = result(&mut runtime);
    for phase in ["start", "update"] {
        assert_eq!(
            actual[phase]["values"][0],
            json!({"hasManyPermission":[true,true]})
        );
        assert_eq!(actual[phase]["values"][2], json!({"address":"0xabc"}));
        assert_eq!(actual[phase]["values"][3], json!({"events":[]}));
        assert_eq!(actual[phase]["values"][4], json!({"events":[]}));
        assert!(actual[phase]["signature"].is_object());
    }
    drop(script);
    drop(runtime);
    drop(state);
    assert_eq!(engine.join().unwrap(), (2, 1));
}

#[test]
fn legacy_kernel_full_response_queue_rejects_without_panicking() {
    let (mut runtime, _, _outbound) = runtime();
    let state = runtime.op_state();
    for _ in 0..1000 {
        state
            .borrow()
            .borrow::<SceneResponseSender>()
            .try_send(SceneResponse::WaitingForInspector)
            .unwrap();
    }
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        runtime.execute_script("<full-queue>", "require('~system/EthereumController').getUserAccount({}).then(()=>result='unexpected', error=>result=String(error))").unwrap();
        tokio::time::timeout(Duration::from_secs(5), runtime.run_event_loop(Default::default())).await.unwrap().unwrap();
    });
    assert!(result(&mut runtime)
        .as_str()
        .unwrap()
        .contains("scene RPC queue unavailable"));
}

#[test]
fn native_startup_reads_more_than_one_thousand_files_without_a_renderer_flush() {
    let (mut runtime, _renderer, mut outbound) = runtime();
    let engine = std::thread::spawn(move || {
        for index in 0..1001 {
            let (_, SceneResponse::ImmediateRpcCall(RpcCall::ReadFile { filename, response })) =
                outbound.blocking_recv().unwrap()
            else {
                panic!("expected startup file read")
            };
            assert_eq!(filename, format!("asset-{index}"));
            response.send(Ok(common::rpc::ReadFileResponse {
                content: vec![7],
                hash: index.to_string(),
            }));
        }
    });
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        runtime.execute_script("<startup-files>", "(async()=>{ const api=require('~system/Runtime'); for(let i=0;i<1001;i++) { const file=await api.readFile({fileName:'asset-'+i}); if(file.content[0]!==7) throw Error('file payload'); } result=1001; })()").unwrap();
        tokio::time::timeout(Duration::from_secs(10), runtime.run_event_loop(Default::default())).await.unwrap().unwrap();
    });
    assert_eq!(result(&mut runtime), json!(1001));
    engine.join().unwrap();
}

#[test]
fn native_legacy_reads_interleave_with_each_renderer_tick_without_extra_flushes() {
    let (mut runtime, renderer, mut outbound) = runtime();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime
        .execute_script("<engine>", "globalThis.engine=require('~system/EngineApi')")
        .unwrap();
    for frame in 0..20 {
        dcl_js::begin_tick(&mut *runtime.op_state().borrow_mut());
        // A read with no received data resolves even before the next renderer update exists.
        rt.block_on(async {
            runtime
                .execute_script(
                    "<before-tick>",
                    "engine.crdtGetMessageFromRenderer({}).then(reply=>result=reply.data)",
                )
                .unwrap();
            tokio::time::timeout(
                Duration::from_secs(1),
                runtime.run_event_loop(Default::default()),
            )
            .await
            .unwrap()
            .unwrap();
        });
        assert_eq!(result(&mut runtime), json!([]));
        assert!(outbound.try_recv().is_err());
        let mut updates = dcl_types::interface::CrdtStore::default();
        let payload = [frame];
        updates.try_update(
            dcl_component::SceneComponentId(1234),
            dcl_types::interface::CrdtType::LWW_ANY,
            dcl_component::SceneEntityId::ROOT,
            dcl_component::SceneCrdtTimestamp(frame as u32 + 1),
            Some(&mut dcl_component::DclReader::new(&payload)),
        );
        renderer
            .send(RendererResponse::Ok(
                updates,
                dcl_types::SceneCensus {
                    born: Default::default(),
                    died: Default::default(),
                },
                0.016,
            ))
            .unwrap();
        rt.block_on(async {
            runtime.execute_script("<tick>", "(async()=>{ const tick=await engine.crdtSendToRenderer({data:new Uint8Array()}); const a=await engine.crdtGetMessageFromRenderer({}); const b=await engine.crdtGetMessageFromRenderer({}); result={tick:tick.data.map(bytes=>Array.from(bytes)),a:a.data,b:b.data}; })()").unwrap();
            tokio::time::timeout(Duration::from_secs(1), runtime.run_event_loop(Default::default())).await.unwrap().unwrap();
        });
        let actual = result(&mut runtime);
        assert_eq!(
            actual["tick"][0].as_array().unwrap().last().unwrap(),
            &json!(frame)
        );
        assert_eq!(actual["a"], json!([]));
        assert_eq!(actual["b"], json!([]));
        assert!(matches!(
            outbound.try_recv(),
            Ok((_, SceneResponse::Ok(..)))
        ));
        assert!(
            outbound.try_recv().is_err(),
            "exactly one renderer credit per frame"
        );
    }
}
