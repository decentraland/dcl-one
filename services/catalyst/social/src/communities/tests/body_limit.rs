//! Real Hyper with serve's drain and trailing-slash normalization around the
//! actual community/RPC HTTP routes, compared to the configured source server.
use super::*;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

async fn response(
    socket: &mut BufReader<tokio::net::TcpStream>,
) -> (u16, axum::http::HeaderMap, String) {
    let mut line = String::new();
    socket.read_line(&mut line).await.unwrap();
    let status = line.split_whitespace().nth(1).unwrap().parse().unwrap();
    let mut headers = axum::http::HeaderMap::new();
    loop {
        line.clear();
        socket.read_line(&mut line).await.unwrap();
        if line.trim_end().is_empty() {
            break;
        }
        let (name, value) = line.split_once(':').unwrap();
        headers.append(
            axum::http::HeaderName::from_bytes(name.as_bytes()).unwrap(),
            value.trim().parse().unwrap(),
        );
    }
    let length = headers
        .get("content-length")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    let mut body = vec![0; length];
    socket.read_exact(&mut body).await.unwrap();
    (status, headers, String::from_utf8(body).unwrap())
}

async fn serve(rig: &Rig) -> (std::net::SocketAddr, tokio::task::JoinHandle<()>) {
    let rpc = Arc::new(crate::rpc::State::new(
        rig.pool.clone(),
        crate::rpc::Settings::default(),
        None,
        Arc::new(crate::rpc::voice::fake::FakeVoiceRooms::new(
            rig.pool.clone(),
        )),
    ));
    let gateway = crate::social_api_gateway_with_limit(
        router(rig.state.clone()).merge(crate::rpc::http::router(rpc)),
        rig.state.settings.http_max_request_body_bytes,
    );
    let app = Router::new().nest("/social-api", gateway).route(
        "/other-domain",
        axum::routing::post(|body: axum::body::Bytes| async move { body.len().to_string() }),
    );
    // serve uses the maximum of content's200MiB and worlds'360MiB upload caps.
    // Preserve the actual outer drain in both configured-cap regressions.
    let app = crate::shared::drain::drain_unread(app, 360 * 1024 * 1024);
    let app = tower_http::normalize_path::NormalizePath::trim_trailing_slash(app);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            axum::ServiceExt::<axum::extract::Request>::into_make_service(app),
        )
        .await
        .unwrap()
    });
    (addr, server)
}

#[tokio::test]
async fn real_http_body_precedence_and_all_body_readers_match_configured_source() {
    let fixture:Value=serde_json::from_str(include_str!(concat!(env!("DCL_ONE_ROOT"),"/services/catalyst/tests/fixtures/conformance/social/source-contracts/community-body-limit.json"))).unwrap();
    let mut settings = Settings::for_tests();
    settings.admin_token = Some("fixture-admin".into());
    settings.http_max_request_body_bytes = fixture["cap"].as_u64().unwrap() as usize;
    let Some(rig) = Rig::new(settings).await else {
        return;
    };
    let (addr, server) = serve(&rig).await;
    let actor = wallet();
    let missing = CommunityId::new().to_string();
    for (index, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        let source_path = case["path"]
            .as_str()
            .unwrap()
            .replace("00000000-0000-4000-8000-000000000001", &missing);
        let path = format!("/social-api{source_path}");
        let method = case["method"].as_str().unwrap();
        let mut head = format!(
            "{method} {path} HTTP/1.1\r\nHost: social.test\r\nConnection: keep-alive\r\nAccess-Control-Request-Headers: authorization,x-test\r\n"
        );
        if case["noOrigin"] != true {
            head.push_str("Origin: https://decentraland.org\r\n");
        }
        if case["signed"] == true {
            for (name, value) in signed_headers(
                &actor,
                method.to_lowercase().as_str(),
                path.split('?').next().unwrap(),
            )
            .await
            {
                head.push_str(&format!("{name}: {value}\r\n"));
            }
            head.push_str(if method == "POST" {
                "Content-Type: application/x-www-form-urlencoded\r\n"
            } else {
                "Content-Type: application/json\r\n"
            });
        }
        if case["bearer"] == true {
            head.push_str("Authorization: Bearer fixture-admin\r\n");
        }
        let data = if let Some(length) = case["length"].as_u64() {
            head.push_str(&format!("Content-Length: {length}\r\n\r\n"));
            if case["sendBody"] == false {
                Vec::new()
            } else {
                vec![b'a'; length as usize]
            }
        } else {
            let length = case["chunked"].as_u64().unwrap() as usize;
            head.push_str("Transfer-Encoding: chunked\r\n\r\n");
            format!("{length:x}\r\n{}\r\n0\r\n\r\n", "a".repeat(length)).into_bytes()
        };
        let (status, headers, body) =
            tokio::time::timeout(std::time::Duration::from_secs(5), async {
                let mut socket =
                    BufReader::new(tokio::net::TcpStream::connect(addr).await.unwrap());
                socket.get_mut().write_all(head.as_bytes()).await.unwrap();
                socket.get_mut().write_all(&data).await.unwrap();
                response(&mut socket).await
            })
            .await
            .unwrap();
        assert_eq!(
            status,
            case["status"].as_u64().unwrap() as u16,
            "case{index}: {method} {source_path}: {body}"
        );
        let expected = case["body"]
            .as_str()
            .unwrap()
            .replace("00000000-0000-4000-8000-000000000001", &missing);
        if let Ok(expected) = serde_json::from_str::<Value>(&expected) {
            assert_eq!(
                serde_json::from_str::<Value>(&body).unwrap(),
                expected,
                "case{index}: {method} {source_path}"
            );
        } else {
            assert_eq!(body, expected, "case{index}: {method} {source_path}");
        }
        assert_eq!(
            super::social_api_parity::application_headers(&headers),
            super::social_api_parity::source_application_headers(&case["headers"]),
            "case{index}: {method} {source_path} headers"
        );
        if status == 413 || case["signed"] == true && case["chunked"].as_u64().is_some() {
            assert_eq!(
                headers.get("connection").unwrap(),
                "close",
                "rejected upload must close"
            );
        }
    }
    // The social cap does not limit another domain behind the same drain layer.
    let mut socket = BufReader::new(tokio::net::TcpStream::connect(addr).await.unwrap());
    socket
        .get_mut()
        .write_all(
            format!(
                "POST /other-domain HTTP/1.1\r\nHost: social.test\r\nContent-Length: 65\r\n\r\n{}",
                "a".repeat(65)
            )
            .as_bytes(),
        )
        .await
        .unwrap();
    let (status, _, body) = response(&mut socket).await;
    assert_eq!((status, body), (200, "65".to_owned()));
    server.abort();
    rig.finish();
}

#[tokio::test]
async fn larger_http_cap_preserves_independent_multipart_and_thumbnail_limits() {
    let source:Value=serde_json::from_str(include_str!(concat!(env!("DCL_ONE_ROOT"),"/services/catalyst/tests/fixtures/conformance/social/source-contracts/community-body-limit.json"))).unwrap();
    let source = &source["multipart"];
    let mut settings = Settings::for_tests();
    settings.http_max_request_body_bytes = source["cap"].as_u64().unwrap() as usize;
    let Some(rig) = Rig::new(settings).await else {
        return;
    };
    let (addr, server) = serve(&rig).await;
    let actor = wallet();
    for case in source["cases"].as_array().unwrap() {
        let image = png(case["thumbnailBytes"].as_u64().unwrap() as usize);
        let padding = vec![b'x'; case["paddingBytes"].as_u64().unwrap() as usize];
        let (wire, content_type) = multipart(&[
            ("name", b"Fixture"),
            ("description", b"Fixture"),
            ("thumbnail", &image),
            ("unused", &padding),
        ]);
        assert_eq!(case["nativeParsedThumbnailBytes"], json!(image.len()));
        if case["paddingBytes"].as_u64().unwrap() > 0 {
            assert!(wire.len() > source["bodyCap"].as_u64().unwrap() as usize);
            assert!(image.len() <= source["thumbnailMax"].as_u64().unwrap() as usize);
        }
        let mut head = format!(
            "POST /social-api/v1/communities HTTP/1.1\r\nHost: social.test\r\nConnection: keep-alive\r\nOrigin: https://decentraland.org\r\nContent-Type: {content_type}\r\n"
        );
        for (name, value) in signed_headers(&actor, "post", "/social-api/v1/communities").await {
            head.push_str(&format!("{name}: {value}\r\n"));
        }
        let body = if case["chunked"] == true {
            head.push_str("Transfer-Encoding: chunked\r\n\r\n");
            let mut body = format!("{:x}\r\n", wire.len()).into_bytes();
            body.extend_from_slice(&wire);
            body.extend_from_slice(b"\r\n0\r\n\r\n");
            body
        } else {
            head.push_str(&format!("Content-Length: {}\r\n\r\n", wire.len()));
            if case["sendBody"] == false {
                Vec::new()
            } else {
                wire
            }
        };
        let (status, headers, body) =
            tokio::time::timeout(std::time::Duration::from_secs(5), async {
                let mut socket =
                    BufReader::new(tokio::net::TcpStream::connect(addr).await.unwrap());
                socket.get_mut().write_all(head.as_bytes()).await.unwrap();
                socket.get_mut().write_all(&body).await.unwrap();
                response(&mut socket).await
            })
            .await
            .unwrap();
        assert_eq!(
            status,
            case["status"].as_u64().unwrap() as u16,
            "{} {body}",
            case["name"]
        );
        assert_eq!(
            serde_json::from_str::<Value>(&body).unwrap(),
            serde_json::from_str::<Value>(case["body"].as_str().unwrap()).unwrap(),
            "{}",
            case["name"]
        );
        assert_eq!(
            super::social_api_parity::application_headers(&headers),
            super::social_api_parity::source_application_headers(&case["headers"]),
            "{} headers",
            case["name"]
        );
        assert_ne!(
            headers
                .get("connection")
                .and_then(|value| value.to_str().ok()),
            Some("close"),
            "the independent form limit is below the global stream cap"
        );
    }
    server.abort();
    rig.finish();
}
