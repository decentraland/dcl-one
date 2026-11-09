use super::*;
use dcl_types::SceneId;
use deno_core::JsRuntime;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    time::timeout,
};

fn client_runtime() -> JsRuntime {
    let (mut runtime, _) = crate::js::create_runtime(false, false, true, Default::default());
    let state = runtime.op_state();
    let mut state = state.borrow_mut();
    state.put(CrdtContext::new(
        SceneId(bevy::prelude::Entity::from_raw(0)),
        "hash".into(),
        "title".into(),
        false,
        true,
        false,
    ));
    state.put(SceneResourceCounters::default());
    state.put(RpcCalls::default());
    runtime
}

fn permissions(runtime: &mut JsRuntime, expected: usize) -> Vec<RpcResultSender<bool>> {
    let calls = std::mem::take(runtime.op_state().borrow_mut().borrow_mut::<RpcCalls>());
    assert_eq!(calls.len(), expected);
    calls
        .into_iter()
        .map(|call| match call {
            RpcCall::RequestGenericPermission { response, .. } => response,
            _ => panic!("expected Fetch permission"),
        })
        .collect()
}

async fn grant(permissions: Vec<RpcResultSender<bool>>) {
    tokio::task::spawn_blocking(move || {
        for permission in permissions {
            permission.send(true);
        }
    })
    .await
    .unwrap();
}

async fn accept(listener: &TcpListener) -> (String, TcpStream) {
    let mut socket = BufReader::new(listener.accept().await.unwrap().0);
    let mut line = String::new();
    socket.read_line(&mut line).await.unwrap();
    (line, socket.into_inner())
}

fn assert_released(runtime: &mut JsRuntime) {
    let state = runtime.op_state();
    let state = state.borrow();
    assert_eq!(outstanding_calls(&state), 0);
    assert!(!state
        .resource_table
        .names()
        .any(|(_, name)| name == "fetchCancelHandle" || name == "readableStream"));
}

#[tokio::test]
async fn all_forty_fetch_and_signed_fetch_calls_complete() {
    let mut runtime = client_runtime();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    runtime
        .execute_script(
            "<test>",
            format!(
                r#"
        const origin = {origin:?};
        const signed = require('~system/SignedFetch').signedFetch({{url: origin + '/signed'}});
    "#
            ),
        )
        .unwrap();
    let calls = std::mem::take(runtime.op_state().borrow_mut().borrow_mut::<RpcCalls>());
    let Some(RpcCall::SignRequest(request)) = calls.into_iter().next() else {
        panic!("signedFetch should request a signature");
    };
    tokio::task::spawn_blocking(move || {
        request
            .response
            .send(Ok(vec![("X-Test".into(), "signed".into())]))
    })
    .await
    .unwrap();
    timeout(
        Duration::from_secs(2),
        std::future::poll_fn(|cx| {
            assert!(runtime.poll_event_loop(cx, Default::default()).is_pending());
            if runtime.op_state().borrow().borrow::<RpcCalls>().is_empty() {
                Poll::Pending
            } else {
                Poll::Ready(())
            }
        }),
    )
    .await
    .expect("signedFetch should queue its Fetch permission");
    runtime
        .execute_script(
            "<test>",
            r#"
        const calls = Array.from({length: 39}, (_, i) =>
            fetch(origin + '/' + i).then(response => response.text()));
        calls.push(signed.then(response => response.body));
        Promise.all(calls).then(bodies => globalThis.success = bodies.every(b => b === 'ok'));
    "#,
        )
        .unwrap();
    grant(permissions(&mut runtime, 40)).await;
    let serve = async {
        for _ in 0..40 {
            let (_, mut socket) = accept(&listener).await;
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
                .await
                .unwrap();
        }
    };
    let (result, ()) = timeout(Duration::from_secs(5), async {
        tokio::join!(runtime.run_event_loop(Default::default()), serve)
    })
    .await
    .expect("all forty Fetch calls should settle");
    result.unwrap();
    runtime
        .execute_script(
            "<test>",
            "if (!globalThis.success) throw Error('expected 40 responses')",
        )
        .unwrap();
    assert_released(&mut runtime);
}

#[tokio::test]
async fn abort_while_awaiting_permission_releases_the_call_before_a_reply() {
    let mut runtime = client_runtime();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let script = runtime
        .load_side_es_module_from_code(
            &deno_core::ModuleSpecifier::parse("ext:test/abort-upload.js").unwrap(),
            format!(r#"
                import {{ ReadableStream }} from "ext:deno_web/06_streams.js";
                globalThis.controller = new AbortController();
                const body = new ReadableStream({{cancel() {{ globalThis.uploadClosed = true; }} }});
                fetch({origin:?}, {{method: "POST", body, signal: controller.signal}})
                    .catch(e => globalThis.failure = e.name);
            "#),
        )
        .await
        .unwrap();
    let evaluated = runtime.mod_evaluate(script);
    let waiting = permissions(&mut runtime, 1);
    assert_eq!(outstanding_calls(&runtime.op_state().borrow()), 1);
    runtime
        .execute_script("<test>", "controller.abort()")
        .unwrap();
    timeout(
        Duration::from_secs(2),
        runtime.run_event_loop(Default::default()),
    )
    .await
    .expect("native Fetch still waits for the permission reply")
    .unwrap();
    evaluated.await.unwrap();
    assert_released(&mut runtime);
    runtime
        .execute_script(
            "<test>",
            "if (failure !== 'AbortError' || !globalThis.uploadClosed) throw Error('expected aborted fetch and cancelled upload')",
        )
        .unwrap();
    grant(waiting).await;
    runtime.run_event_loop(Default::default()).await.unwrap();
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

#[tokio::test]
async fn abort_skips_a_queued_fetch_and_releases_an_active_body_read() {
    let mut runtime = client_runtime();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    runtime
        .execute_script(
            "<test>",
            format!(
                r#"
        const origin = {origin:?};
        const controllers = Array.from({{length:5}}, () => new AbortController());
        const held = controllers.map((c, i) => fetch(origin + '/hold/' + i, {{signal:c.signal}})
            .then(() => 'completed', e => e.name));
    "#
            ),
        )
        .unwrap();
    grant(permissions(&mut runtime, 5)).await;
    let mut sockets = Vec::new();
    let hold_five = async {
        for _ in 0..5 {
            let (line, mut socket) = accept(&listener).await;
            assert!(line.starts_with("GET /hold/"));
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nx")
                .await
                .unwrap();
            sockets.push(socket);
        }
    };
    tokio::select! {
        result = runtime.run_event_loop(Default::default()) => panic!("Fetch finished before bodies were sent: {result:?}"),
        result = timeout(Duration::from_secs(5), hold_five) => result.expect("five requests should reach the server"),
    }
    runtime.execute_script("<test>", r#"
        const queued = new AbortController();
        const cancelled = fetch(origin + '/cancelled', {signal: queued.signal}).then(
            () => { throw Error('queued fetch reached the server'); },
            error => { if (error.name !== 'AbortError') throw error; });
        fetch(origin + '/next').then(async response => {
            if (await response.text() !== 'ok') throw Error('unexpected body');
            for (const c of controllers) c.abort();
            if (!(await Promise.all(held)).every(e => e === 'AbortError')) throw Error('expected five aborts');
        });
    "#).unwrap();
    grant(permissions(&mut runtime, 2)).await;
    assert!(runtime
        .run_event_loop(Default::default())
        .now_or_never()
        .is_none());
    assert_eq!(outstanding_calls(&runtime.op_state().borrow()), 7);
    let aborted = runtime
        .execute_script("<test>", "queued.abort(); cancelled")
        .unwrap();
    let aborted = runtime.resolve(aborted);
    timeout(
        Duration::from_secs(2),
        runtime.with_event_loop_promise(aborted, Default::default()),
    )
    .await
    .expect("queued abort must settle while all five slots are held")
    .unwrap();
    assert_eq!(outstanding_calls(&runtime.op_state().borrow()), 6);
    runtime
        .execute_script("<test>", "controllers[0].abort()")
        .unwrap();
    let serve_next = async {
        let (line, mut socket) = accept(&listener).await;
        assert!(line.starts_with("GET /next "));
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
            .await
            .unwrap();
    };
    let (result, ()) = timeout(Duration::from_secs(5), async {
        tokio::join!(runtime.run_event_loop(Default::default()), serve_next)
    })
    .await
    .expect("the next Fetch must proceed while the other four bodies are still held");
    result.unwrap();
    assert_released(&mut runtime);
}
