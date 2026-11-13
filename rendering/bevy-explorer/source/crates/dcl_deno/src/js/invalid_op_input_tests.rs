use super::{context, create_runtime, run_scene};
use crate::js::OpOverrideProblems;
use std::{sync::Arc, time::Duration};

use common::structs::SceneJsFile;
use dcl_types::{interface::crdt_context::CrdtContext, SceneId};
use deno_core::{ascii_str, JsRuntime};

fn assert_refused(script: &str, needle: &str) {
    let err = run_scene(false, script).expect_err(script);
    assert!(err.contains(needle), "{script}: unexpected error: {err}");
}

#[test]
fn no_raw_socket_lock_or_cppgc_class_ops_are_reachable() {
    run_scene(
        false,
        r#"
        const ops = Object.keys(Deno.core.ops);
        const raw = ops.filter((name) => /^op_(net|tls|dns|node_unstable)_/.test(name));
        if (raw.length) { throw new Error(`raw socket ops: ${raw}`); }
        for (const name of ["DOMMatrix", "DOMPoint", "ImageData", "CSSStyleSheet", "Console"]) {
            if (name in Deno.core.ops) { throw new Error(`${name} is reachable`); }
        }
        for (const name of ["op_lock_manager_request", "op_lock_manager_query", "op_broadcast_subscribe"]) {
            let threw = false;
            try { Deno.core.ops[name]("x", "exclusive", false, false); } catch { threw = true; }
            if (!threw) { throw new Error(`${name} is enabled`); }
        }
        "#,
    )
    .unwrap();
}

#[test]
fn core_ops_that_abort_on_scene_input_are_disabled() {
    assert_refused(
        r#"Deno.core.ops.op_compile_function("", "file:///x", undefined, [1])"#,
        "op is disabled",
    );
    assert_refused(
        "Deno.core.ops.op_set_handled_promise_rejection_handler(() => {})",
        "op is disabled",
    );
}

#[test]
fn text_decoders_work_and_cap_their_input() {
    run_scene(
        false,
        r#"
        if (new TextDecoder("windows-1252").decode(new Uint8Array([0x80])) !== "\u20ac") {
            throw new Error("single decode");
        }
        const utf16 = new TextDecoder("utf-16le");
        const text = utf16.decode(new Uint8Array([0x41]), { stream: true }) + utf16.decode(new Uint8Array([0]));
        if (text !== "A") { throw new Error(`stream decode: ${text}`); }
        let threw = false;
        try { new TextDecoder("utf-16le", { fatal: true }).decode(new Uint8Array([0x41])); } catch (e) { threw = e instanceof TypeError; }
        if (!threw) { throw new Error("fatal decode accepted malformed input"); }
        "#,
    )
    .unwrap();
    assert_refused(
        r#"Deno.core.ops.op_encoding_decode_single(new Uint8Array(33 * 1024 * 1024), "latin1", false, false)"#,
        "too large",
    );
}

#[test]
fn cancel_handles_are_capped_and_print_does_not_reach_stdout() {
    assert_refused(
        "for (let i = 0; i < 5000; i++) { Deno.core.ops.op_cancel_handle(); }",
        "too many open resources",
    );
    run_scene(false, r#"Deno.core.print("x".repeat(10000), true);"#).unwrap();
}

/// An index accessor on the array prototypes would run inside deno_core's rejection drain.
#[test]
fn prototype_index_accessors_are_refused() {
    for is_server in [false, true] {
        run_scene(
            is_server,
            r#"
            const attempts = {
                defineProperty: () => Object.defineProperty(Array.prototype, "0", { set() {}, configurable: true }),
                numberKey: () => Object.defineProperty(Object.prototype, 5, { get() {}, configurable: true }),
                defineProperties: () => Object.defineProperties(Array.prototype, { 1: { set() {}, configurable: true } }),
                defineSetter: () => Array.prototype.__defineSetter__("2", () => {}),
                defineGetter: () => Object.prototype.__defineGetter__(3, () => {}),
                setPrototypeOf: () => Object.setPrototypeOf(Array.prototype, { set 0(v) {} }),
                protoAssign: () => { Array.prototype.__proto__ = { set 0(v) {} }; },
            };
            for (const [name, attempt] of Object.entries(attempts)) {
                let threw = false;
                try { attempt(); } catch (e) { threw = e instanceof TypeError; }
                if (!threw) { throw new Error(`${name} was allowed`); }
            }
            if (Reflect.defineProperty(Array.prototype, "0", { set() {}, configurable: true })) {
                throw new Error("Reflect.defineProperty was allowed");
            }
            if (Reflect.setPrototypeOf(Array.prototype, {})) { throw new Error("Reflect.setPrototypeOf was allowed"); }
            // named keys, other objects and plain index writes still work
            Object.defineProperty(Array.prototype, "extra", { value: 1, configurable: true });
            Object.defineProperties(Object.prototype, { extra2: { value: 2, configurable: true } });
            Object.defineProperty([], "0", { get() { return 1; } });
            Object.setPrototypeOf({}, null);
            Array.prototype[3] = 1;
            delete Array.prototype[3];
            "#,
        )
        .unwrap();
    }
}

/// Rejections still drain once the scene has tried to put a setter in the drain's path.
#[test]
fn a_rejection_drain_runs_no_scene_setter() {
    let (mut runtime, _) = create_runtime(false, false, Default::default());
    {
        let state = runtime.op_state();
        let mut state = state.borrow_mut();
        state.put(context(true));
        state.put(dcl_types::SceneResourceCounters::default());
        state.put(Vec::<dcl_types::SceneLogMessage>::default());
        state.put(dcl_types::SceneElapsedTime(0.0));
    }
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let _guard = rt.enter();
    runtime
        .execute_script(
            "<test>",
            r#"
            try {
                Object.defineProperty(Array.prototype, "0", { set() { Promise.reject(1); }, configurable: true });
            } catch {}
            setTimeout(() => Promise.reject(0), 0);
            setTimeout(() => { globalThis.__done = true; }, 5);
            "#,
        )
        .unwrap();
    rt.block_on(runtime.run_event_loop(Default::default()))
        .unwrap();
    assert_eq!(eval_string(&mut runtime, "String(globalThis.__done)"), "true");
}

#[test]
fn op_panic_is_refused() {
    assert_refused(r#"Deno.core.ops.op_panic("x")"#, "not available");
}

/// The builtin add ops wrap like a release build instead of overflow-panicking.
#[test]
fn add_ops_wrap_instead_of_overflowing() {
    let (mut runtime, _) = create_runtime(false, false, Default::default());
    runtime.op_state().borrow_mut().put(context(false));
    runtime
        .execute_script(
            "<test>",
            r#"
            globalThis.__sync = Deno.core.ops.op_add(2147483647, 1);
            Deno.core.ops.op_add_async(2147483647, 1).then((v) => { globalThis.__async = v; });
            "#,
        )
        .unwrap();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(runtime.run_event_loop(Default::default()))
        .unwrap();
    assert_eq!(
        eval_string(&mut runtime, "`${globalThis.__sync} ${globalThis.__async}`"),
        "-2147483648 -2147483648"
    );
    // release builds wrap either way, so also pin that the overrides are installed
    let names: Vec<_> = crate::js::hardened_ops::override_ops()
        .iter()
        .map(|op| op.name)
        .collect();
    assert!(names.contains(&"op_add") && names.contains(&"op_add_async"));
}

/// A view left out of bounds by shrinking its resizable buffer is refused, not rebuilt.
#[test]
fn an_out_of_bounds_view_is_refused() {
    // V8 reports an out-of-bounds view as empty at offset 0, so it arrives as an empty view
    // rather than a refused one; either way nothing past the buffer is read
    let err = run_scene(
        false,
        r#"
        const buffer = new ArrayBuffer(16, { maxByteLength: 16 });
        const view = new Uint8Array(buffer, 8, 4);
        buffer.resize(4);
        Deno.core.ops.op_log_test_plan(view);
        "#,
    )
    .expect_err("an out-of-bounds view was accepted");
    assert!(
        err.contains("cannot be passed") || err.contains("missing field"),
        "{err}"
    );
}

/// Each voice chat action decodes through `Plain` in its tagged-enum shape.
#[test]
fn voice_chat_actions_decode_as_plain_data() {
    use crate::js::plain::Plain;
    use deno_core::{v8, FromV8};
    use system_bridge::VoiceChatAction as A;

    let address = || "0xabc".to_owned();
    let community_id = || "c1".to_owned();
    let cases = [
        (r#"{op:"startPrivateCall",address:"0xabc"}"#, A::StartPrivateCall { address: address() }),
        (r#"{op:"acceptCall"}"#, A::AcceptCall),
        (r#"{op:"rejectCall"}"#, A::RejectCall),
        (r#"{op:"endCall"}"#, A::EndCall),
        (r#"{op:"startCommunityCall",communityId:"c1"}"#, A::StartCommunityCall { community_id: community_id() }),
        (r#"{op:"joinCommunityCall",communityId:"c1"}"#, A::JoinCommunityCall { community_id: community_id() }),
        (r#"{op:"leaveCall"}"#, A::LeaveCall),
        (r#"{op:"endCommunityCall"}"#, A::EndCommunityCall),
        (r#"{op:"requestToSpeak",raise:true}"#, A::RequestToSpeak { raise: true }),
        (r#"{op:"promoteSpeaker",address:"0xabc"}"#, A::PromoteSpeaker { address: address() }),
        (r#"{op:"demoteSpeaker",address:"0xabc"}"#, A::DemoteSpeaker { address: address() }),
        (r#"{op:"kickPlayer",address:"0xabc"}"#, A::KickPlayer { address: address() }),
        (r#"{op:"rejectSpeakRequest",address:"0xabc"}"#, A::RejectSpeakRequest { address: address() }),
        (r#"{op:"setMic",enabled:false}"#, A::SetMic { enabled: false }),
        (r#"{op:"dismissError"}"#, A::DismissError),
    ];
    let mut runtime = JsRuntime::new(Default::default());
    for (js, expected) in cases {
        let value = runtime.execute_script("<test>", format!("({js})")).unwrap();
        deno_core::scope!(scope, &mut runtime);
        let value = v8::Local::new(scope, value);
        let Plain(decoded) = Plain::<A>::from_v8(scope, value).expect(js);
        assert_eq!(decoded, expected);
    }
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
        "Deno.core.ops.op_ws_close(9999, 1000, undefined)",
        "Deno.core.ops.op_ws_get_buffer(9999)",
        "Deno.core.ops.op_ws_get_buffer_as_string(9999)",
        "Deno.core.ops.op_ws_get_close_code(9999)",
        "Deno.core.ops.op_ws_get_error(9999)",
    ] {
        assert_refused(script, "Bad resource ID");
    }
    for op in ["op_ws_connect", "op_ws_next_event"] {
        let (mut runtime, _) = create_runtime(false, false, Default::default());
        runtime.op_state().borrow_mut().put(context(false));
        let script = format!(
            r#"Deno.core.ops.{op}(9999).then(() => {{ globalThis.__ws = "settled"; }}, (e) => {{ globalThis.__ws = String(e); }})"#
        );
        runtime.execute_script("<test>", script).unwrap();
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(runtime.run_event_loop(Default::default()))
            .unwrap();
        let outcome = eval_string(&mut runtime, "String(globalThis.__ws)");
        assert!(outcome.contains("Bad resource ID"), "{op}: {outcome}");
    }
}

#[test]
fn websocket_create_rejects_urls_it_cannot_dial() {
    for (url, needle) in [
        ("http://example.org/", "ws:// or wss://"),
        ("wss:", "empty host"),
        ("wss://", "empty host"),
        ("ws://example.org/", "must be `wss`"),
        ("wss://example.org/#x", "fragment"),
        ("wss://example.org:0/", "port"),
    ] {
        let script = format!(r#"Deno.core.ops.op_ws_create("{url}", "")"#);
        let err = run_scene(false, &script).expect_err(url);
        assert!(err.contains(needle), "{url}: unexpected error: {err}");
    }
    for protocols in ["a,b", "a, a", "a b", ", a"] {
        let script = format!(r#"Deno.core.ops.op_ws_create("wss://example.org/", "{protocols}")"#);
        let err = run_scene(false, &script).expect_err(protocols);
        assert!(err.contains("protocol"), "{protocols}: unexpected error: {err}");
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
            "getter or setter",
        ),
        (
            "const a = [['k', 'v']]; Object.defineProperty(a[0], 1, { get() { throw new Error('x'); } }); Deno.core.ops.op_url_stringify_search_params(a)",
            "getter or setter",
        ),
        (
            "Deno.core.ops.op_log_test_plan({ tests: [new String('x')] })",
            "cannot be passed",
        ),
    ] {
        assert_refused(script, needle);
    }
    // a getter is refused without running, and an array hole never reads the prototype
    run_scene(
        false,
        r#"
        let ran = false;
        try { Deno.core.ops.op_log_test_plan({ get tests() { ran = true; return []; } }); } catch {}
        __bootstrap.primordials.ObjectDefineProperty(Array.prototype, 1, { get() { ran = true; return "x"; }, configurable: true });
        try { Deno.core.ops.op_url_stringify_search_params([["k", "v"], , ["a", "b"]]); } catch {}
        delete Array.prototype[1];
        if (ran) { throw new Error("a getter ran while an op read its argument"); }
        "#,
    )
    .unwrap();
}

#[test]
fn web_platform_ops_validate_their_buffers_and_state() {
    for (script, needle) in [
        ("Deno.core.ops.op_url_get_serialization()", "no URL serialization is pending"),
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
fn structured_clone_ignores_the_host_object_brand() {
    run_scene(
        false,
        r#"
        const branded = { [Symbol.for("Deno.core.hostObject")]() { return { type: "x" }; }, a: 1 };
        const mark = performance.mark("m", { detail: branded });
        if (mark.detail.a !== 1) { throw new Error("mark detail was not cloned"); }
        const copy = Deno.core.ops.op_structured_clone(branded, { x: 1 });
        if (copy.a !== 1) { throw new Error("the clone lost data"); }
        let threw = false;
        try { Deno.core.ops.op_structured_clone(new SharedArrayBuffer(1)); } catch { threw = true; }
        if (!threw) { throw new Error("a shared buffer was cloned"); }
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
    let (mut runtime, _) = create_runtime(false, false, Default::default());
    runtime.op_state().borrow_mut().put(context(false));
    runtime
        .execute_script(
            "<test>",
            r#"
            const ops = Deno.core.ops;
            const rid = ops.op_readable_stream_resource_allocate_sized(1e19);
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

/// Every op a scene can call is ours, overridden, disabled or on the reviewed list.
#[test]
fn every_upstream_op_is_reviewed() {
    use crate::js::{hardened_ops, own_ops, stream_resource};
    let overridden: Vec<_> = hardened_ops::override_ops()
        .into_iter()
        .chain(stream_resource::override_ops())
        .map(|op| op.name)
        .collect();
    let lists = [
        ("overridden", overridden.as_slice()),
        ("disabled", hardened_ops::DISABLED_OPS),
        ("reviewed", hardened_ops::REVIEWED_UPSTREAM_OPS),
    ];
    let own: std::collections::HashSet<_> = own_ops(true).iter().map(|op| op.name).collect();
    let mut registered = std::collections::HashSet::new();
    for super_user in [false, true] {
        let (mut runtime, _) = create_runtime(false, super_user, Default::default());
        let names = eval_string(&mut runtime, "Object.keys(Deno.core.ops).join(',')");
        registered.extend(names.split(',').map(str::to_owned));
    }
    let unreviewed: Vec<_> = registered
        .iter()
        .filter(|name| {
            !own.contains(name.as_str())
                && !lists.iter().any(|(_, list)| list.contains(&name.as_str()))
        })
        .collect();
    assert!(unreviewed.is_empty(), "unreviewed upstream ops: {unreviewed:?}");
    for (kind, list) in lists {
        for name in list {
            assert!(registered.contains(*name), "{kind} op {name} is not registered");
            let on = lists.iter().filter(|(_, other)| other.contains(name)).count();
            assert_eq!(on, 1, "{name} is on more than one list");
        }
    }
}

#[test]
fn every_override_replaces_an_op_of_the_same_shape() {
    for super_user in [false, true] {
        let (runtime, _) = create_runtime(false, super_user, Default::default());
        let state = runtime.op_state();
        let state = state.borrow();
        let problems = &state.borrow::<OpOverrideProblems>().0;
        assert!(problems.is_empty(), "{problems:#?}");
    }
}

fn eval_string(runtime: &mut JsRuntime, script: &'static str) -> String {
    let value = runtime.execute_script("<test>", script).unwrap();
    deno_core::scope!(scope, runtime);
    deno_core::v8::Local::new(scope, value).to_rust_string_lossy(scope)
}

struct Upload {
    content_length: Option<usize>,
    body: Vec<u8>,
    outcome: String,
    left_a_stream: bool,
}

/// POSTs the `body` that `setup` declares to a loopback server, which reads it by its
/// `Content-Length` and answers with the byte count and sum.
fn upload(setup: &str) -> Upload {
    upload_from(false, setup)
}

fn upload_from(is_server: bool, setup: &str) -> Upload {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream);
        let mut content_length = None;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if line == "\r\n" {
                break;
            }
            if let Some((name, value)) = line.split_once(':') {
                if name.eq_ignore_ascii_case("content-length") {
                    content_length = Some(value.trim().parse().unwrap());
                }
            }
        }
        let mut body = vec![0u8; content_length.unwrap_or(0)];
        reader.read_exact(&mut body).unwrap();
        let reply = format!("{} {}", body.len(), body.iter().map(|b| *b as u64).sum::<u64>());
        write!(
            reader.get_mut(),
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}",
            reply.len()
        )
        .unwrap();
        (content_length, body)
    });

    let (mut runtime, _) = create_runtime(false, false, Default::default());
    let state = runtime.op_state();
    {
        let mut state = state.borrow_mut();
        state.put(CrdtContext::new(
            SceneId(bevy::prelude::Entity::from_raw(0)),
            "hash".into(),
            "title".into(),
            false,
            true,
            is_server,
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
                &deno_core::ModuleSpecifier::parse("file:///test/upload_body.js").unwrap(),
                format!(
                    r#"
                const {{ ReadableStream }} = Deno.core.loadExtScript("ext:deno_web/06_streams.js");
                const {{ Blob }} = Deno.core.loadExtScript("ext:deno_web/09_file.js");
                {setup}
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
    // nothing reached the server: release its accept, and read its reply so it can finish
    let mut release = None;
    if !server.is_finished() {
        if let Ok(mut stream) = std::net::TcpStream::connect(("127.0.0.1", port)) {
            let _ = stream.write_all(b"\r\n");
            release = Some(stream);
        }
    }
    let (content_length, body) = server.join().unwrap();
    drop(release);
    let left_a_stream = state
        .borrow()
        .resource_table
        .names()
        .any(|(_, name)| name.starts_with("readableStream"));
    Upload {
        content_length,
        body,
        outcome: eval_string(&mut runtime, "String(globalThis.outcome)"),
        left_a_stream,
    }
}

/// A fetch with a streamed body reaches the server whole, through every producer path:
/// sync writes, the backpressured async write and close.
#[test]
fn streamed_upload_reaches_the_server() {
    let upload = upload(
        r#"
        const chunks = [new Uint8Array(10).fill(1), new Uint8Array(200 * 1024).fill(2), new Uint8Array(5).fill(3)];
        const body = new ReadableStream({
            pull(controller) {
                const chunk = chunks.shift();
                if (chunk) { controller.enqueue(chunk); } else { controller.close(); }
            },
        });
        "#,
    );
    let expected_sum = 10 + 200 * 1024 * 2 + 5 * 3;
    assert_eq!(upload.outcome, format!("{} {expected_sum}", 10 + 200 * 1024 + 5));
    assert_eq!(&upload.body[..10], &[1; 10]);
    assert_eq!(&upload.body[upload.body.len() - 5..], &[3; 5]);
    assert!(!upload.left_a_stream, "the upload left a stream resource behind");
}

/// A sized body sends its length, however large, while readers never preallocate it.
#[test]
fn a_large_blob_upload_keeps_its_content_length() {
    let length = 17 * 1024 * 1024;
    let upload = upload(&format!(
        "const body = new Blob([new Uint8Array({length}).fill(1)]);"
    ));
    assert_eq!(upload.content_length, Some(length));
    assert_eq!(upload.outcome, format!("{length} {length}"));
    assert!(!upload.left_a_stream, "the upload left a stream resource behind");
}

/// A server scene's upload, streamed or not, is bounded like its response bodies.
#[test]
fn a_server_scene_upload_is_capped() {
    let stream = |chunks: usize| {
        format!(
            r#"
            let left = {chunks};
            const body = new ReadableStream({{
                pull(controller) {{
                    if (left-- > 0) {{ controller.enqueue(new Uint8Array(1024 * 1024).fill(1)); }} else {{ controller.close(); }}
                }},
            }});
            "#
        )
    };
    let sent = upload_from(true, &stream(3));
    assert_eq!(sent.outcome, format!("{0} {0}", 3 * 1024 * 1024));
    let refused = upload_from(true, &stream(11));
    assert!(refused.outcome.contains("exceeds"), "{}", refused.outcome);
    assert_eq!(refused.content_length, None, "the server received a request");
    assert!(!refused.left_a_stream, "the upload left a stream resource behind");
    let buffered = upload_from(true, "const body = new Uint8Array(11 * 1024 * 1024);");
    assert!(buffered.outcome.contains("exceeds"), "{}", buffered.outcome);
    assert_eq!(buffered.content_length, None, "the server received a request");
}

/// Buffers reach serde_v8 without their prototype, so no scene getter runs while an op
/// argument is decoded.
#[test]
fn buffer_arguments_run_no_scene_code() {
    run_scene(
        false,
        r#"
        const ran = [];
        for (const key of ["tests", "start", "len"]) {
            Object.defineProperty(Object.prototype, key, { get() { ran.push(key); return undefined; }, configurable: true });
        }
        for (const value of [new Uint8Array(4), new Float64Array(2), new ArrayBuffer(8), new DataView(new ArrayBuffer(4))]) {
            try { Deno.core.ops.op_log_test_plan(value); } catch {}
            try { Deno.core.ops.op_blob_slice_part("x", value); } catch {}
        }
        if (ran.length > 0) { throw new Error(`scene getters ran: ${ran}`); }
        "#,
    )
    .unwrap();
}

/// `performance.now()` works whatever the buffer's alignment.
#[test]
fn performance_now_takes_a_misaligned_buffer() {
    run_scene(
        false,
        r#"
        const bytes = new Uint8Array(9);
        Deno.core.ops.op_now(bytes.subarray(1));
        const [seconds, nanos] = new Uint32Array(bytes.slice(1).buffer);
        if (nanos % 2000000 !== 0 || nanos >= 1e9 || seconds > 60) {
            throw new Error(`op_now wrote ${seconds} ${nanos}`);
        }
        if (!(performance.now() >= 0)) { throw new Error("performance.now broke"); }
        "#,
    )
    .unwrap();
}

/// A runtime holds a bounded number of streams, and a stream reserves its sink's slot so
/// taking the sink never fails at the cap.
#[test]
fn open_streams_are_capped_per_runtime() {
    run_scene(
        false,
        r#"
        const ops = Deno.core.ops;
        const rids = [];
        for (let i = 0; i < 128; i++) { rids.push(ops.op_readable_stream_resource_allocate()); }
        let refused = "";
        try { ops.op_readable_stream_resource_allocate_sized(1); } catch (e) { refused = String(e); }
        if (!refused.includes("too many open streams")) { throw new Error(`not refused: ${refused}`); }
        const sinks = rids.map((rid) => ops.op_readable_stream_resource_get_sink(rid));
        ops.op_readable_stream_resource_close(sinks[0]);
        ops.op_close(rids[0]);
        ops.op_readable_stream_resource_allocate_sized(1);
        "#,
    )
    .unwrap();
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
fn sweep_ops(super_user: bool, is_server: bool) {
    let kill_flag = dcl_js::KillFlag::default();
    let (mut runtime, _) = create_runtime(false, super_user, kill_flag.clone());
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
            // past argument decoding: a valid specifier with non-string items
            ["", "file:///x", undefined, [1, trap()]],
            ["", "file:///x", [1, trap()], [1]],
            // a branded object reaches structured clone's host-object path
            [{{ [Symbol.for("Deno.core.hostObject")]() {{ return {{ type: "x" }}; }} }}],
        ];
        const ops = Deno.core.ops;
        const names = Object.keys(ops).sort();
        // deno_core builtins and the overrides are swept with the extension ops
        for (const name of ["op_timer_now", "op_now", "op_time_origin", "op_load_ext_script", "op_eval_context",
                            "op_serialize", "op_structured_clone", "op_blob_create_part", "op_blob_clone_part", "op_fetch",
                            "op_fetch_send", "op_fetch_promise_is_settled", "op_utf8_to_byte_string"]) {{
            if (!names.includes(name)) throw new Error(`${{name}} is not swept`);
        }}
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
                // free stream slots so later stream ops still reach their own checks
                if (typeof value === "number" && /^op_readable_stream_resource_(allocate|allocate_sized|get_sink)$/.test(name)) {{
                    try {{ ops.op_close(value); }} catch {{}}
                }}
                if (value instanceof Promise) {{
                    value.then(() => result.resolved++, () => result.rejected++);
                }}
            }});
        }}
        result.done = true;
        "#
    );
    let state = runtime.op_state();
    dcl_js::init_state(
        &mut *state.borrow_mut(),
        Default::default(),
        context(is_server),
        "op-sweep".to_owned(),
        SceneJsFile(Arc::new(sweep.clone())),
        Default::default(),
        dcl_types::SceneResponseSender::new(SceneId::DUMMY, scene_sx),
        renderer_rx,
        global_rx,
        super_user.then_some(system_sx),
        Default::default(),
        kill_flag,
    );
    let sweep_source = sweep;
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        // the server seals `~scene.js` away from the ops table, so there the sweep runs as a
        // plain script with the same ops
        if is_server {
            runtime.execute_script("<sweep>", sweep_source).unwrap();
        } else {
            runtime
                .execute_script("<sweep>", ascii_str!("require(\"~scene.js\")"))
                .unwrap();
        }
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
    // async ops still settle after the sweep
    {
        let _runtime = rt.enter();
        runtime
            .execute_script(
                "<after>",
                ascii_str!("globalThis.__after = 'pending'; Deno.core.ops.op_void_async().then(() => { __after = 'settled'; });"),
            )
            .unwrap();
    }
    for _ in 0..100 {
        rt.block_on(async {
            let _ = tokio::time::timeout(
                Duration::from_millis(50),
                runtime.run_event_loop(Default::default()),
            )
            .await;
        });
        if eval_string(&mut runtime, "String(globalThis.__after)") == "settled" {
            break;
        }
    }
    assert_eq!(eval_string(&mut runtime, "String(globalThis.__after)"), "settled");
    let summary = eval_string(&mut runtime, "JSON.stringify(globalThis.__sweep)");
    eprintln!("op sweep (super_user: {super_user}, is_server: {is_server}): {summary}");
    let summary: serde_json::Value = serde_json::from_str(&summary).unwrap();
    assert_eq!(summary["done"], true, "the sweep did not finish: {summary}");
    assert!(
        summary["ops"].as_u64().unwrap() > 200,
        "too few ops swept: {summary}"
    );
    let count = |key: &str| summary[key].as_u64().unwrap();
    assert!(count("threw") > 0 && count("returned") > 0, "{summary}");
    assert!(count("resolved") + count("rejected") > 0, "no async op settled: {summary}");
    // the isolate is still usable after the sweep
    assert_eq!(eval_string(&mut runtime, "String(6 * 7)"), "42");
}

#[test]
fn every_client_op_survives_junk_arguments() {
    sweep_ops(false, false);
}

#[test]
fn every_system_scene_op_survives_junk_arguments() {
    sweep_ops(true, false);
}

#[test]
fn every_server_scene_op_survives_junk_arguments() {
    sweep_ops(false, true);
}
