use super::{context, create_runtime};
use common::base_domain::{self, HudServices, Service};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    sync::mpsc,
    time::Duration,
};

// Use the real system module, fetch ops, inbox and scene permission grant.
// Only the renderer replies are supplied by the fixture.
fn run_scene(super_user: bool, services: HudServices, script: String) -> String {
    let (mut runtime, _) = create_runtime(
        false,
        super_user,
        "hud-gateway-test",
        false,
        Default::default(),
    );
    let state = runtime.op_state();
    let (host, mut inbox) = tokio::sync::mpsc::unbounded_channel();
    {
        let mut state = state.borrow_mut();
        state.put(context(false));
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
                    if uri.parse::<http::Uri>().is_err() =>
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
    let scope = &mut runtime.handle_scope();
    deno_core::v8::Local::new(scope, outcome).to_rust_string_lossy(scope)
}

#[test]
fn signed_hud_gateway_fetch_is_scoped_to_engine_routes() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let base = format!("{origin}/mount/notifications");
    let credentialed = base.replacen("http://", "http://operator:password@", 1);
    let marketplace = format!("{origin}/market");
    let share = format!("{origin}/share");
    base_domain::set_services([
        (Service::Notifications, credentialed.as_str()),
        (Service::MarketplaceApi, marketplace.as_str()),
        (Service::Reels, share.as_str()),
    ])
    .unwrap();
    let without_gateway = base_domain::hud_services();
    base_domain::set_gateway(&format!("{origin}/mount")).unwrap();
    let services = base_domain::hud_services();
    let target = format!("{base}/notifications/read?case=Kept&search=two words");
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
            let redirect =
                request.starts_with("GET /mount/camera-reel-service/api/images/redirect/metadata");
            let stalled =
                request.starts_with("GET /mount/camera-reel-service/api/images/stall/metadata");
            seen.send(request).unwrap();
            if stalled {
                reader
                    .get_mut()
                    .write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\ns",
                    )
                    .unwrap();
                // Hold the body open until the fetch cancels and closes its socket.
                reader
                    .get_mut()
                    .set_read_timeout(Some(Duration::from_secs(4)))
                    .unwrap();
                let _ = reader.get_mut().read(&mut [0u8; 1]);
                continue;
            }
            let reply = if redirect {
                "HTTP/1.1 302 Found\r\nLocation: /untargeted\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            } else {
                "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok"
            };
            reader.get_mut().write_all(reply.as_bytes()).unwrap();
        }
    });
    let script = format!("const target={target:?}, base={base:?};")
        + r#"
        const api = require("~system/BevyExplorerApi");
        const services = await api.getServiceUrls();
        if (services.notifications !== base) throw Error("unexpected notifications base");
        const signed = (url, init = {}) => api.kernelFetch({url, init, meta: "{}"});
        async function denied(label, operation) {
            try { await operation(); } catch (error) { return error; }
            throw Error("unexpectedly admitted " + label);
        }
        const response = await signed(target, {
            method: "PUT", body: "Exact Bytes", headers: {"Content-Type": "text/plain"}
        });
        const identity = {
            "x-identity-auth-chain-0": "engine-signed", "x-identity-timestamp": "123", "x-identity-metadata": "{}"
        };
        await denied("replay", () => fetch(target, {method: "PUT", headers: identity}));
        await denied("arbitrary route", () => signed(base + "/admin"));
        for (const headers of [
            {"x-forwarded-host": "attacker.invalid"}, {"Host": "attacker.invalid"}, {"X-Identity-Timestamp": "forged"}
        ]) {
            await denied("forged header", () => signed(target, {method: "PUT", headers}));
        }
        await denied("encoded slash", () => signed(base + "/%2e%2e%2fadmin"));
        await denied("dot segments", () => signed(base + "/anything/../notifications/read", {method: "PUT"}));
        const item = services.marketplaceApi + "/v1/items?id=legacy";
        const headers = await Deno.core.ops.op_kernel_fetch_headers(item, "GET", "{}");
        await api.getServiceUrls();
        await (await fetch(item, {headers: Object.fromEntries(headers)})).text();
        await denied("marketplace writes", () => signed(services.marketplaceApi + "/v1/items", {method: "POST"}));
        await denied("share page", () => signed(services.reels + "/api/images/private/metadata"));
        await denied("redirect", () => signed(services.cameraReel + "/api/images/redirect/metadata"));
        const controller = new AbortController();
        setTimeout(() => controller.abort(), 200);
        const error = await denied("aborted request", () =>
            signed(services.cameraReel + "/api/images/stall/metadata", {signal: controller.signal}));
        return [response.status, response.body, error.name].join(":");"#;
    let privileged = run_scene(true, services.clone(), script);
    let private_override = run_scene(true, without_gateway, r#"
        const api = require("~system/BevyExplorerApi");
        const services = await api.getServiceUrls();
        try {
            await api.kernelFetch({url: services.notifications + "/notifications", init: {method: "GET"}, meta: "{}"});
            return "accepted";
        } catch { return "denied"; }
    "#.into());
    let ordinary = run_scene(
        false,
        services,
        format!(
            r#"try {{await fetch("{target}", {{method:"PUT"}}); return "accepted"}} catch {{return "denied"}}"#
        ),
    );
    stop.send(()).unwrap();
    server.join().unwrap();
    assert_eq!(privileged, "200:ok:AbortError");
    assert_eq!(private_override, "denied");
    assert_eq!(ordinary, "denied");
    let requests: Vec<_> = requests.try_iter().collect();
    assert!((3..=4).contains(&requests.len()), "{requests:?}");
    assert!(requests[1].starts_with("GET /market/v1/items?id=legacy "));
    assert!(requests[2].starts_with("GET /mount/camera-reel-service/api/images/redirect/metadata "));
    if requests.len() == 4 {
        assert!(
            requests[3].starts_with("GET /mount/camera-reel-service/api/images/stall/metadata ")
        );
    }
    let raw = &requests[0];
    assert!(raw.starts_with(
        "PUT /mount/notifications/notifications/read?case=Kept&search=two%20words HTTP/1.1\r\n"
    ));
    assert!(raw.ends_with("Exact Bytes"));
    let headers = raw.to_ascii_lowercase();
    assert!(headers.contains("x-identity-auth-chain-0: engine-signed"));
    assert!(headers.contains("content-type: text/plain") && headers.contains("host: 127.0.0.1:"));
}
