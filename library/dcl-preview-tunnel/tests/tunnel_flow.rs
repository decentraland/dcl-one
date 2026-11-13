use dcl_preview_tunnel::protocol::{ChannelKind, Control, decode_data, encode_data};
use dcl_preview_tunnel::{AppState, Config, router};
use futures::{SinkExt, StreamExt};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

type Socket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

/// The guard on a frame that must arrive. Long enough that no load on the
/// machine trips it, so only a broken build ever waits it out.
const HANG_GUARD: Duration = Duration::from_secs(30);

/// The listener is bound and listening before it is handed to the server, so a
/// connect made as soon as this returns waits in the backlog until it is served.
async fn spawn_service(cfg: Config) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = router(Arc::new(AppState::new(cfg)));
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    addr
}

/// Closes the agent's side of a trunk and returns once the service has dropped
/// it. The service tears the agent down before it lets the socket go, and it
/// never answers the close frame first, so the stream ending here comes after
/// the teardown.
async fn close_trunk(mut trunk: Socket) {
    trunk.close(None).await.unwrap();
    tokio::time::timeout(HANG_GUARD, async {
        while let Some(Ok(_)) = trunk.next().await {}
    })
    .await
    .expect("the service never dropped the closed trunk");
}

async fn recv_control(socket: &mut Socket) -> Control {
    loop {
        let msg = tokio::time::timeout(HANG_GUARD, socket.next())
            .await
            .expect("timed out waiting for a control frame")
            .expect("trunk ended unexpectedly")
            .expect("trunk frame error");
        match msg {
            Message::Text(text) => match Control::decode(&text).expect("valid control json") {
                Control::Ping | Control::Pong => continue,
                control => return control,
            },
            Message::Ping(_) | Message::Pong(_) => continue,
            other => panic!("expected a text control frame, got {other:?}"),
        }
    }
}

async fn recv_binary(socket: &mut Socket) -> Vec<u8> {
    loop {
        let msg = tokio::time::timeout(HANG_GUARD, socket.next())
            .await
            .expect("timed out waiting for a data frame")
            .expect("trunk ended unexpectedly")
            .expect("trunk frame error");
        match msg {
            Message::Binary(bytes) => return bytes.to_vec(),
            Message::Text(text) => match Control::decode(&text) {
                Some(Control::Ping) | Some(Control::Pong) => continue,
                other => panic!("expected a data frame, got control {other:?}"),
            },
            Message::Ping(_) | Message::Pong(_) => continue,
            other => panic!("expected a binary data frame, got {other:?}"),
        }
    }
}

async fn connect_agent(addr: SocketAddr, hello: Control) -> (Socket, String, String, String) {
    let mut socket = tokio_tungstenite::connect_async(format!("ws://{addr}/t/_connect"))
        .await
        .unwrap()
        .0;
    socket
        .send(Message::Text(hello.encode().into()))
        .await
        .unwrap();
    match recv_control(&mut socket).await {
        Control::Welcome {
            id,
            public_url,
            resume_key,
            ..
        } => (socket, id, public_url, resume_key),
        other => panic!("expected welcome, got {other:?}"),
    }
}

/// A relay that accepts trunks without a token, as a local test relay does.
fn anonymous() -> Config {
    Config {
        allow_anonymous: true,
        ..Config::default()
    }
}

async fn expect_close(addr: SocketAddr, hello: Control) -> u16 {
    let mut socket = tokio_tungstenite::connect_async(format!("ws://{addr}/t/_connect"))
        .await
        .unwrap()
        .0;
    socket
        .send(Message::Text(hello.encode().into()))
        .await
        .unwrap();
    let msg = tokio::time::timeout(HANG_GUARD, socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    match msg {
        Message::Close(Some(frame)) => u16::from(frame.code),
        other => panic!("expected a close frame, got {other:?}"),
    }
}

fn plain_hello() -> Control {
    Control::Hello {
        token: None,
        resume: None,
        agent: "test-agent/0".into(),
    }
}

#[tokio::test]
async fn http_request_multiplexes_through_the_trunk() {
    let addr = spawn_service(anonymous()).await;
    let (mut trunk, id, public_url, _) = connect_agent(addr, plain_hello()).await;
    assert_eq!(public_url, format!("http://127.0.0.1:5167/t/{id}"));

    let client = reqwest::Client::new();
    let request = tokio::spawn({
        let url = format!("http://{addr}/t/{id}/about?foo=bar");
        async move { client.get(url).header("x-probe", "1").send().await.unwrap() }
    });

    let (ch, path, query, headers) = match recv_control(&mut trunk).await {
        Control::Open {
            ch,
            kind: ChannelKind::Http,
            method: Some(method),
            path,
            query,
            headers,
            ..
        } => {
            assert_eq!(method, "GET");
            (ch, path, query, headers)
        }
        other => panic!("expected http open, got {other:?}"),
    };
    assert_eq!(path, "/about");
    assert_eq!(query.as_deref(), Some("foo=bar"));
    assert!(headers.iter().any(|(k, v)| k == "x-probe" && v == "1"));
    assert_eq!(recv_control(&mut trunk).await, Control::End { ch });

    trunk
        .send(Message::Text(
            Control::OpenOk {
                ch,
                status: 200,
                headers: Some(vec![("content-type".into(), "application/json".into())]),
                subprotocol: None,
            }
            .encode()
            .into(),
        ))
        .await
        .unwrap();
    trunk
        .send(Message::Binary(encode_data(ch, true, b"{\"ok\":").into()))
        .await
        .unwrap();
    trunk
        .send(Message::Binary(encode_data(ch, true, b"true}").into()))
        .await
        .unwrap();
    trunk
        .send(Message::Text(Control::End { ch }.encode().into()))
        .await
        .unwrap();

    let response = request.await.unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "application/json"
    );
    assert_eq!(response.text().await.unwrap(), "{\"ok\":true}");
}

#[tokio::test]
async fn ws_channel_negotiates_subprotocol_and_preserves_frame_types() {
    let addr = spawn_service(anonymous()).await;
    let (mut trunk, id, _, _) = connect_agent(addr, plain_hello()).await;

    let mut request = format!("ws://{addr}/t/{id}/mini-comms/room-1")
        .into_client_request()
        .unwrap();
    request.headers_mut().insert(
        "Sec-WebSocket-Protocol",
        HeaderValue::from_static("rfc5, rfc4"),
    );
    let client = tokio::spawn(tokio_tungstenite::connect_async(request));

    let ch = match recv_control(&mut trunk).await {
        Control::Open {
            ch,
            kind: ChannelKind::Ws,
            path,
            subprotocols,
            ..
        } => {
            assert_eq!(path, "/mini-comms/room-1");
            assert_eq!(
                subprotocols,
                Some(vec!["rfc5".to_string(), "rfc4".to_string()])
            );
            ch
        }
        other => panic!("expected ws open, got {other:?}"),
    };
    trunk
        .send(Message::Text(
            Control::OpenOk {
                ch,
                status: 101,
                headers: None,
                subprotocol: Some("rfc5".into()),
            }
            .encode()
            .into(),
        ))
        .await
        .unwrap();

    let (mut socket, response) = client.await.unwrap().unwrap();
    assert_eq!(
        response.headers().get("sec-websocket-protocol").unwrap(),
        "rfc5"
    );

    socket
        .send(Message::Text("hello-text".to_string().into()))
        .await
        .unwrap();
    let frame = decode_data(&recv_binary(&mut trunk).await).unwrap();
    assert_eq!(frame.ch, ch);
    assert!(!frame.binary);
    assert_eq!(frame.payload, b"hello-text");

    socket
        .send(Message::Binary(vec![0u8, 1, 2, 255].into()))
        .await
        .unwrap();
    let frame = decode_data(&recv_binary(&mut trunk).await).unwrap();
    assert!(frame.binary);
    assert_eq!(frame.payload, vec![0u8, 1, 2, 255]);

    trunk
        .send(Message::Binary(
            encode_data(ch, false, b"{\"reply\":1}").into(),
        ))
        .await
        .unwrap();
    trunk
        .send(Message::Binary(encode_data(ch, true, &[9u8, 8, 7]).into()))
        .await
        .unwrap();
    match tokio::time::timeout(HANG_GUARD, socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap()
    {
        Message::Text(text) => assert_eq!(text.as_str(), "{\"reply\":1}"),
        other => panic!("expected text frame, got {other:?}"),
    }
    match tokio::time::timeout(HANG_GUARD, socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap()
    {
        Message::Binary(bytes) => assert_eq!(bytes.to_vec(), vec![9u8, 8, 7]),
        other => panic!("expected binary frame, got {other:?}"),
    }

    socket.close(None).await.unwrap();
    match recv_control(&mut trunk).await {
        Control::Close { ch: closed, .. } => assert_eq!(closed, ch),
        other => panic!("expected close, got {other:?}"),
    }
}

#[tokio::test]
async fn unknown_id_is_404_and_disconnected_agent_is_502() {
    let addr = spawn_service(anonymous()).await;
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://{addr}/t/zzzzzzzzzz/about"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);

    let (trunk, id, _, _) = connect_agent(addr, plain_hello()).await;
    close_trunk(trunk).await;
    let resp = client
        .get(format!("http://{addr}/t/{id}/about"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 502);
}

#[tokio::test]
async fn open_err_maps_to_502_with_the_agent_error() {
    let addr = spawn_service(anonymous()).await;
    let (mut trunk, id, _, _) = connect_agent(addr, plain_hello()).await;
    let client = reqwest::Client::new();
    let request = tokio::spawn({
        let url = format!("http://{addr}/t/{id}/about");
        async move { reqwest::get(url).await.unwrap() }
    });
    let _ = &client;
    let ch = match recv_control(&mut trunk).await {
        Control::Open { ch, .. } => ch,
        other => panic!("expected open, got {other:?}"),
    };
    assert_eq!(recv_control(&mut trunk).await, Control::End { ch });
    trunk
        .send(Message::Text(
            Control::OpenErr {
                ch,
                error: "connect refused".into(),
            }
            .encode()
            .into(),
        ))
        .await
        .unwrap();
    let resp = request.await.unwrap();
    assert_eq!(resp.status(), 502);
    assert!(resp.text().await.unwrap().contains("connect refused"));
}

#[tokio::test]
async fn token_mismatch_is_rejected_with_4401() {
    let cfg = Config {
        tokens: vec!["secret".into()],
        ..Config::default()
    };
    let addr = spawn_service(cfg).await;
    let mut socket = tokio_tungstenite::connect_async(format!("ws://{addr}/t/_connect"))
        .await
        .unwrap()
        .0;
    socket
        .send(Message::Text(
            Control::Hello {
                token: Some("wrong".into()),
                resume: None,
                agent: "test-agent/0".into(),
            }
            .encode()
            .into(),
        ))
        .await
        .unwrap();
    let msg = tokio::time::timeout(HANG_GUARD, socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    match msg {
        Message::Close(Some(frame)) => {
            assert_eq!(u16::from(frame.code), 4401);
        }
        other => panic!("expected close 4401, got {other:?}"),
    }

    let (_trunk, id, _, _) = connect_agent(
        addr,
        Control::Hello {
            token: Some("secret".into()),
            resume: None,
            agent: "test-agent/0".into(),
        },
    )
    .await;
    assert_eq!(id.len(), 10);
}

#[tokio::test]
async fn resume_with_key_keeps_the_public_id() {
    let cfg = Config {
        public_base_url: Some("https://tunnel.example".into()),
        ..anonymous()
    };
    let addr = spawn_service(cfg).await;
    let (trunk, id, public_url, resume_key) = connect_agent(addr, plain_hello()).await;
    assert_eq!(public_url, format!("https://tunnel.example/t/{id}"));
    close_trunk(trunk).await;

    let (_trunk2, id2, public_url2, _) = connect_agent(
        addr,
        Control::Hello {
            token: None,
            resume: Some(dcl_preview_tunnel::protocol::Resume {
                id: id.clone(),
                key: resume_key,
            }),
            agent: "test-agent/0".into(),
        },
    )
    .await;
    assert_eq!(id2, id);
    assert_eq!(public_url2, public_url);

    let (_trunk3, id3, _, _) = connect_agent(
        addr,
        Control::Hello {
            token: None,
            resume: Some(dcl_preview_tunnel::protocol::Resume {
                id: id.clone(),
                key: "wrong-key".into(),
            }),
            agent: "test-agent/0".into(),
        },
    )
    .await;
    assert_ne!(id3, id);
}

#[tokio::test]
async fn allow_ids_pins_the_public_url_pool() {
    let cfg = Config {
        allow_ids: vec!["stable-one".into()],
        ..anonymous()
    };
    let addr = spawn_service(cfg).await;
    let (_trunk, id, _, _) = connect_agent(addr, plain_hello()).await;
    assert_eq!(id, "stable-one");

    let mut socket = tokio_tungstenite::connect_async(format!("ws://{addr}/t/_connect"))
        .await
        .unwrap()
        .0;
    socket
        .send(Message::Text(plain_hello().encode().into()))
        .await
        .unwrap();
    let msg = tokio::time::timeout(HANG_GUARD, socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    match msg {
        Message::Close(Some(frame)) => assert_eq!(u16::from(frame.code), 4409),
        other => panic!("expected close 4409, got {other:?}"),
    }
}

#[tokio::test]
async fn a_relay_without_tokens_refuses_trunks_unless_opted_in() {
    let addr = spawn_service(Config::default()).await;
    assert_eq!(expect_close(addr, plain_hello()).await, 4401);
    let addr = spawn_service(anonymous()).await;
    let (_trunk, id, _, _) = connect_agent(addr, plain_hello()).await;
    assert_eq!(id.len(), 10);
}

#[tokio::test]
async fn the_agent_count_is_capped() {
    let addr = spawn_service(Config {
        max_agents: 1,
        ..anonymous()
    })
    .await;
    let (_trunk, _, _, _) = connect_agent(addr, plain_hello()).await;
    assert_eq!(expect_close(addr, plain_hello()).await, 4409);
}

#[tokio::test]
async fn an_id_in_its_reconnect_grace_still_counts_toward_the_agent_cap() {
    let addr = spawn_service(Config {
        max_agents: 1,
        ..anonymous()
    })
    .await;
    let (trunk, id, _, resume_key) = connect_agent(addr, plain_hello()).await;
    close_trunk(trunk).await;
    assert_eq!(expect_close(addr, plain_hello()).await, 4409);
    let (_trunk, resumed, _, _) = connect_agent(
        addr,
        Control::Hello {
            token: None,
            resume: Some(dcl_preview_tunnel::protocol::Resume {
                id: id.clone(),
                key: resume_key,
            }),
            agent: "test-agent/0".into(),
        },
    )
    .await;
    assert_eq!(resumed, id, "a resume is never refused by the cap");
}

#[tokio::test]
async fn an_allow_listed_id_in_grace_is_reused_at_the_agent_cap() {
    let addr = spawn_service(Config {
        max_agents: 1,
        allow_ids: vec!["stable-one".into()],
        ..anonymous()
    })
    .await;
    let (trunk, _, _, _) = connect_agent(addr, plain_hello()).await;
    close_trunk(trunk).await;
    let (_trunk, id, _, _) = connect_agent(addr, plain_hello()).await;
    assert_eq!(id, "stable-one");
}

/// Sends `request` through the tunnel and answers it with `headers`.
async fn proxied_response(
    trunk: &mut Socket,
    request: reqwest::RequestBuilder,
    headers: Vec<(String, String)>,
) -> (Vec<(String, String)>, reqwest::Response) {
    let pending = tokio::spawn(async move { request.send().await.unwrap() });
    let (ch, forwarded) = match recv_control(trunk).await {
        Control::Open { ch, headers, .. } => (ch, headers),
        other => panic!("expected http open, got {other:?}"),
    };
    assert_eq!(recv_control(trunk).await, Control::End { ch });
    trunk
        .send(Message::Text(
            Control::OpenOk {
                ch,
                status: 200,
                headers: Some(headers),
                subprotocol: None,
            }
            .encode()
            .into(),
        ))
        .await
        .unwrap();
    trunk
        .send(Message::Text(Control::End { ch }.encode().into()))
        .await
        .unwrap();
    (forwarded, pending.await.unwrap())
}

#[tokio::test]
async fn browser_credentials_for_the_relay_never_reach_an_agent() {
    let addr = spawn_service(anonymous()).await;
    let (mut trunk, id, _, _) = connect_agent(addr, plain_hello()).await;
    let client = reqwest::Client::new();
    let (forwarded, response) = proxied_response(
        &mut trunk,
        client
            .get(format!("http://{addr}/t/{id}/page"))
            .header("cookie", "session=example")
            .header("authorization", "Basic ZXhhbXBsZQ==")
            .header("x-probe", "1"),
        vec![],
    )
    .await;
    assert_eq!(response.status(), 200);
    assert!(forwarded.iter().any(|(k, _)| k == "x-probe"));
    assert!(
        !forwarded
            .iter()
            .any(|(k, _)| k == "cookie" || k == "authorization"),
        "{forwarded:?}"
    );
    let (forwarded, _) = proxied_response(
        &mut trunk,
        client
            .get(format!("http://{addr}/t/{id}/api"))
            .header("authorization", "Bearer example"),
        vec![],
    )
    .await;
    assert!(
        forwarded
            .iter()
            .any(|(k, v)| k == "authorization" && v == "Bearer example")
    );
}

#[tokio::test]
async fn origin_wide_response_headers_are_dropped() {
    let addr = spawn_service(anonymous()).await;
    let (mut trunk, id, _, _) = connect_agent(addr, plain_hello()).await;
    let blocked = [
        ("set-cookie", "session=planted; Path=/"),
        ("clear-site-data", "\"storage\""),
        ("nel", "{\"report_to\":\"x\",\"max_age\":1}"),
        ("report-to", "{\"group\":\"x\"}"),
        ("reporting-endpoints", "x=\"https://example.com\""),
        ("strict-transport-security", "max-age=1"),
        ("alt-svc", "clear"),
        ("accept-ch", "Sec-CH-UA"),
    ];
    let mut headers = vec![("content-type".to_string(), "text/html".to_string())];
    headers.extend(blocked.iter().map(|(k, v)| (k.to_string(), v.to_string())));
    let (_, response) = proxied_response(
        &mut trunk,
        reqwest::Client::new().get(format!("http://{addr}/t/{id}/")),
        headers,
    )
    .await;
    assert_eq!(response.headers().get("content-type").unwrap(), "text/html");
    for (name, _) in blocked {
        assert!(response.headers().get(name).is_none(), "{name}");
    }
}

#[tokio::test]
async fn service_workers_stay_scoped_to_one_tunnel() {
    let addr = spawn_service(anonymous()).await;
    let (mut trunk, id, _, _) = connect_agent(addr, plain_hello()).await;
    let client = reqwest::Client::new();
    let bare = client
        .get(format!("http://{addr}/t/{id}"))
        .header("service-worker", "script")
        .send()
        .await
        .unwrap();
    assert_eq!(bare.status(), 403);
    let (_, response) = proxied_response(
        &mut trunk,
        client
            .get(format!("http://{addr}/t/{id}/sw.js"))
            .header("service-worker", "script"),
        vec![
            ("content-type".into(), "text/javascript".into()),
            ("service-worker-allowed".into(), "/".into()),
        ],
    )
    .await;
    assert_eq!(response.status(), 200);
    assert!(response.headers().get("service-worker-allowed").is_none());
}

#[tokio::test]
async fn a_cancelled_request_frees_its_channel_slot() {
    let addr = spawn_service(Config {
        max_channels_per_agent: 1,
        ..anonymous()
    })
    .await;
    let (mut trunk, id, _, _) = connect_agent(addr, plain_hello()).await;
    let client = reqwest::Client::new();
    let pending = tokio::spawn({
        let client = client.clone();
        let url = format!("http://{addr}/t/{id}/slow");
        async move { client.get(url).send().await }
    });
    let ch = match recv_control(&mut trunk).await {
        Control::Open { ch, .. } => ch,
        other => panic!("expected http open, got {other:?}"),
    };
    assert_eq!(recv_control(&mut trunk).await, Control::End { ch });
    let busy = client
        .get(format!("http://{addr}/t/{id}/other"))
        .send()
        .await
        .unwrap();
    assert_eq!(busy.status(), 503, "one open channel is the agent's limit");

    pending.abort();
    let _ = pending.await;
    assert_eq!(
        recv_control(&mut trunk).await,
        Control::Close {
            ch,
            code: None,
            reason: None
        },
        "the agent hears that the client went away"
    );
    let next = tokio::spawn({
        let url = format!("http://{addr}/t/{id}/next");
        async move { client.get(url).send().await.unwrap() }
    });
    assert!(matches!(
        recv_control(&mut trunk).await,
        Control::Open { .. }
    ));
    next.abort();
}
