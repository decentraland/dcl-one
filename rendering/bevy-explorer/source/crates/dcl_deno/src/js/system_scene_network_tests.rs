use super::{context, create_runtime};
use common::base_domain::{self, Service};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    sync::mpsc,
    time::Duration,
};

// Use the real system module, fetch ops, inbox and scene permission grant.
// Only the renderer replies are supplied by the fixture.
fn run_scene(
    super_user: bool,
    is_server: bool,
    services: Vec<(String, String)>,
    script: String,
) -> String {
    let (mut runtime, _) = create_runtime(
        false,
        super_user,
        Default::default(),
    );
    let state = runtime.op_state();
    let (host, mut inbox) = tokio::sync::mpsc::unbounded_channel();
    {
        let mut state = state.borrow_mut();
        state.put(context(is_server));
        state.put(dcl_types::SceneResourceCounters::default());
        state.put(dcl_types::RpcCalls::default());
        state.put(Vec::<dcl_types::SceneLogMessage>::default());
        state.put(dcl_types::SceneElapsedTime(0.0));
        if super_user {
            state.put(dcl_js::SuperUserScene(host));
        }
    }
    std::thread::spawn(move || {
        while let Some(event) = inbox.blocking_recv() {
            match event {
                system_bridge::SystemApi::GetServiceUrls(response) => {
                    response.send(services.clone());
                }
                system_bridge::SystemApi::SignRequest { uri, response, .. }
                    if reqwest::Url::parse(&uri).is_err() =>
                {
                    response.send(Err("invalid signing URI".into()))
                }
                system_bridge::SystemApi::SignRequest { response, .. } => response.send(Ok(vec![
                    ("x-identity-auth-chain-0".into(), "engine-signed".into()),
                    ("x-identity-timestamp".into(), "123".into()),
                    ("x-identity-metadata".into(), "{}".into()),
                ])),
                _ => {}
            }
        }
    });
    let (permissions, grants) = mpsc::channel::<common::rpc::RpcResultSender<bool>>();
    std::thread::spawn(move || {
        for grant in grants {
            grant.send(true);
        }
    });
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
        runtime.execute_script("<test>", format!("globalThis.result = 'pending'; (async () => {{{script}}})().then(x => result=x, e => result=String(e))")).unwrap();
        let grant = async {
            loop {
                tokio::time::sleep(Duration::from_millis(2)).await;
                for call in std::mem::take(&mut **state.borrow_mut().borrow_mut::<dcl_types::RpcCalls>()) {
                    if let common::rpc::RpcCall::RequestGenericPermission {response,..}=call { permissions.send(response).unwrap(); }
                }
            }
        };
        tokio::select! {
            r = tokio::time::timeout(Duration::from_secs(8), runtime.run_event_loop(Default::default())) => r.unwrap().unwrap(),
            _ = grant => unreachable!(),
        }
    });
    let outcome = runtime
        .execute_script("<test>", "String(globalThis.result)")
        .unwrap();
    deno_core::scope!(scope, runtime);
    deno_core::v8::Local::new(scope, outcome).to_rust_string_lossy(scope)
}

#[test]
fn operator_system_scene_fetches_private_http_services() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let base = format!(
        "http://localhost:{}/notifications",
        listener.local_addr().unwrap().port()
    );
    let credentialed = base.replacen("http://", "http://operator:password@", 1);
    base_domain::set_services([(Service::Notifications, credentialed.as_str())]).unwrap();
    let services = base_domain::hud_services();
    let (seen, requests) = mpsc::channel();
    listener.set_nonblocking(true).unwrap();
    let (stop, stopped) = mpsc::channel::<()>();
    let server = std::thread::spawn(move || {
        while matches!(stopped.try_recv(), Err(mpsc::TryRecvError::Empty)) {
            let Ok((stream, _)) = listener.accept() else {
                std::thread::sleep(Duration::from_millis(2));
                continue;
            };
            stream.set_nonblocking(false).unwrap();
            let mut reader = BufReader::new(stream);
            let mut request = String::new();
            let mut size = 0;
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap() == 0 || line == "\r\n" {
                    break;
                }
                if let Some(n) = line.to_ascii_lowercase().strip_prefix("content-length: ") {
                    size = n.trim().parse().unwrap();
                }
                request.push_str(&line);
            }
            let mut body = vec![0; size];
            reader.read_exact(&mut body).unwrap();
            request.push_str(&String::from_utf8(body).unwrap());
            let redirect = request.starts_with("GET /redirect ");
            let stalled = request.starts_with("GET /stall ");
            seen.send(request).unwrap();
            if stalled {
                reader
                    .get_mut()
                    .write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\ns",
                    )
                    .unwrap();
                reader
                    .get_mut()
                    .set_read_timeout(Some(Duration::from_secs(4)))
                    .unwrap();
                let _ = reader.get_mut().read(&mut [0u8; 1]);
                continue;
            }
            let reply = if redirect {
                "HTTP/1.1 302 Found\r\nLocation: http://[fd00:ec2::254]/latest/meta-data/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            } else {
                "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok"
            };
            reader.get_mut().write_all(reply.as_bytes()).unwrap();
        }
    });
    let script = format!("const origin={origin:?}, base={base:?};")
        + r#"
        // Native privilege applies before service discovery and to ordinary fetch too.
        if (await (await fetch(origin + "/plain")).text() !== "ok") throw Error("plain fetch failed");
        const api = require("~system/BevyExplorerApi");
        const services = await api.getServiceUrls();
        if (services.notifications !== base) throw Error("service URL exposed credentials");
        const response = await api.kernelFetch({
            url: services.notifications + "/custom?case=Kept&search=two words", meta: "{}",
            init: {method: "PUT", body: "Exact Bytes", headers: {"Content-Type": "text/plain", "X-Feature": "custom"}}
        });
        async function denied(url, init) {
            try { await fetch(url, init); } catch (error) { return error; }
            throw Error("unexpectedly admitted " + url);
        }
        for (const url of ["http://169.254.169.254/latest/meta-data/", "http://[fd00:ec2::254]/latest/meta-data/", "http://[fe80::1]/", origin + "/redirect"]) {
            await denied(url);
        }
        const controller = new AbortController();
        setTimeout(() => controller.abort(), 200);
        const error = await denied(origin + "/stall", {signal: controller.signal});
        return [response.status, response.body, error.name].join(":");
    "#;
    let privileged = run_scene(true, false, services.clone(), script);
    let denied = format!(
        r#"
        for (const url of [{origin:?}, {base:?}]) {{
            try {{ await fetch(url); return "accepted"; }} catch (error) {{
                if (!String(error).includes("non-public")) return String(error);
            }}
        }}
        return "denied";
    "#
    );
    let ordinary = run_scene(false, false, services.clone(), denied.clone());
    let hosted = run_scene(true, true, services, denied);
    stop.send(()).unwrap();
    server.join().unwrap();
    assert_eq!(privileged, "200:ok:AbortError");
    assert_eq!(ordinary, "denied");
    assert_eq!(hosted, "denied");
    let requests: Vec<_> = requests.try_iter().collect();
    assert!((3..=4).contains(&requests.len()), "{requests:?}");
    assert!(requests[0].starts_with("GET /plain HTTP/1.1\r\n"));
    let raw = &requests[1];
    assert!(raw.starts_with("PUT /notifications/custom?case=Kept&search=two%20words HTTP/1.1\r\n"));
    assert!(raw.ends_with("Exact Bytes"));
    let headers = raw.to_ascii_lowercase();
    assert!(headers.contains("x-identity-auth-chain-0: engine-signed"));
    assert!(headers.contains("content-type: text/plain"));
    assert!(headers.contains("x-feature: custom"));
    assert!(!headers.contains("authorization:"));
    assert!(requests[2].starts_with("GET /redirect "));
    if requests.len() == 4 {
        assert!(requests[3].starts_with("GET /stall "));
    }
}
