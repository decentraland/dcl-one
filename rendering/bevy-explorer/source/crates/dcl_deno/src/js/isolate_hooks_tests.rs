// Scene code that reaches V8 callbacks must end at most its own scene. The cases that would
// end the process run in a process of their own, so a regression fails one test instead of
// the whole run.

use super::{context, create_runtime};
use deno_core::{v8, JsRuntime};
use std::time::Duration;

thread_local! {
    // a scene's ops and timers run on its own thread, as in the sidecar
    static RT: tokio::runtime::Runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
}

fn scene() -> (JsRuntime, dcl_js::KillFlag) {
    let handle = RT.with(|rt| rt.handle().clone());
    let _guard = handle.enter();
    let kill_flag = dcl_js::KillFlag::default();
    let (mut runtime, _) = create_runtime(false, false, "isolate-hooks", false, kill_flag.clone());
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
    let scope = &mut runtime.handle_scope();
    Ok(v8::Local::new(scope, value).to_rust_string_lossy(scope))
}

/// Runs the scene's event loop and the tasks V8 posts to the platform until `done` or ten
/// seconds.
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
fn a_scene_cannot_end_the_process_from_a_long_error_message() {
    passes_in_own_process("long_error_strings_are_cut");
}

#[test]
#[ignore = "run in its own process by the test that names it"]
fn long_error_strings_are_cut() {
    let (mut runtime, _) = scene();
    // each pair of strings is longer than V8's longest string once encoded as UTF-8
    let outcome = eval(
        &mut runtime,
        r#"
        const long = "é".repeat(2 ** 27);
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
fn a_scene_cannot_end_the_process_without_an_error_global() {
    passes_in_own_process("stacks_format_without_the_error_global");
}

#[test]
#[ignore = "run in its own process by the test that names it"]
fn stacks_format_without_the_error_global() {
    for setup in ["delete globalThis.Error;", "globalThis.Error = 5;"] {
        let (mut runtime, _) = scene();
        let outcome = eval(
            &mut runtime,
            &format!("{setup} String(new TypeError('x').stack).split('\\n')[0]"),
        )
        .unwrap();
        assert_eq!(outcome, "TypeError: x", "{setup}");
    }
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
        let (mut runtime, _) = scene();
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
    let (mut runtime, kill_flag) = scene();
    // the compile can settle before the script returns, ending it as terminated
    let outcome = eval(
        &mut runtime,
        r#"
        globalThis.keep = [];
        Object.defineProperty(Object.prototype, "then", {
            get() { for (;;) { keep.push(new Array(1e5).fill(1.5)); } },
            configurable: true,
        });
        WebAssembly.compile(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]));
        "#,
    );
    assert!(
        outcome.as_ref().is_ok() || outcome.as_ref().is_err_and(|e| e.contains("terminated")),
        "{outcome:?}"
    );
    assert!(drive(&mut runtime, |_| kill_flag.killed()));
}

#[test]
fn a_scene_cannot_end_the_process_from_a_failed_dynamic_import() {
    passes_in_own_process("failed_dynamic_import");
}

#[test]
#[ignore = "run in its own process by the test that names it"]
fn failed_dynamic_import() {
    let (mut runtime, _) = scene();
    eval(
        &mut runtime,
        r#"
        globalThis.out = "pending";
        Object.defineProperty(TypeError.prototype, "name", { get() { throw 1; }, configurable: true });
        import("file:///x.js").then(() => { out = "resolved"; }, () => { out = "rejected"; });
        "#,
    )
    .unwrap();
    assert!(drive(&mut runtime, |runtime| eval(runtime, "out").unwrap() != "pending"));
    assert_eq!(eval(&mut runtime, "out").unwrap(), "rejected");
}
