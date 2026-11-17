use super::{context, create_runtime, fetch_outcome, redirect_servers, run_scene};
use deno_core::JsRuntime;
use std::time::Duration;

thread_local! {
    // a scene's ops and timers run on its own thread, as in the sidecar
    static RT: tokio::runtime::Runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
}

/// Scripts run inside a tokio runtime, as in the scene thread: timers schedule on it.
fn eval(runtime: &mut JsRuntime, script: String) -> Result<String, String> {
    let handle = RT.with(|rt| rt.handle().clone());
    let _guard = handle.enter();
    let value = runtime
        .execute_script("<test>", script)
        .map_err(|e| e.to_string())?;
    deno_core::scope!(scope, runtime);
    Ok(deno_core::v8::Local::new(scope, value).to_rust_string_lossy(scope))
}

fn client_runtime() -> JsRuntime {
    let (runtime, _) = create_runtime(false, false, "test-runtime-surface", Default::default());
    {
        let state = runtime.op_state();
        let mut state = state.borrow_mut();
        state.put(context(false));
        state.put(dcl_types::SceneResourceCounters::default());
        state.put(Vec::<dcl_types::SceneLogMessage>::default());
        state.put(dcl_types::SceneElapsedTime(0.0));
    }
    runtime
}

fn run_event_loop(runtime: &mut JsRuntime) {
    RT.with(|rt| {
        rt.block_on(async {
            tokio::time::timeout(
                Duration::from_secs(5),
                runtime.run_event_loop(Default::default()),
            )
            .await
        })
    })
    .expect("event loop did not settle")
    .unwrap();
}

#[test]
fn intl_and_locale_apis_work() {
    run_scene(
        false,
        r#"
        const checks = {
            number: (1234.5).toLocaleString("en-US"),
            compare: "a".localeCompare("b"),
            date: new Intl.DateTimeFormat("en-US", { timeZone: "UTC" }).format(new Date(0)),
            german: (1234.5).toLocaleString("de-DE"),
        };
        if (checks.number !== "1,234.5" || checks.compare >= 0 || checks.date !== "1/1/1970"
            || checks.german !== "1.234,5") {
            throw new Error(JSON.stringify(checks));
        }
        "#,
    )
    .unwrap();
}

#[test]
fn console_log_reaches_the_scene_log() {
    let mut runtime = client_runtime();
    eval(
        &mut runtime,
        r#"console.log("hello", { a: { b: 1 } }, [1, 2]); console.error("bad")"#.to_owned(),
    )
    .unwrap();
    let state = runtime.op_state();
    let state = state.borrow();
    let logs = state.borrow::<Vec<dcl_types::SceneLogMessage>>();
    let messages = logs.iter().map(|l| l.message.as_str()).collect::<Vec<_>>();
    assert_eq!(
        messages,
        [
            r#"LOG "hello" Object {a: Object {b: 1}} Array(2) [1, 2]"#,
            r#"ERROR "bad""#
        ]
    );
}

/// serde_v8 results are null-prototype objects; scenes get ordinary objects at every depth.
#[test]
fn op_results_are_ordinary_objects_at_every_depth() {
    let (mut runtime, _) = create_runtime(false, true, "test-op-results", Default::default());
    runtime.op_state().borrow_mut().put(context(false));
    let outcome = eval(
        &mut runtime,
        r#"
        const result = Deno.core.ops.op_get_permission_types();
        const first = result[0];
        JSON.stringify({
            outer: String(first),
            concat: "" + first,
            keys: Object.keys(first).includes("toString"),
            proto: Object.getPrototypeOf(first) === Object.prototype,
        })
        "#
        .to_owned(),
    )
    .unwrap();
    let outcome: serde_json::Value = serde_json::from_str(&outcome).unwrap();
    assert_eq!(
        outcome,
        serde_json::json!({
            "outer": "[object Object]",
            "concat": "[object Object]",
            "keys": false,
            "proto": true,
        })
    );
}

#[test]
fn nested_values_in_op_results_are_ordinary_objects() {
    use serde::Serialize;
    #[derive(Serialize)]
    struct Inner {
        b: u32,
    }
    #[derive(Serialize)]
    struct Outer {
        data: Inner,
        list: Vec<Inner>,
        maybe: Option<Inner>,
    }
    let mut runtime = client_runtime();
    {
        deno_core::scope!(scope, runtime);
        let value = deno_core::ToV8::to_v8(
            super::super::plain::JsPlain(Outer {
                data: Inner { b: 1 },
                list: vec![Inner { b: 2 }],
                maybe: Some(Inner { b: 3 }),
            }),
            scope,
        )
        .unwrap();
        let global = scope.get_current_context().global(scope);
        let key = deno_core::v8::String::new(scope, "__value").unwrap();
        global.set(scope, key.into(), value);
    }
    let outcome = eval(
        &mut runtime,
        r#"[String(__value), String(__value.data), String(__value.list[0]), String(__value.maybe),
            Object.keys(__value.data).join()].join("|")"#
            .to_owned(),
    )
    .unwrap();
    assert_eq!(
        outcome,
        "[object Object]|[object Object]|[object Object]|[object Object]|b"
    );
}

#[test]
fn file_contents_reach_scenes_as_bytes() {
    let mut runtime = client_runtime();
    {
        deno_core::scope!(scope, runtime);
        let value = deno_core::ToV8::to_v8(
            super::super::plain::JsPlain(super::super::op_wrappers::runtime::ReadFile {
                content: vec![1, 2, 3].into(),
                hash: "h".to_owned(),
            }),
            scope,
        )
        .unwrap();
        let global = scope.get_current_context().global(scope);
        let key = deno_core::v8::String::new(scope, "__value").unwrap();
        global.set(scope, key.into(), value);
    }
    let outcome = eval(
        &mut runtime,
        r#"JSON.stringify([__value.content instanceof Uint8Array, Array.from(__value.content),
            __value.hash, String(__value)])"#
            .to_owned(),
    )
    .unwrap();
    assert_eq!(outcome, r#"[true,[1,2,3],"h","[object Object]"]"#);
}

/// `performance.now()` keeps the 2 ms resolution scenes have always read.
#[test]
fn performance_now_has_two_millisecond_granularity() {
    let mut runtime = client_runtime();
    let outcome = eval(
        &mut runtime,
        r#"(() => {
            const samples = [];
            const until = Date.now() + 30;
            while (Date.now() < until) {
                samples.push(performance.now());
            }
            return JSON.stringify([
                samples.every((t) => t % 2 === 0),
                samples[samples.length - 1] > samples[0],
            ]);
        })()"#
            .to_owned(),
    )
    .unwrap();
    assert_eq!(outcome, "[true,true]");
}

#[test]
fn internals_reads_do_not_reach_object_prototype() {
    let ports = redirect_servers();
    let outcome = fetch_outcome(
        false,
        format!(
            r#"let leaked = false;
            Object.defineProperty(Object.prototype, "__inspectorNetwork", {{
                get() {{ leaked = true; return undefined; }},
                configurable: true,
            }});
            fetch("http://127.0.0.1:{}/target")
            .then((r) => r.text(), (e) => `error ${{e}}`)
            .then((body) => globalThis.__outcome = `${{body}} ${{leaked}}`);"#,
            ports[1]
        ),
    );
    assert_eq!(outcome, "arrived false");
}

#[test]
fn response_blob_and_form_data_work() {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let body = "--b\r\nContent-Disposition: form-data; name=\"f\"; filename=\"a.txt\"\r\nContent-Type: text/plain\r\n\r\nhello\r\n--b--\r\n";
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            let _ = stream.read(&mut [0u8; 4096]);
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: multipart/form-data; boundary=b\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    let outcome = fetch_outcome(
        false,
        format!(
            r#"(async () => {{
                const blob = await (await fetch("http://127.0.0.1:{port}/")).blob();
                const form = await (await fetch("http://127.0.0.1:{port}/")).formData();
                const file = form.get("f");
                return JSON.stringify([blob.size > 0, file.name, await file.text()]);
            }})().then((s) => globalThis.__outcome = s, (e) => globalThis.__outcome = `error ${{e}}`);"#
        ),
    );
    assert_eq!(outcome, r#"[true,"a.txt","hello"]"#);
}

#[test]
fn broadcast_channels_do_not_cross_scenes() {
    use std::sync::mpsc;
    let (ready_sx, ready_rx) = mpsc::channel::<()>();
    let (sent_sx, sent_rx) = mpsc::channel::<()>();
    std::thread::scope(|s| {
        let listener = s.spawn(move || {
            let mut runtime = client_runtime();
            eval(
                &mut runtime,
                r#"globalThis.got = 0; new BroadcastChannel("x").onmessage = () => { globalThis.got++; }"#
                    .to_owned(),
            )
            .unwrap();
            ready_sx.send(()).unwrap();
            sent_rx.recv().unwrap();
            run_event_loop(&mut runtime);
            eval(&mut runtime, "String(got)".to_owned()).unwrap()
        });
        s.spawn(move || {
            ready_rx.recv().unwrap();
            let mut runtime = client_runtime();
            eval(
                &mut runtime,
                r#"new BroadcastChannel("x").postMessage("hi")"#.to_owned(),
            )
            .unwrap();
            run_event_loop(&mut runtime);
            let err = eval(
                &mut runtime,
                r#"Deno.core.ops.op_broadcast_send(0, "x", new Uint8Array(1))"#.to_owned(),
            )
            .unwrap_err();
            assert!(!err.is_empty());
            sent_sx.send(()).unwrap();
        })
        .join()
        .unwrap();
        assert_eq!(listener.join().unwrap(), "0");
    });
}

#[test]
fn the_vendored_fetch_ops_take_the_upstream_contract() {
    let mut runtime = client_runtime();
    let outcome = eval(
        &mut runtime,
        r#"
        const settled = Deno.core.ops.op_fetch_promise_is_settled(Promise.resolve(1));
        const pending = Deno.core.ops.op_fetch_promise_is_settled(new Promise(() => {}));
        const bytes = Deno.core.ops.op_utf8_to_byte_string("é");
        JSON.stringify([settled, pending, bytes.length])
        "#
        .to_owned(),
    )
    .unwrap();
    assert_eq!(outcome, "[true,false,2]");
}

#[test]
fn wasm_streaming_compilation_is_absent() {
    let mut runtime = client_runtime();
    let outcome = eval(
        &mut runtime,
        r#"typeof WebAssembly.compileStreaming + " " + typeof WebAssembly.instantiateStreaming"#
            .to_owned(),
    )
    .unwrap();
    assert_eq!(outcome, "undefined undefined");
}

/// A fresh, empty storage folder for one test.
fn storage_dir(name: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "dcl-deno-{name}-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Signs `address` in, as the renderer does by sending the player identity component.
fn sign_in(runtime: &mut JsRuntime, address: &str) {
    use dcl_component::{
        proto_components::sdk::components::PbPlayerIdentityData, DclReader, DclWriter,
        SceneComponentId, SceneEntityId,
    };
    let mut buf = Vec::new();
    DclWriter::new(&mut buf).write(&PbPlayerIdentityData {
        address: address.to_owned(),
        is_guest: false,
    });
    let mut store = dcl_types::interface::CrdtStore::default();
    store.force_update(
        SceneComponentId::PLAYER_IDENTITY_DATA,
        dcl_types::interface::CrdtType::LWW_ANY,
        SceneEntityId::PLAYER,
        Some(&mut DclReader::new(&buf)),
    );
    runtime
        .op_state()
        .borrow_mut()
        .put(dcl_js::RendererStore(store));
}

fn storage_runtime(dir: &std::path::Path, address: &str) -> JsRuntime {
    let mut runtime = client_runtime();
    runtime
        .op_state()
        .borrow_mut()
        .put(super::super::local_storage::LocalStorageDir(dir.to_owned()));
    sign_in(&mut runtime, address);
    runtime
}

const PLAYER_A: &str = "0xabcdef0000000000000000000000000000000001";
const PLAYER_B: &str = "0xabcdef0000000000000000000000000000000002";

#[test]
fn local_storage_round_trips_per_player() {
    let dir = storage_dir("round-trip");
    let mut runtime = storage_runtime(&dir, PLAYER_A);
    let outcome = eval(
        &mut runtime,
        r#"
        localStorage.setItem("a", "1");
        localStorage.b = "2";
        const first = [localStorage.length, localStorage.getItem("a"), localStorage.b,
            localStorage.key(0), Object.keys(localStorage).sort().join(), "a" in localStorage];
        localStorage.removeItem("a");
        const removed = [localStorage.length, localStorage.getItem("a")];
        JSON.stringify([first, removed])
        "#
        .to_owned(),
    )
    .unwrap();
    assert_eq!(outcome, r#"[[2,"1","2","a","a,b",true],[1,null]]"#);
    drop(runtime);

    // another player on the same machine sees none of it, and clearing leaves it in place
    let mut other = storage_runtime(&dir, PLAYER_B);
    let outcome = eval(
        &mut other,
        r#"
        const before = [localStorage.length, localStorage.getItem("b")];
        localStorage.setItem("c", "3");
        localStorage.clear();
        JSON.stringify([before, localStorage.length])
        "#
        .to_owned(),
    )
    .unwrap();
    assert_eq!(outcome, "[[0,null],0]");
    drop(other);

    let mut runtime = storage_runtime(&dir, PLAYER_A);
    let outcome = eval(
        &mut runtime,
        r#"JSON.stringify([localStorage.length, localStorage.getItem("b")])"#.to_owned(),
    )
    .unwrap();
    assert_eq!(outcome, r#"[1,"2"]"#);
    drop(runtime);
    let _ = std::fs::remove_dir_all(dir);
}

/// Data the previous runtime wrote stays readable: same file, table and key prefix.
#[test]
fn local_storage_reads_an_existing_database() {
    let dir = storage_dir("existing");
    {
        let conn = rusqlite::Connection::open(dir.join("local_storage")).unwrap();
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
            CREATE TABLE IF NOT EXISTS data (key VARCHAR UNIQUE, value VARCHAR);",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO data (key, value) VALUES (?, ?)",
            rusqlite::params![format!("{PLAYER_A}:saved"), "kept"],
        )
        .unwrap();
    }
    let mut runtime = storage_runtime(&dir, PLAYER_A);
    let outcome = eval(
        &mut runtime,
        r#"JSON.stringify([localStorage.getItem("saved"), Object.keys(localStorage)])"#.to_owned(),
    )
    .unwrap();
    assert_eq!(outcome, r#"["kept",["saved"]]"#);
    drop(runtime);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn local_storage_refuses_an_oversized_item() {
    let dir = storage_dir("quota");
    let mut runtime = storage_runtime(&dir, PLAYER_A);
    let err = eval(
        &mut runtime,
        r#"localStorage.setItem("big", "x".repeat(10 * 1024 * 1024))"#.to_owned(),
    )
    .unwrap_err();
    assert!(err.contains("Exceeded maximum storage size"), "{err}");
    drop(runtime);
    let _ = std::fs::remove_dir_all(dir);
}
