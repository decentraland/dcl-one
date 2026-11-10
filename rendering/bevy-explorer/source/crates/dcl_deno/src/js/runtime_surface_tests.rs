use super::{context, create_runtime, fetch_outcome, run_scene};
use deno_core::JsRuntime;
use std::time::{Duration, Instant};

static RT: std::sync::LazyLock<tokio::runtime::Runtime> = std::sync::LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap()
});

/// Scripts run inside a tokio runtime, as in the scene thread: timers schedule on it.
fn eval(runtime: &mut JsRuntime, script: String) -> Result<String, String> {
    let _guard = RT.enter();
    let value = runtime
        .execute_script("<test>", script)
        .map_err(|e| e.to_string())?;
    deno_core::scope!(scope, runtime);
    Ok(deno_core::v8::Local::new(scope, value).to_rust_string_lossy(scope))
}

fn client_runtime() -> JsRuntime {
    let (runtime, _) = create_runtime(false, false, Default::default());
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
    RT.block_on(async {
        tokio::time::timeout(
            Duration::from_secs(5),
            runtime.run_event_loop(Default::default()),
        )
        .await
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
    let (mut runtime, _) = create_runtime(false, true, Default::default());
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
fn every_clock_has_two_millisecond_granularity() {
    let mut runtime = client_runtime();
    let outcome = eval(
        &mut runtime,
        r#"
        const millisOf = (parts) => Number(parts.find((p) => p.type === "fractionalSecond").value);
        const intl = new Intl.DateTimeFormat("en-US", { second: "numeric", fractionalSecondDigits: 3 });
        const clocks = {
            dateNow: () => Date.now(),
            newDate: () => new Date().getTime(),
            performanceNow: () => performance.now(),
            timeOrigin: () => performance.timeOrigin,
            timerNow: () => Deno.core.ops.op_timer_now(),
            intlFormat: () => Number(intl.format().split(".")[1]),
            intlFormatToParts: () => millisOf(intl.formatToParts()),
            temporalInstant: () => Number(Temporal.Now.instant().epochNanoseconds % 1000000000n),
            temporalZoned: () => Temporal.Now.zonedDateTimeISO("UTC").epochMilliseconds,
            temporalPlainTime: () => {
                const t = Temporal.Now.plainTimeISO("UTC");
                return t.millisecond * 1000000 + t.microsecond * 1000 + t.nanosecond;
            },
        };
        const samples = Object.fromEntries(Object.keys(clocks).map((name) => [name, []]));
        const until = Date.now() + 30;
        while (Date.now() < until) {
            for (const [name, read] of Object.entries(clocks)) {
                samples[name].push(read());
            }
        }
        const step = { temporalInstant: 2000000, temporalPlainTime: 2000000 };
        JSON.stringify(Object.fromEntries(Object.entries(samples).map(([name, values]) => [
            name,
            values.every((t) => t % (step[name] ?? 2) === 0),
        ])))
        "#
        .to_owned(),
    )
    .unwrap();
    let outcome: std::collections::BTreeMap<String, bool> = serde_json::from_str(&outcome).unwrap();
    assert!(outcome.len() == 10 && outcome.values().all(|coarse| *coarse), "{outcome:?}");
    // the clocks still advance
    let advanced = eval(
        &mut runtime,
        r#"(() => {
            const read = () => [performance.now(), Date.now(), Deno.core.ops.op_timer_now()];
            const first = read();
            const until = Date.now() + 10;
            while (Date.now() < until) {}
            return JSON.stringify(read().map((t, i) => t > first[i]));
        })()"#
        .to_owned(),
    )
    .unwrap();
    assert_eq!(advanced, "[true,true,true]");
}

/// The coarsened `Date` still behaves as the builtin for everything but the current time.
#[test]
fn date_keeps_its_contract() {
    let mut runtime = client_runtime();
    let outcome = eval(
        &mut runtime,
        r#"
        class Later extends Date {}
        const later = new Later(5);
        JSON.stringify({
            explicit: new Date(5).getTime(),
            parts: new Date(2020, 0, 2).getDate(),
            invalid: Number.isNaN(new Date(undefined).getTime()),
            called: typeof Date(),
            instance: new Date() instanceof Date,
            constructor: new Date().constructor === Date,
            subclass: later instanceof Later && later instanceof Date && later.getTime() === 5,
            name: Date.name,
            length: Date.length,
            parse: Date.parse("1970-01-01T00:00:00.010Z"),
            utc: Date.UTC(1970, 0, 1, 0, 0, 0, 7),
        })
        "#
        .to_owned(),
    )
    .unwrap();
    let outcome: serde_json::Value = serde_json::from_str(&outcome).unwrap();
    assert_eq!(
        outcome,
        serde_json::json!({
            "explicit": 5,
            "parts": 2,
            "invalid": true,
            "called": "string",
            "instance": true,
            "constructor": true,
            "subclass": true,
            "name": "Date",
            "length": 7,
            "parse": 10,
            "utc": 7,
        })
    );
}

#[test]
fn timers_never_fire_early() {
    let mut runtime = client_runtime();
    let started = Instant::now();
    eval(
        &mut runtime,
        r#"setTimeout(() => { globalThis.fired = true; }, 15)"#.to_owned(),
    )
    .unwrap();
    run_event_loop(&mut runtime);
    assert!(started.elapsed() >= Duration::from_millis(15));
    assert_eq!(eval(&mut runtime, "String(globalThis.fired)".to_owned()).unwrap(), "true");
}

/// Runs `f` against a fresh client scene on its own thread, as each scene has in the sidecar.
fn in_scene<T: Send>(f: impl FnOnce(&mut JsRuntime) -> T + Send) -> T {
    std::thread::scope(|s| s.spawn(|| f(&mut client_runtime())).join().unwrap())
}

#[test]
fn blob_parts_are_per_scene() {
    use std::sync::mpsc;
    let (id_sx, id_rx) = mpsc::channel::<String>();
    let (done_sx, done_rx) = mpsc::channel::<()>();
    let read = |runtime: &mut JsRuntime, id: &str| {
        eval(
            runtime,
            format!(
                r#"Deno.core.ops.op_blob_read_part("{id}")
                .then((b) => {{ globalThis.read = String(Array.from(b)); }}, (e) => {{ globalThis.read = String(e); }})"#
            ),
        )
        .unwrap();
        run_event_loop(runtime);
        eval(runtime, "globalThis.read".to_owned()).unwrap()
    };
    std::thread::scope(|s| {
        let owner = s.spawn(move || {
            let mut runtime = client_runtime();
            let id = eval(
                &mut runtime,
                "Deno.core.ops.op_blob_create_part(new Uint8Array([1, 2, 3]))".to_owned(),
            )
            .unwrap();
            id_sx.send(id.clone()).unwrap();
            done_rx.recv().unwrap();
            read(&mut runtime, &id)
        });
        let other = s.spawn(move || {
            let id = id_rx.recv().unwrap();
            let outcome = read(&mut client_runtime(), &id);
            done_sx.send(()).unwrap();
            outcome
        });
        assert!(other.join().unwrap().contains("not found"));
        assert_eq!(owner.join().unwrap(), "1,2,3");
    });
}

#[test]
fn blob_storage_is_capped_per_scene() {
    const MIB: usize = 1024 * 1024;
    let cap = super::super::blob_store::MAX_BLOB_BYTES / MIB;
    let script = format!(
        r#"
        const {{ Blob }} = Deno.core.loadExtScript("ext:deno_web/09_file.js");
        const big = new Blob([new Uint8Array({} * 1024 * 1024)]);
        let refused = "";
        try {{ new Blob([new Uint8Array(2 * 1024 * 1024)]); }} catch (e) {{ refused = String(e); }}
        const small = new Blob([new Uint8Array(1024)]);
        JSON.stringify({{ size: big.size, refused, small: small.size }})
        "#,
        cap - 1
    );
    // a second scene has its own allowance
    for _ in 0..2 {
        let outcome = in_scene(|runtime| eval(runtime, script.clone()).unwrap());
        let outcome: serde_json::Value = serde_json::from_str(&outcome).unwrap();
        assert_eq!(outcome["size"], (cap - 1) * MIB);
        assert!(outcome["refused"].as_str().unwrap().contains("full"), "{outcome}");
        assert_eq!(outcome["small"], 1024);
    }
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
            eval(&mut runtime, r#"new BroadcastChannel("x").postMessage("hi")"#.to_owned())
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
fn a_followed_redirect_reports_its_final_url() {
    let ports = super::redirect_servers();
    let outcome = fetch_outcome(
        false,
        None,
        format!(
            r#"fetch("http://127.0.0.1:{}/redirect")
            .then(async (r) => JSON.stringify({{ status: r.status, redirected: r.redirected, url: r.url, body: await r.text() }}),
                  (e) => `error ${{e}}`)
            .then((s) => globalThis.__outcome = s);"#,
            ports[0]
        ),
    );
    let outcome: serde_json::Value = serde_json::from_str(&outcome).expect(&outcome);
    assert_eq!(
        outcome,
        serde_json::json!({
            "status": 200,
            "redirected": true,
            "url": format!("http://127.0.0.1:{}/target", ports[1]),
            "body": "arrived",
        })
    );
}

/// An https page redirecting to http is refused before the http server sees a request.
#[test]
fn a_redirect_from_https_to_http_is_refused() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    let dir = std::env::temp_dir().join(format!("dcl-fetch-tls-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let status = std::process::Command::new("openssl")
        .args(["req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1"])
        .args(["-subj", "/CN=localhost", "-keyout"])
        .arg(dir.join("key.pem"))
        .arg("-out")
        .arg(dir.join("cert.pem"))
        .stderr(std::process::Stdio::null())
        .status()
        .expect("the test needs the openssl command to make a self-signed certificate");
    assert!(status.success());
    let identity = native_tls::Identity::from_pkcs8(
        &std::fs::read(dir.join("cert.pem")).unwrap(),
        &std::fs::read(dir.join("key.pem")).unwrap(),
    )
    .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let acceptor = native_tls::TlsAcceptor::new(identity).unwrap();

    let plain = TcpListener::bind("127.0.0.1:0").unwrap();
    let plain_port = plain.local_addr().unwrap().port();
    let plain_hits = Arc::new(AtomicUsize::new(0));
    {
        let plain_hits = plain_hits.clone();
        std::thread::spawn(move || {
            for stream in plain.incoming() {
                plain_hits.fetch_add(1, Ordering::SeqCst);
                let _ = stream.unwrap().write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 7\r\nConnection: close\r\n\r\narrived",
                );
            }
        });
    }
    let secure = TcpListener::bind("127.0.0.1:0").unwrap();
    let secure_port = secure.local_addr().unwrap().port();
    let secure_hits = Arc::new(AtomicUsize::new(0));
    {
        let secure_hits = secure_hits.clone();
        std::thread::spawn(move || {
            for stream in secure.incoming() {
                let Ok(mut stream) = acceptor.accept(stream.unwrap()) else {
                    continue;
                };
                secure_hits.fetch_add(1, Ordering::SeqCst);
                let mut buf = [0u8; 4096];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(format!("HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:{plain_port}/target\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").as_bytes());
            }
        });
    }

    let client = || {
        reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap()
    };
    let fetch = |url: String| {
        fetch_outcome(
            false,
            Some(client()),
            format!(
                r#"fetch({url:?}).then(async (r) => `${{r.status}} ${{await r.text()}}`, (e) => `error ${{e}}`)
                .then((s) => globalThis.__outcome = s);"#
            ),
        )
    };
    let refused = fetch(format!("https://localhost:{secure_port}/redirect"));
    assert!(refused.contains("https to http"), "{refused}");
    assert_eq!(secure_hits.load(Ordering::SeqCst), 1);
    assert_eq!(plain_hits.load(Ordering::SeqCst), 0);
    // the same http server is reachable directly
    assert_eq!(fetch(format!("http://127.0.0.1:{plain_port}/target")), "200 arrived");
    assert_eq!(plain_hits.load(Ordering::SeqCst), 1);
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
fn a_refused_connection_rejects_with_fetch_failed_and_its_cause() {
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let outcome = fetch_outcome(
        false,
        None,
        format!(
            r#"fetch("http://127.0.0.1:{port}/").then(() => "resolved",
                (e) => `${{e.name}}: ${{e.message}} / ${{e.cause instanceof Error}}`)
            .then((s) => globalThis.__outcome = s);"#
        ),
    );
    assert_eq!(outcome, "TypeError: fetch failed / true");
}

#[test]
fn the_server_keeps_no_custom_fetch_clients() {
    run_scene(
        true,
        r#"
        let threw = false;
        try { new Request("https://example.org/", { client: {} }); } catch { threw = true; }
        if (!threw) throw new Error("a non-client object was accepted as a client");
        "#,
    )
    .unwrap();
}
