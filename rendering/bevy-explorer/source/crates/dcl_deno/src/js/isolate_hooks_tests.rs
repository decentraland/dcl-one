// Scene code that reaches V8 callbacks or native memory must end at most its own scene. The
// cases that would end the process run in a process of their own, so a regression fails one
// test instead of the whole run.

use super::{context, fetch_outcome, run_scene};
use deno_core::{v8, JsRuntime};
use std::time::Duration;

thread_local! {
    // a scene's ops and timers run on its own thread, as in the sidecar
    static RT: tokio::runtime::Runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
}

const MIB: usize = 1024 * 1024;

/// A client scene; `heap` and `array_buffers` replace the default caps.
fn scene(heap: Option<usize>, array_buffers: Option<usize>) -> (JsRuntime, dcl_js::KillFlag) {
    let handle = RT.with(|rt| rt.handle().clone());
    let _guard = handle.enter();
    let kill_flag = dcl_js::KillFlag::default();
    crate::js::TEST_HEAP_LIMIT.set(heap);
    crate::js::TEST_ARRAY_BUFFER_LIMIT.set(array_buffers);
    let (runtime, _) = crate::js::create_runtime(false, false, kill_flag.clone());
    crate::js::TEST_HEAP_LIMIT.set(None);
    crate::js::TEST_ARRAY_BUFFER_LIMIT.set(None);
    {
        let state = runtime.op_state();
        let mut state = state.borrow_mut();
        state.put(context(false));
        state.put(dcl_types::SceneResourceCounters::default());
        state.put(dcl_types::RpcCalls::default());
        state.put(Vec::<dcl_types::SceneLogMessage>::default());
        state.put(dcl_types::SceneElapsedTime(0.0));
    }
    (runtime, kill_flag)
}

fn eval(runtime: &mut JsRuntime, script: &str) -> Result<String, String> {
    let handle = RT.with(|rt| rt.handle().clone());
    let _guard = handle.enter();
    let value = runtime
        .execute_script("<test>", script.to_owned())
        .map_err(|e| e.to_string())?;
    deno_core::scope!(scope, runtime);
    Ok(v8::Local::new(scope, value).to_rust_string_lossy(scope))
}

fn run_event_loop(runtime: &mut JsRuntime) -> Result<(), String> {
    RT.with(|rt| rt.block_on(async {
        tokio::time::timeout(
            Duration::from_secs(10),
            runtime.run_event_loop(Default::default()),
        )
        .await
    }))
    .map_err(|_| "event loop did not settle".to_owned())?
    .map_err(|e| e.to_string())
}

/// Runs the scene's event loop and the tasks V8 posts to the platform, which the test platform
/// leaves to its embedder, until `done` or ten seconds.
fn drive(runtime: &mut JsRuntime, mut done: impl FnMut(&mut JsRuntime) -> bool) -> bool {
    let platform = v8::V8::get_current_platform();
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while std::time::Instant::now() < deadline {
        while v8::Platform::pump_message_loop(&platform, runtime.v8_isolate(), false) {}
        RT.with(|rt| {
            rt.block_on(async {
                let _ = tokio::time::timeout(
                    Duration::from_millis(5),
                    runtime.run_event_loop(Default::default()),
                )
                .await;
            })
        });
        if done(runtime) {
            return true;
        }
    }
    false
}

/// Runs the ignored test `name` of this module in a process of its own and checks it passed.
fn passes_in_own_process(name: &str) {
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            &format!("js::tests::isolate_hooks::{name}"),
            "--exact",
            "--ignored",
            "--test-threads=1",
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success() && stdout.contains("1 passed"),
        "{name}: {:?}\n{stdout}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn a_scene_cannot_end_the_process_from_wasm_streaming() {
    passes_in_own_process("wasm_streaming_rejects");
}

#[test]
#[ignore = "run in its own process by the test that names it"]
fn wasm_streaming_rejects() {
    for is_server in [false, true] {
        run_scene(
            is_server,
            r#"
            if (typeof WebAssembly.compileStreaming !== "undefined"
                || typeof WebAssembly.instantiateStreaming !== "undefined") {
                throw new Error("streaming compilation is reachable");
            }
            "#,
        )
        .unwrap();
    }
    // a context made after start-up gets the streaming functions back; they reject
    let (mut runtime, _) = scene(None, None);
    deno_core::scope!(scope, runtime);
    let fresh = v8::Context::new(scope, Default::default());
    let scope = &mut v8::ContextScope::new(scope, fresh);
    let source = v8::String::new(
        scope,
        r#"globalThis.out = "pending";
        WebAssembly.compileStreaming(1).then(() => { out = "resolved"; }, (e) => { out = String(e); });
        WebAssembly.instantiateStreaming(1).catch(() => {});"#,
    )
    .unwrap();
    v8::Script::compile(scope, source, None)
        .unwrap()
        .run(scope)
        .unwrap();
    scope.perform_microtask_checkpoint();
    let key = v8::String::new(scope, "out").unwrap();
    let out = fresh.global(scope).get(scope, key.into()).unwrap();
    let out = out.to_rust_string_lossy(scope);
    assert!(out.contains("not available to scenes"), "{out}");
}

#[test]
fn a_scene_cannot_end_the_process_from_a_long_error_message() {
    passes_in_own_process("long_error_strings_are_cut");
}

#[test]
#[ignore = "run in its own process by the test that names it"]
fn long_error_strings_are_cut() {
    let (mut runtime, _) = scene(None, None);
    // each pair of strings is longer than V8's longest string once encoded as UTF-8
    let outcome = eval(
        &mut runtime,
        r#"
        const long = "\u00e9".repeat(2 ** 27);
        const named = new Error(long);
        named.name = long;
        const header = named.stack.length;
        const holder = { [long]: () => { throw new Error(long); } };
        let frame = -1;
        try { holder[long](); } catch (e) { frame = e.stack.length; }
        JSON.stringify([header, frame])
        "#,
    )
    .unwrap();
    let [header, frame]: [usize; 2] = serde_json::from_str(&outcome).unwrap();
    assert!(header > 64 * 1024 && header < 256 * 1024, "{outcome}");
    assert!(frame > 64 * 1024 && frame < 256 * 1024, "{outcome}");
}

#[test]
fn a_scene_cannot_end_the_process_from_code_run_while_formatting_a_stack() {
    passes_in_own_process("formatting_a_stack_runs_no_scene_code");
}

#[test]
#[ignore = "run in its own process by the test that names it"]
fn formatting_a_stack_runs_no_scene_code() {
    let forever = "{ globalThis.ran = true; const keep = []; for (;;) { keep.push(new Array(1e5).fill(1.5)); } }";
    for setup in [
        format!("Object.defineProperty(globalThis, 'Error', {{ get() {forever}, configurable: true }});"),
        format!("Error.prepareStackTrace = () => {forever};"),
        format!("Object.defineProperty(TypeError.prototype, 'name', {{ get() {forever} }});"),
        format!("Object.defineProperty(globalThis, 'made', {{ value: new TypeError('x') }}); Object.defineProperty(made, 'message', {{ get() {forever} }});"),
    ] {
        let (mut runtime, _) = scene(Some(64 * MIB), None);
        let outcome = eval(
            &mut runtime,
            &format!(
                "globalThis.ran = false; {setup} const error = globalThis.made ?? new TypeError('x'); \
                 JSON.stringify([ran, String(error.stack).split('\\n')[0]])"
            ),
        )
        .unwrap();
        let (ran, header): (bool, String) = serde_json::from_str(&outcome).unwrap();
        assert!(!ran, "{setup}: {outcome}");
        assert!(header.starts_with("TypeError") || header == "Error: x", "{setup}: {outcome}");
    }
}

#[test]
fn a_scene_cannot_end_the_process_from_a_wasm_compile_while_terminated() {
    passes_in_own_process("wasm_compile_during_termination");
}

#[test]
#[ignore = "run in its own process by the test that names it"]
fn wasm_compile_during_termination() {
    let (mut runtime, kill_flag) = scene(Some(64 * MIB), None);
    eval(
        &mut runtime,
        r#"
        globalThis.keep = [];
        Object.defineProperty(Object.prototype, "then", {
            get() { for (;;) { keep.push(new Array(1e5).fill(1.5)); } },
            configurable: true,
        });
        WebAssembly.compile(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]));
        "#,
    )
    .unwrap();
    assert!(drive(&mut runtime, |_| kill_flag.killed()));
}

#[test]
fn a_scene_cannot_end_the_process_from_a_failed_dynamic_import() {
    passes_in_own_process("failed_dynamic_import");
}

#[test]
#[ignore = "run in its own process by the test that names it"]
fn failed_dynamic_import() {
    let (mut runtime, _) = scene(None, None);
    eval(
        &mut runtime,
        r#"
        globalThis.out = "pending";
        Object.defineProperty(TypeError.prototype, "name", { get() { throw 1; }, configurable: true });
        import("file:///x.js").then(() => { out = "resolved"; }, () => { out = "rejected"; });
        "#,
    )
    .unwrap();
    let _ = run_event_loop(&mut runtime);
    assert_eq!(eval(&mut runtime, "out").unwrap(), "rejected");
}

/// ArrayBuffers live outside the heap; a scene's together stay within its budget, and past it
/// the allocation fails or the scene ends.
#[test]
fn array_buffers_are_capped_per_scene() {
    let (mut runtime, kill_flag) = scene(None, Some(16 * MIB));
    let budget = runtime
        .op_state()
        .borrow()
        .borrow::<std::sync::Arc<super::super::isolate_hooks::ArrayBufferBudget>>()
        .clone();
    eval(
        &mut runtime,
        "globalThis.keep = [new ArrayBuffer(6 << 20), new Uint8Array(6 << 20)];",
    )
    .unwrap();
    assert!((12 * MIB..=16 * MIB).contains(&budget.used()));
    for script in ["keep.push(new ArrayBuffer(6 << 20))", "keep.push(new Float64Array(1 << 20))"] {
        let outcome = eval(&mut runtime, script);
        assert!(
            outcome.as_ref().is_err_and(|e| e.contains("RangeError") || e.contains("terminated")),
            "{script}: {outcome:?}"
        );
        assert!(budget.used() <= 16 * MIB);
        if kill_flag.killed() {
            break;
        }
    }
}

/// A stream a scene feeds can be read whole only up to a cap.
#[test]
fn reading_a_scene_stream_to_its_end_is_capped() {
    let (mut runtime, _) = scene(None, None);
    eval(
        &mut runtime,
        r#"
        const ops = Deno.core.ops;
        const rid = ops.op_readable_stream_resource_allocate();
        const sink = ops.op_readable_stream_resource_get_sink(rid);
        globalThis.out = "pending";
        ops.op_read_all(rid).then((b) => { out = `read ${b.length}`; }, (e) => {
            out = String(e);
            ops.op_readable_stream_resource_close(sink);
        });
        (async () => {
            const chunk = new Uint8Array(1 << 20);
            for (let i = 0; i < 80; i++) {
                if (!(await ops.op_readable_stream_resource_write_buf(sink, chunk))) { break; }
            }
            ops.op_readable_stream_resource_close(sink);
        })().catch(() => {});
        "#,
    )
    .unwrap();
    run_event_loop(&mut runtime).unwrap();
    let out = eval(&mut runtime, "out").unwrap();
    assert!(out.contains("larger than"), "{out}");
}

/// Only a stream the fetch JS made can be an upload body.
#[test]
fn a_fetch_upload_body_must_be_a_stream() {
    let (mut runtime, _) = scene(None, None);
    let err = eval(
        &mut runtime,
        r#"
        const ops = Deno.core.ops;
        const sink = ops.op_readable_stream_resource_get_sink(ops.op_readable_stream_resource_allocate());
        ops.op_fetch("POST", "http://127.0.0.1:9/", [], null, true, null, sink)
        "#,
    )
    .unwrap_err();
    assert!(err.contains("BadResource") || err.contains("resource"), "{err}");
}

/// Blobs a scene drops free their share of its Blob storage for later ones.
#[test]
fn dropped_blobs_free_their_storage() {
    let (mut runtime, _) = scene(None, None);
    eval(
        &mut runtime,
        r#"
        const { Blob } = Deno.core.loadExtScript("ext:deno_web/09_file.js");
        const tick = () => new Promise((resolve) => setTimeout(resolve, 0));
        globalThis.out = "pending";
        (async () => {
            let made = 0;
            const source = new Uint8Array(1 << 20);
            for (let i = 0; i < 192; i++) {
                try { new Blob([source]); } catch {
                    await tick();
                    await tick();
                    new Blob([source]);
                }
                made++;
                if (i % 8 === 0) { await tick(); }
            }
            out = `made ${made}`;
        })().catch((e) => { out = String(e); });
        "#,
    )
    .unwrap();
    drive(&mut runtime, |runtime| eval(runtime, "out").unwrap() != "pending");
    assert_eq!(eval(&mut runtime, "out").unwrap(), "made 192");
}

/// `require` reads what it evaluated from arrays it owns, whatever Array.prototype holds.
#[test]
fn the_seal_holds_against_array_prototype_accessors() {
    run_scene(
        true,
        r#"
        let reached = false;
        const steal = function (...args) { if (args.length > 5 && args[5]) { reached = true; } };
        for (const index of [0, 1]) {
            Object.defineProperty(Array.prototype, index, {
                get() { return index === 0 ? steal : null; },
                set(_) {},
                configurable: true,
            });
        }
        try { require("~system/Testing"); } catch {}
        delete Array.prototype[0];
        delete Array.prototype[1];
        if (reached) { throw new Error("internals reached scene code"); }
        if (typeof Deno !== "undefined") { throw new Error("`Deno` is reachable"); }
        "#,
    )
    .unwrap();
}

/// `blob()` and multipart `formData()` work on a response, and a File the latter makes reads
/// the coarsened clock.
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
        None,
        format!(
            r#"(async () => {{
                const blob = await (await fetch("http://127.0.0.1:{port}/")).blob();
                const form = await (await fetch("http://127.0.0.1:{port}/")).formData();
                const file = form.get("f");
                return JSON.stringify([blob.size > 0, file.name, await file.text(), file.lastModified % 2]);
            }})().then((s) => globalThis.__outcome = s, (e) => globalThis.__outcome = `error ${{e}}`);"#
        ),
    );
    assert_eq!(outcome, r#"[true,"a.txt","hello",0]"#);
}
