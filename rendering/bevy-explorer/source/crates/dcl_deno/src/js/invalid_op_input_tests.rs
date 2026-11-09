use super::{context, create_runtime, run_scene};
use crate::js::{OpOverrideProblems, DISABLED_FEATURE_CALLS};
use std::{sync::Arc, time::Duration};

use common::structs::SceneJsFile;
use dcl_types::{interface::crdt_context::CrdtContext, SceneId};
use deno_core::{ascii_str, JsRuntime};

fn assert_refused(script: &str, needle: &str) {
    let err = run_scene(false, script).expect_err(script);
    assert!(err.contains(needle), "{script}: unexpected error: {err}");
}

#[test]
fn deno_net_ops_are_refused() {
    for script in [
        r#"Deno.core.ops.op_net_listen_udp({ hostname: "127.0.0.1", port: 9998, transport: "udp" }, false, false)"#,
        r#"Deno.core.ops.op_net_listen_tcp({ hostname: "127.0.0.1", port: 9997, transport: "tcp" }, true, false)"#,
        r#"Deno.core.ops.op_net_listen_tls({ hostname: "127.0.0.1", port: 9996 }, { reusePort: true }, null)"#,
        r#"Deno.core.ops.op_net_listen_unixpacket("/nonexistent/scene.sock")"#,
        r#"Deno.core.ops.op_tls_key_null()"#,
        r#"Deno.core.ops.op_tls_cert_resolver_create()"#,
        r#"Deno.core.ops.op_tls_cert_resolver_resolve_error(null, "sni", "error")"#,
    ] {
        assert_refused(script, "raw socket access");
    }
}

#[test]
fn op_panic_is_refused() {
    assert_refused(r#"Deno.core.ops.op_panic("x")"#, "not available");
}

#[test]
fn call_site_needs_room_for_line_and_column() {
    assert_refused(
        "Deno.core.ops.op_current_user_call_site(new Uint8Array(0))",
        "8 bytes",
    );
    run_scene(
        false,
        r#"
        const buf = new Uint8Array(8);
        Deno.core.ops.op_current_user_call_site(buf);
        if (new Uint32Array(buf.buffer)[0] === 0) { throw new Error("no line written"); }
        "#,
    )
    .unwrap();
}

#[test]
fn websocket_ops_reject_an_unknown_rid() {
    for script in [
        "Deno.core.ops.op_ws_get_buffered_amount(9999)",
        r#"Deno.core.ops.op_ws_send_text(9999, "x")"#,
        "Deno.core.ops.op_ws_send_binary(9999, new Uint8Array(1))",
        "Deno.core.ops.op_ws_send_binary_ab(9999, new ArrayBuffer(1))",
    ] {
        assert_refused(script, "Bad resource ID");
    }
}

#[test]
fn websocket_create_rejects_urls_the_handshake_cannot_dial() {
    for url in ["http://example.org/", "wss:", "ws://example.org/"] {
        let script = format!(
            r#"Deno.core.ops.op_ws_create("WebSocket.abort", "{url}", "", null, null)
                .then(() => {{ throw new Error("connected"); }}, (e) => {{ globalThis.__ws = String(e); }})"#
        );
        let (mut runtime, _) =
            create_runtime(false, false, "test-ws-url", false, Default::default());
        runtime.op_state().borrow_mut().put(context(false));
        runtime.execute_script("<test>", script).unwrap();
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(runtime.run_event_loop(Default::default()))
            .unwrap();
        // refused before the permission request, which nothing here would answer
        let err = eval_string(&mut runtime, "String(globalThis.__ws)");
        assert!(err.starts_with("Error"), "{url}: unexpected outcome: {err}");
    }
}

#[test]
fn stream_sink_ops_reject_invalid_handles() {
    assert_refused(
        "Deno.core.ops.op_readable_stream_resource_close(null)",
        "expected i32",
    );
    for script in [
        "Deno.core.ops.op_readable_stream_resource_close(9999)",
        "Deno.core.ops.op_readable_stream_resource_write_sync(9999, new Uint8Array(1))",
        r#"Deno.core.ops.op_readable_stream_resource_write_error(9999, "x")"#,
        "Deno.core.ops.op_readable_stream_resource_get_sink(9999)",
        r#"
        const rid = Deno.core.ops.op_readable_stream_resource_allocate();
        const sink = Deno.core.ops.op_readable_stream_resource_get_sink(rid);
        Deno.core.ops.op_readable_stream_resource_close(sink);
        Deno.core.ops.op_readable_stream_resource_close(sink);
        "#,
        r#"
        const rid = Deno.core.ops.op_readable_stream_resource_allocate();
        Deno.core.ops.op_readable_stream_resource_close(rid);
        "#,
    ] {
        assert_refused(script, "Bad resource ID");
    }
}

#[test]
fn op_arguments_that_run_js_while_read_are_refused() {
    for (script, needle) in [
        (
            "Deno.core.ops.op_log_test_plan(new Proxy({}, { get() { throw new Error('x'); } }))",
            "cannot be passed",
        ),
        (
            "Deno.core.ops.op_log_test_plan({ get tests() { throw new Error('x'); } })",
            "reading the argument threw",
        ),
        (
            "const a = [['k', 'v']]; Object.defineProperty(a[0], 1, { get() { throw new Error('x'); } }); Deno.core.ops.op_url_stringify_search_params(a)",
            "reading the argument threw",
        ),
        (
            "Deno.core.ops.op_log_test_plan({ tests: [new String('x')] })",
            "cannot be passed",
        ),
    ] {
        assert_refused(script, needle);
    }
}

#[test]
fn web_platform_ops_validate_their_buffers_and_state() {
    for (script, needle) in [
        ("Deno.core.ops.op_url_get_serialization()", "no URL serialization is pending"),
        (
            r#"Deno.core.ops.op_encoding_encode_into("x", new Uint8Array(1), new Uint32Array(1))"#,
            "two values",
        ),
        ("Deno.core.ops.op_leak_tracing_get(9, 0)", "not available"),
        (r#"Deno.core.ops.op_blob_remove_part("x")"#, "Blob part not found"),
    ] {
        assert_refused(script, needle);
    }
    run_scene(
        false,
        r#"
        if (Deno.core.ops.op_url_parse("http://a/b/../c", new Uint32Array(1)) !== 2) {
            throw new Error("a short buffer must fail the parse");
        }
        const out = new Uint8Array(8);
        const { read, written } = new TextEncoder().encodeInto("h\u00e9llo", out);
        if (read !== 5 || written !== 6) { throw new Error(`encodeInto: ${read} ${written}`); }
        "#,
    )
    .unwrap();
}

#[test]
fn performance_marks_and_measures_work() {
    run_scene(
        false,
        r#"
        performance.mark("start");
        const end = performance.mark("end", { detail: { items: [1, "two"] } });
        if (end.detail.items[1] !== "two") { throw new Error("mark detail was not cloned"); }
        const measure = performance.measure("span", "start", "end");
        if (!(measure.duration >= 0)) { throw new Error(`measure: ${measure.duration}`); }
        "#,
    )
    .unwrap();
}

#[test]
fn structured_clone_refuses_what_it_cannot_copy() {
    for (script, needle) in [
        (
            "Deno.core.ops.op_serialize(1, [], undefined, false, undefined)",
            "host objects or transfers",
        ),
        (
            "Deno.core.ops.op_serialize(1, undefined, [new ArrayBuffer(1)], false, undefined)",
            "host objects or transfers",
        ),
        (
            "Deno.core.ops.op_deserialize(new Uint8Array([0xff, 0x0f, 0x49, 0x02]), [], undefined, false)",
            "host objects or transfers",
        ),
        (
            "Deno.core.ops.op_serialize(new SharedArrayBuffer(1))",
            "could not be cloned",
        ),
        (
            "Deno.core.ops.op_serialize({ get x() { throw new Error('getter ran'); } })",
            "getter ran",
        ),
        (
            "Deno.core.ops.op_serialize(new Proxy({}, {}))",
            "could not be cloned",
        ),
        // a host object, then a shared buffer, by their wire tags
        ("Deno.core.ops.op_deserialize(new Uint8Array([0xff, 0x0f, 0x5c, 0x00]))", "deserializ"),
        ("Deno.core.ops.op_deserialize(new Uint8Array([0xff, 0x0f, 0x75, 0x00]))", "deserializ"),
        ("Deno.core.ops.op_deserialize(new Uint8Array([0x01, 0x02]))", "could not deserialize"),
    ] {
        assert_refused(script, needle);
    }
    run_scene(
        false,
        r#"
        const value = { a: [1, "b", null], c: new Uint8Array([7]), d: new Map([[1, 2]]) };
        const copy = Deno.core.deserialize(Deno.core.serialize(value));
        if (copy.a[1] !== "b" || copy.c[0] !== 7 || copy.d.get(1) !== 2) {
            throw new Error("the clone lost data");
        }
        "#,
    )
    .unwrap();
}

#[test]
fn eval_context_reports_errors_and_refuses_host_options() {
    run_scene(
        false,
        r#"
        const [value] = Deno.core.evalContext("6 * 7", "file:///x");
        if (value !== 42) { throw new Error(`result: ${value}`); }
        const [, compile] = Deno.core.evalContext("(", "file:///x");
        if (!compile || !compile.isCompileError) { throw new Error("no compile error"); }
        const [, thrown] = Deno.core.evalContext("throw new TypeError('t')", "file:///x");
        if (!thrown || thrown.isCompileError || !thrown.isNativeError || thrown.thrown.message !== "t") {
            throw new Error("runtime error not reported");
        }
        "#,
    )
    .unwrap();
    assert_refused(
        "const a = [1]; Object.defineProperty(a, 0, { get() { throw new Error('x'); } }); Deno.core.ops.op_eval_context('1', 'file:///x', a)",
        "host-defined options",
    );
    assert_refused(
        "Deno.core.ops.op_message_port_post_message(0, { get data() { throw new Error('x'); } })",
        "MessagePort",
    );
}

#[test]
fn struct_arguments_ignore_inherited_getters() {
    for (field, op_call) in [
        ("start", "Deno.core.ops.op_blob_slice_part('x', [])"),
        ("start", "Deno.core.ops.op_blob_slice_part('x', new Uint8Array(4))"),
        ("tests", "Deno.core.ops.op_log_test_plan(new ArrayBuffer(4))"),
        ("tests", "Deno.core.ops.op_log_test_plan([[]])"),
    ] {
        let script = format!(
            "Object.defineProperty(Object.prototype, '{field}', {{ get() {{ throw new Error('x'); }} }}); {op_call}"
        );
        run_scene(false, &script).expect_err(&script);
    }
}

#[test]
fn shared_subtrees_cannot_multiply_a_struct_argument() {
    assert_refused(
        "let v = []; for (let i = 0; i < 40; i++) v = [v, v]; Deno.core.ops.op_url_stringify_search_params(v)",
        "too large",
    );
}

#[test]
fn a_huge_stream_length_is_not_preallocated() {
    let (mut runtime, _) = create_runtime(false, false, "test-size-hint", false, Default::default());
    runtime.op_state().borrow_mut().put(context(false));
    runtime
        .execute_script(
            "<test>",
            r#"
            const ops = Deno.core.ops;
            const rid = ops.op_readable_stream_resource_allocate_sized(2 ** 50);
            const sink = ops.op_readable_stream_resource_get_sink(rid);
            ops.op_readable_stream_resource_write_sync(sink, new Uint8Array([1, 2, 3]));
            ops.op_readable_stream_resource_close(sink);
            ops.op_read_all(rid).then((bytes) => { globalThis.__read = String(bytes.length); }, (e) => { globalThis.__read = String(e); });
            "#,
        )
        .unwrap();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(runtime.run_event_loop(Default::default()))
        .unwrap();
    assert_eq!(eval_string(&mut runtime, "String(globalThis.__read)"), "3");
}

#[test]
fn every_override_replaces_an_op_of_the_same_shape() {
    for super_user in [false, true] {
        let (mut runtime, _) = create_runtime(false, super_user, "test-overrides", false, Default::default());
        let state = runtime.op_state();
        let state = state.borrow();
        let problems = &state.borrow::<OpOverrideProblems>().0;
        assert!(problems.is_empty(), "{problems:#?}");
    }
}

fn eval_string(runtime: &mut JsRuntime, script: &'static str) -> String {
    let value = runtime.execute_script("<test>", script).unwrap();
    let scope = &mut runtime.handle_scope();
    deno_core::v8::Local::new(scope, value).to_rust_string_lossy(scope)
}

/// A fetch with a streamed body reaches the server whole, through every producer path:
/// sync writes, the backpressured async write and close.
#[test]
fn streamed_upload_reaches_the_server() {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream);
        let mut length = 0;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if line == "\r\n" {
                break;
            }
            if let Some((name, value)) = line.split_once(':') {
                if name.eq_ignore_ascii_case("content-length") {
                    length = value.trim().parse().unwrap();
                }
            }
        }
        let mut body = vec![0u8; length];
        reader.read_exact(&mut body).unwrap();
        let reply = format!("{} {}", body.len(), body.iter().map(|b| *b as u64).sum::<u64>());
        write!(
            reader.get_mut(),
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}",
            reply.len()
        )
        .unwrap();
        body
    });

    let (mut runtime, _) = create_runtime(false, false, "test-upload", true, Default::default());
    let state = runtime.op_state();
    {
        let mut state = state.borrow_mut();
        state.put(CrdtContext::new(
            SceneId(bevy::prelude::Entity::from_raw(0)),
            "hash".into(),
            "title".into(),
            false,
            true,
            false,
        ));
        state.put(dcl_types::SceneResourceCounters::default());
        state.put(dcl_types::RpcCalls::default());
    }
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let script = runtime
            .load_side_es_module_from_code(
                &deno_core::ModuleSpecifier::parse("ext:test/streamed_upload.js").unwrap(),
                format!(
                    r#"
                import {{ ReadableStream }} from "ext:deno_web/06_streams.js";
                const chunks = [new Uint8Array(10).fill(1), new Uint8Array(200 * 1024).fill(2), new Uint8Array(5).fill(3)];
                const body = new ReadableStream({{
                    pull(controller) {{
                        const chunk = chunks.shift();
                        if (chunk) {{ controller.enqueue(chunk); }} else {{ controller.close(); }}
                    }},
                }});
                globalThis.outcome = "pending";
                fetch("http://127.0.0.1:{port}/upload", {{ method: "POST", body }})
                    .then((r) => r.text(), (e) => `error ${{e}}`)
                    .then((s) => {{ globalThis.outcome = s; }});
            "#
                ),
            )
            .await
            .unwrap();
        let evaluated = runtime.mod_evaluate(script);
        let grant = async {
            loop {
                tokio::time::sleep(Duration::from_millis(5)).await;
                let calls = std::mem::take(
                    &mut **state.borrow_mut().borrow_mut::<dcl_types::RpcCalls>(),
                );
                for call in calls {
                    if let common::rpc::RpcCall::RequestGenericPermission { response, .. } = call
                    {
                        std::thread::spawn(move || response.send(true));
                    }
                }
            }
        };
        tokio::select! {
            r = tokio::time::timeout(Duration::from_secs(10), runtime.run_event_loop(Default::default())) => {
                r.expect("upload did not settle").unwrap()
            }
            _ = grant => unreachable!(),
        }
        evaluated.await.unwrap();
    });
    let expected_sum = 10 + 200 * 1024 * 2 + 5 * 3;
    assert_eq!(
        eval_string(&mut runtime, "String(globalThis.outcome)"),
        format!("{} {expected_sum}", 10 + 200 * 1024 + 5)
    );
    let body = server.join().unwrap();
    assert_eq!(&body[..10], &[1; 10]);
    assert_eq!(&body[body.len() - 5..], &[3; 5]);
    assert!(
        !state
            .borrow()
            .resource_table
            .names()
            .any(|(_, name)| name.starts_with("readableStream")),
        "the upload left a stream resource behind"
    );
}

/// Ops the sweep does not call, with the reason.
const SWEEP_SKIPPED: &[(&str, &str)] = &[(
    "op_dispatch_exception",
    "terminates the calling isolate by design, which ends that scene and stops the sweep",
)];

/// Calls every op registered in a client-mode scene runtime with junk arguments. Each call
/// must return, throw or settle a promise: a panic or exit in any op takes the shared
/// sidecar, and every scene in it, down. Set `DCL_OP_SWEEP_TRACE=1` to log each call
/// before it is made, which names the op when one aborts the test process.
fn sweep_client_ops(super_user: bool) {
    let kill_flag = dcl_js::KillFlag::default();
    let (mut runtime, _) = create_runtime(false, super_user, "op-sweep", false, kill_flag.clone());
    let (scene_sx, _scene_rx) = dcl_types::scene_response_channel();
    // dropped so ops that wait on the renderer see a closed channel instead of waiting
    let (_, renderer_rx) = tokio::sync::mpsc::unbounded_channel();
    let (_global_sx, global_rx) = tokio::sync::broadcast::channel(1);
    let (system_sx, _system_rx) = tokio::sync::mpsc::unbounded_channel();
    let skipped = SWEEP_SKIPPED
        .iter()
        .map(|(name, _)| format!("{name:?}"))
        .collect::<Vec<_>>()
        .join(",");
    let trace = std::env::var("DCL_OP_SWEEP_TRACE").is_ok_and(|v| v == "1");
    let sweep = format!(
        r#"
        const skipped = new Set([{skipped}]);
        const trace = {trace};
        const nested = (depth) => {{ let v = []; for (let i = 0; i < depth; i++) v = [v]; return v; }};
        const cyclic = () => {{ const o = {{}}; o.self = o; o.list = [o]; return o; }};
        const trap = () => new Proxy({{}}, {{ get() {{ throw new Error("trap"); }}, ownKeys() {{ throw new Error("trap"); }} }});
        const throwingArray = () => {{ const a = [1, "x"]; Object.defineProperty(a, 0, {{ get() {{ throw new Error("trap"); }} }}); return a; }};
        const throwingStringObject = () => Object.assign(new String("8ab7e7a2-5a3b-4f0c-9d1e-0a1b2c3d4e5f"), {{ [Symbol.toPrimitive]() {{ throw new Error("trap"); }} }});
        const throwingString = () => ({{ toString() {{ throw new Error("trap"); }}, valueOf() {{ throw new Error("trap"); }}, [Symbol.toPrimitive]() {{ throw new Error("trap"); }} }});
        const shapes = () => [
            [],
            [null],
            [undefined, undefined, undefined, undefined],
            [0, 0, 0, 0, 0, 0, 0, 0],
            [1, 2, 3, 4],
            [9999, 9999, 9999, 9999],
            [-1, 2 ** 53, NaN, Infinity, -0],
            [4294967295, -2147483649, 1.5],
            ["", "", "", ""],
            ["\uD800", "\0", "a\uDFFFb", "%"],
            ["x", "wss://127.0.0.1:1/", "ws://127.0.0.1:1/", "/nonexistent"],
            [new Uint8Array(0), new Uint8Array(0), new Uint8Array(0)],
            [new Uint8Array(16), new ArrayBuffer(8), new Uint8Array(1)],
            [new Float64Array(2), new DataView(new ArrayBuffer(4))],
            [{{}}, [], {{ a: 1 }}],
            [true, false, true],
            [Symbol("s"), Symbol("s")],
            [() => {{}}, async () => {{}}],
            [0n, 2n ** 70n],
            [[1, 2], [3, [4]]],
            [nested(10000)],
            [cyclic(), cyclic()],
            [trap(), trap()],
            [[trap(), trap()], [trap()], {{ a: trap() }}],
            [throwingArray(), throwingArray(), throwingArray()],
            [{{ a: Symbol("s") }}, [Symbol("s")], [[Symbol("s")]]],
            [throwingString(), throwingString(), throwingString()],
            [[throwingString()], {{ a: throwingString() }}],
            ["http://127.0.0.1:1/", new Uint32Array(1)],
            ["http://127.0.0.1:1/b/../c", "http://127.0.0.1:2/", new Uint32Array(1)],
            ["http://127.0.0.1:1/", 1, "b", new Uint32Array(1)],
            ["x", new Uint8Array(1), new Uint32Array(1)],
            ["x", new Uint8Array(1), new Uint32Array(0)],
            [new Uint32Array(0), new Uint32Array(0), new Uint32Array(0), new Uint32Array(0)],
            [throwingStringObject(), throwingStringObject(), throwingStringObject()],
            [[throwingStringObject()], [[throwingStringObject(), throwingStringObject()]]],
            // a valid leading scalar, so the trap is read only after the first argument converts
            [0, trap(), trap(), trap()],
            [0, 0, trap(), trap()],
            [0, {{ get data() {{ throw new Error("trap"); }} }}],
            [0, throwingArray(), throwingArray()],
            [0, [trap()], {{ a: trap() }}],
            [0, "x", throwingString(), throwingString()],
            ["1", "file:///x", throwingArray()],
            ["x", trap(), trap()],
            ["x", "x", trap(), trap()],
            ["x", 0, throwingArray(), throwingArray()],
            [new Uint8Array(8), trap(), trap()],
            [0, new Uint8Array(8), trap(), trap()],
        ];
        const ops = Deno.core.ops;
        const names = Object.keys(ops).sort();
        globalThis.__sweep = {{ ops: 0, shapes: shapes().length, calls: 0, returned: 0, threw: 0, resolved: 0, rejected: 0, done: false }};
        const result = globalThis.__sweep;
        for (const name of names) {{
            if (skipped.has(name) || typeof ops[name] !== "function") continue;
            result.ops++;
            shapes().forEach((args, shape) => {{
                if (trace) ops.op_print(`op sweep: ${{name}} shape ${{shape}}\n`, true);
                result.calls++;
                let value;
                try {{
                    value = ops[name](...args);
                }} catch (e) {{
                    result.threw++;
                    return;
                }}
                result.returned++;
                if (value instanceof Promise) {{
                    value.then(() => result.resolved++, () => result.rejected++);
                }}
            }});
        }}
        result.done = true;
        "#
    );
    let disabled_feature_calls = DISABLED_FEATURE_CALLS.load(std::sync::atomic::Ordering::Relaxed);
    let state = runtime.op_state();
    dcl_js::init_state(
        &mut *state.borrow_mut(),
        Default::default(),
        context(false),
        "op-sweep".to_owned(),
        SceneJsFile(Arc::new(sweep)),
        Default::default(),
        dcl_types::SceneResponseSender::new(SceneId::DUMMY, scene_sx),
        renderer_rx,
        global_rx,
        super_user.then_some(system_sx),
        Default::default(),
        kill_flag,
    );
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        runtime
            .execute_script("<sweep>", ascii_str!("require(\"~scene.js\")"))
            .unwrap();
        // let async ops settle; ops that wait on an absent renderer or a permission stay pending
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while let Ok(result) =
            tokio::time::timeout_at(deadline, runtime.run_event_loop(Default::default())).await
        {
            match result {
                Ok(()) => break,
                Err(e) => eprintln!("op sweep: event loop error: {e}"),
            }
        }
    });
    let summary = eval_string(&mut runtime, "JSON.stringify(globalThis.__sweep)");
    eprintln!("op sweep (super_user: {super_user}): {summary}");
    let summary: serde_json::Value = serde_json::from_str(&summary).unwrap();
    assert_eq!(summary["done"], true, "the sweep did not finish: {summary}");
    assert!(
        summary["ops"].as_u64().unwrap() > 200,
        "too few ops swept: {summary}"
    );
    let count = |key: &str| summary[key].as_u64().unwrap();
    // every op took every shape: none ended the sweep's loop early
    assert_eq!(count("calls"), count("ops") * count("shapes"), "{summary}");
    assert!(count("threw") > 0 && count("returned") > 0, "{summary}");
    assert!(count("resolved") + count("rejected") > 0, "no async op settled: {summary}");
    // an op gated on a disabled deno feature proceeds after the check, so none may be reachable
    assert_eq!(
        DISABLED_FEATURE_CALLS.load(std::sync::atomic::Ordering::Relaxed),
        disabled_feature_calls,
        "an op reached a disabled deno feature check"
    );
    // the isolate is still usable after the sweep
    assert_eq!(eval_string(&mut runtime, "String(6 * 7)"), "42");
}

#[test]
fn every_client_op_survives_junk_arguments() {
    sweep_client_ops(false);
}

#[test]
fn every_system_scene_op_survives_junk_arguments() {
    sweep_client_ops(true);
}
