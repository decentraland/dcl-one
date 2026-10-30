//! The stream protocol and the server's security rules, against the server in this process.

mod support;

use benchmark::limits::Limits;
use support::{assert_isolation_headers, scratch, Request, TestServer};

#[tokio::test]
async fn the_stream_exists_only_under_this_runs_token() {
    let server = TestServer::start("token").await;
    let other = "0".repeat(32);

    for path in [
        format!("/__perf/{other}/csv"),
        format!("/__perf/{other}/end"),
        "/__perf/csv".to_string(),
        "/__perf/change_build/end".to_string(),
        "/__perf/".to_string(),
        format!("/__perf/{}", server.token()),
        format!("/__perf/{}/", server.token()),
        format!("/__perf/{}/csv/extra", server.token()),
        format!("/__perf/{}/log/", server.token()),
        format!("/__perf/{}/csv", &server.token()[1..]),
        format!("/__perf/{}0/csv", server.token()),
        format!("/__perf/{}/csv", server.token().to_uppercase()),
    ] {
        let reply = server
            .send("POST", &path, &[("X-Perf-Offset", "0")], b"x")
            .await;
        assert_eq!(reply.status, 404, "{path}");
        assert_isolation_headers(&reply);
    }

    assert!(!server.csv_path().exists(), "nothing was stored");
    assert_eq!(
        server.shared.access.refused(),
        11,
        "the refusals are counted"
    );
    server.stop().await;
}

#[tokio::test]
async fn a_known_token_with_an_unknown_route_is_not_found() {
    let server = TestServer::start("unknown").await;
    for (method, what) in [
        ("POST", "other"),
        ("GET", "other"),
        ("POST", "reset"),
        ("GET", "reset-done"),
    ] {
        let reply = server.send(method, &server.perf(what), &[], b"").await;
        assert_eq!(reply.status, 404, "{method} {what}");
    }
    for what in ["csv", "log", "end"] {
        for method in ["GET", "PUT", "DELETE"] {
            let reply = server.send(method, &server.perf(what), &[], b"").await;
            assert_eq!(reply.status, 405, "{method} {what}");
        }
    }
    server.stop().await;
}

#[tokio::test]
async fn only_loopback_hosts_with_the_bound_port_are_served() {
    let server = TestServer::start("host").await;
    std::fs::write(server.shared.root.join("index.html"), "app").unwrap();
    let port = server.port;

    for host in [
        format!("127.0.0.1:{port}"),
        format!("localhost:{port}"),
        format!("LOCALHOST:{port}"),
        format!("[::1]:{port}"),
    ] {
        let reply = Request::new("GET", "/")
            .header("Host", &host)
            .send(port)
            .await;
        assert_eq!(reply.status, 200, "{host}");
    }

    for host in [
        "evil.example".to_string(),
        format!("evil.example:{port}"),
        format!("localhost.evil.example:{port}"),
        format!("192.168.1.5:{port}"),
        format!("0.0.0.0:{port}"),
        "localhost".to_string(),
        format!("localhost:{}", port.wrapping_add(1)),
        String::new(),
    ] {
        for path in ["/".to_string(), server.perf("end"), server.perf("csv")] {
            let reply = Request::new("POST", &path)
                .header("Host", &host)
                .header("Content-Type", "application/json")
                .header("X-Perf-Offset", "0")
                .body(b"{\"exitCode\":0}")
                .send(port)
                .await;
            assert_eq!(reply.status, 421, "{host:?} {path}");
            assert_isolation_headers(&reply);
        }
    }

    // No Host at all, and two of them: nothing here says which site the request is for.
    let none = Request::new("GET", "/").send(port).await;
    assert_eq!(none.status, 421);
    let two = Request::new("GET", "/")
        .header("Host", &format!("127.0.0.1:{port}"))
        .header("Host", "evil.example")
        .send(port)
        .await;
    assert_eq!(two.status, 421);

    assert!(
        server.shared.end.get().is_none(),
        "no request from a refused Host ended the run"
    );
    assert!(!server.csv_path().exists());
    server.stop().await;
}

#[tokio::test]
async fn offsets_append_at_the_length_and_a_repeat_is_acknowledged_not_appended() {
    let server = TestServer::start("offsets").await;

    let first = server.post_chunk("csv", "0", b"abc").await;
    assert_eq!(first.status, 204, "the first chunk creates the file");
    assert_eq!(first.header("x-perf-length"), Some("3"));
    assert_eq!(
        server.post_chunk("csv", "0", b"abc").await.status,
        204,
        "a repeat"
    );
    assert_eq!(
        server.post_chunk("csv", "3", b"def").await.status,
        204,
        "the next chunk"
    );
    assert_eq!(
        server.post_chunk("csv", "4", b"efgh").await.status,
        204,
        "a chunk that overlaps what is stored appends only its tail"
    );
    let stored = server.post_chunk("csv", "8", b"").await;
    assert_eq!(
        (stored.status, stored.header("x-perf-length")),
        (204, Some("8"))
    );

    assert_eq!(
        server.post_chunk("csv", "10", b"x").await.status,
        409,
        "a gap"
    );
    assert_eq!(
        server.post_chunk("csv", "2", b"zz").await.status,
        409,
        "an overlap that differs"
    );
    assert_eq!(
        server.post_chunk("csv", "6", b"gZ").await.status,
        409,
        "an overlap that differs at its end"
    );
    assert_eq!(
        server.post_chunk("csv", "9", b"").await.status,
        409,
        "an empty chunk past the end"
    );

    assert_eq!(
        server
            .post_chunk("log", "0", &[b'o', b'k', b' ', 0xC3])
            .await
            .status,
        204
    );
    assert_eq!(
        server.post_chunk("log", "4", &[0xA9, b'!']).await.status,
        204,
        "the bytes of one character may arrive in two requests"
    );

    let refused = server.post_chunk("csv", "99", b"x").await;
    assert_eq!(
        refused.header("x-perf-length"),
        Some("8"),
        "a refusal says what is stored"
    );

    server.shared.csv.close().await;
    server.shared.log.close().await;
    assert_eq!(std::fs::read(server.csv_path()).unwrap(), b"abcdefgh");
    assert_eq!(
        std::fs::read(server.log_path()).unwrap(),
        "ok é!".as_bytes()
    );
    server.stop().await;
}

#[tokio::test]
async fn a_missing_or_bad_offset_is_refused() {
    let server = TestServer::start("bad-offset").await;
    let path = server.perf("csv");

    let missing = server.send("POST", &path, &[], b"q").await;
    assert_eq!(missing.status, 400);

    for bad in [
        "-1",
        "+1",
        "1.5",
        "",
        "abc",
        "0x10",
        "9223372036854775808",
        "1,2",
    ] {
        let reply = server
            .send("POST", &path, &[("X-Perf-Offset", bad)], b"q")
            .await;
        assert_eq!(reply.status, 400, "{bad:?}");
    }

    let twice = server
        .send(
            "POST",
            &path,
            &[("X-Perf-Offset", "0"), ("X-Perf-Offset", "0")],
            b"q",
        )
        .await;
    assert_eq!(twice.status, 400, "two offsets say nothing");

    assert!(!server.csv_path().exists());
    server.stop().await;
}

#[tokio::test]
async fn a_large_request_is_stored_in_order_whatever_the_pieces_it_arrives_in() {
    let server = TestServer::start("large").await;
    let bytes: Vec<u8> = (0..3_500_000u32).map(|n| (n % 251) as u8).collect();

    assert_eq!(
        server
            .post_chunk("log", "0", &bytes[..3_000_000])
            .await
            .status,
        204
    );
    // The retry of a request whose answer was lost: the stored part is compared as it arrives, the
    // rest appended.
    let overlapping = server
        .post_chunk("log", "1000000", &bytes[1_000_000..])
        .await;
    assert_eq!(overlapping.status, 204);
    assert_eq!(overlapping.header("x-perf-length"), Some("3500000"));

    let mut changed = bytes[2_900_000..3_100_000].to_vec();
    changed[150_000] ^= 1;
    assert_eq!(
        server.post_chunk("log", "2900000", &changed).await.status,
        409
    );

    server.shared.log.close().await;
    assert_eq!(std::fs::read(server.log_path()).unwrap(), bytes);
    server.stop().await;
}

#[tokio::test]
async fn a_body_cut_short_leaves_what_arrived_and_a_retry_completes_it() {
    let server = TestServer::start("short").await;
    let body = b"0123456789";

    let cut = Request::new("POST", &server.perf("log"))
        .header("Host", &server.host())
        .header("X-Perf-Offset", "0")
        .short_body(body, 10, 4)
        .send(server.port)
        .await;
    assert!([0, 400, 408].contains(&cut.status), "{}", cut.status);
    assert_eq!(server.shared.log.length().await, 4);

    let retry = server.post_chunk("log", "0", body).await;
    assert_eq!(
        (retry.status, retry.header("x-perf-length")),
        (204, Some("10"))
    );
    server.shared.log.close().await;
    assert_eq!(std::fs::read(server.log_path()).unwrap(), body);
    server.stop().await;
}

#[tokio::test]
async fn sizes_are_capped_before_a_byte_is_stored() {
    let limits = Limits {
        chunk_bytes: 16,
        control_bytes: 64,
        file_bytes: 24,
    };
    let dir = scratch("caps");
    let root = dir.join("root");
    std::fs::create_dir_all(&root).unwrap();
    let server = TestServer::start_with(dir, root, limits).await;

    let too_big = server.post_chunk("log", "0", &[b'x'; 17]).await;
    assert_eq!(too_big.status, 413, "a request holds at most chunk_bytes");
    assert!(
        !server.log_path().exists(),
        "refused on its declared length, before anything was stored"
    );

    let no_length = Request::new("POST", &server.perf("log"))
        .header("Host", &server.host())
        .header("X-Perf-Offset", "0")
        .chunked(b"abc")
        .send(server.port)
        .await;
    assert_eq!(no_length.status, 411, "the body needs a Content-Length");
    assert!(!server.log_path().exists());

    assert_eq!(server.post_chunk("log", "0", &[b'a'; 16]).await.status, 204);
    assert_eq!(server.post_chunk("log", "16", &[b'b'; 8]).await.status, 204);
    let over = server.post_chunk("log", "24", b"c").await;
    assert_eq!(over.status, 413, "a file holds at most file_bytes");
    assert_eq!(over.header("x-perf-length"), Some("24"));
    let straddling = server.post_chunk("log", "20", &[b'b'; 8]).await;
    assert_eq!(
        straddling.status, 413,
        "a request that reaches past the cap is refused whole"
    );
    assert_eq!(
        server.post_chunk("log", "16", &[b'b'; 8]).await.status,
        204,
        "a request that only repeats stored bytes is still acknowledged"
    );

    server.shared.log.close().await;
    let stored = std::fs::read(server.log_path()).unwrap();
    assert_eq!(stored.len(), 24);
    server.stop().await;
}

#[tokio::test]
async fn end_and_the_reset_routes_take_a_few_bytes_with_or_without_a_length() {
    let limits = Limits {
        chunk_bytes: 16,
        control_bytes: 64,
        file_bytes: 24,
    };
    let dir = scratch("control-caps");
    let root = dir.join("root");
    std::fs::create_dir_all(&root).unwrap();
    let server = TestServer::start_with(dir, root, limits).await;
    let big = vec![b' '; 1000];

    for what in ["end", "reset-done"] {
        let declared = Request::new("POST", &server.perf(what))
            .header("Host", &server.host())
            .header("Content-Type", "application/json")
            .body(&big)
            .send(server.port)
            .await;
        assert_eq!(declared.status, 413, "{what}, with a length");

        let chunked = Request::new("POST", &server.perf(what))
            .header("Host", &server.host())
            .header("Content-Type", "application/json")
            .chunked(&big)
            .send(server.port)
            .await;
        assert_eq!(chunked.status, 413, "{what}, with none");
    }

    assert!(server.shared.end.get().is_none());
    assert!(!server.shared.access.reset_reported());
    server.stop().await;
}

#[tokio::test]
async fn end_closes_the_run_with_the_pages_report_and_nothing_is_accepted_after() {
    let server = TestServer::start("end").await;
    assert_eq!(
        server.post_chunk("csv", "0", b"header\r\n").await.status,
        204
    );
    assert_eq!(server.post_chunk("log", "0", b"line\n").await.status, 204);

    let ended = server
        .post_end(r#"{"exitCode": 101, "csvBytes": 8, "logBytes": 5, "failed": true}"#)
        .await;
    assert_eq!(ended.status, 204);

    let report = server.shared.end.get().expect("the run has ended");
    assert_eq!(
        (
            report.exit_code,
            report.csv_bytes,
            report.log_bytes,
            report.failed
        ),
        (101, Some(8), Some(5), true)
    );

    assert_eq!(
        server.post_chunk("csv", "8", b"more").await.status,
        409,
        "after end"
    );
    assert_eq!(
        server.post_chunk("log", "0", b"line\n").await.status,
        409,
        "even a repeat"
    );
    let again = server.post_end(r#"{"exitCode": 0}"#).await;
    assert_eq!(again.status, 204);
    assert_eq!(
        server.shared.end.get().unwrap().exit_code,
        101,
        "the first end is the run's"
    );
    server.stop().await;
}

#[tokio::test]
async fn end_takes_json_of_the_right_shape_and_nothing_else() {
    let server = TestServer::start("end-shape").await;

    for content_type in [
        "text/plain",
        "application/x-www-form-urlencoded",
        "application/jsonp",
    ] {
        let reply = server
            .send(
                "POST",
                &server.perf("end"),
                &[("Content-Type", content_type)],
                br#"{"exitCode":0}"#,
            )
            .await;
        assert_eq!(
            reply.status, 415,
            "{content_type}: a cross-site form or no-cors fetch can send this"
        );
    }
    let none = server
        .send("POST", &server.perf("end"), &[], br#"{"exitCode":0}"#)
        .await;
    assert_eq!(none.status, 415, "no content type");

    for bad in [
        "{}",
        r#"{"exitCode": "0"}"#,
        r#"{"exitCode": 1.5}"#,
        r#"{"exitCode": 0, "csvBytes": "8"}"#,
        r#"{"exitCode": 0, "logBytes": -1}"#,
        r#"{"exitCode": 0, "csvBytes": 1.5}"#,
        r#"{"exitCode": 0, "failed": "no"}"#,
        "[0]",
        "not json",
        "",
    ] {
        let reply = server.post_end(bad).await;
        assert_eq!(reply.status, 400, "{bad:?}");
    }
    assert!(
        server.shared.end.get().is_none(),
        "none of them ended the run"
    );

    let with_charset = server
        .send(
            "POST",
            &server.perf("end"),
            &[("Content-Type", "application/json; charset=utf-8")],
            br#"{"exitCode": 0}"#,
        )
        .await;
    assert_eq!(with_charset.status, 204);
    let report = server.shared.end.get().unwrap();
    assert_eq!(
        (report.csv_bytes, report.log_bytes, report.failed),
        (None, None, false)
    );
    server.stop().await;
}

#[tokio::test]
async fn no_preflight_is_answered_so_a_cross_site_page_cannot_get_past_one() {
    let server = TestServer::start("preflight").await;
    let reply = server
        .send(
            "OPTIONS",
            &server.perf("end"),
            &[
                ("Origin", "https://evil.example"),
                ("Access-Control-Request-Method", "POST"),
                ("Access-Control-Request-Headers", "content-type"),
            ],
            b"",
        )
        .await;
    assert_eq!(reply.status, 405);
    assert!(reply.header("access-control-allow-origin").is_none());
    server.stop().await;
}

#[tokio::test]
async fn the_reset_page_is_served_and_its_report_is_recorded() {
    let server = TestServer::start("reset").await;

    let page = server.send("GET", &server.perf("reset"), &[], b"").await;
    assert_eq!(page.status, 200);
    assert_eq!(
        page.header("content-type"),
        Some("text/html; charset=utf-8")
    );
    assert!(page.text().contains("dcl-local-guest-root-key"));
    assert_isolation_headers(&page);

    assert!(!server.shared.access.reset_reported());
    let done = server
        .send(
            "POST",
            &server.perf("reset-done"),
            &[],
            b"serviceWorkers=1 localStorage=2 guestKey=kept",
        )
        .await;
    assert_eq!(done.status, 204);
    assert!(server.shared.access.reset_reported());
    server.stop().await;
}

fn site(server: &TestServer) -> std::path::PathBuf {
    let root = server.shared.root.clone();
    std::fs::create_dir_all(root.join("sub")).unwrap();
    std::fs::write(root.join("index.html"), "<html>home</html>").unwrap();
    std::fs::write(root.join("app.js"), "export default 1").unwrap();
    std::fs::write(root.join("engine_bg.wasm"), (0..=255u8).collect::<Vec<_>>()).unwrap();
    std::fs::write(root.join("sub").join("index.html"), "sub home").unwrap();
    std::fs::write(root.join("sub").join("note.txt"), "note").unwrap();
    std::fs::write(server.dir.join("secret.txt"), "outside the root").unwrap();
    root
}

#[tokio::test]
async fn files_are_served_with_the_types_the_web_needs_and_the_isolation_headers() {
    let server = TestServer::start("static").await;
    site(&server);

    let home = server.send("GET", "/", &[], b"").await;
    assert_eq!(home.status, 200);
    assert_eq!(home.text(), "<html>home</html>");
    assert_eq!(
        home.header("content-type"),
        Some("text/html; charset=utf-8")
    );
    assert_isolation_headers(&home);

    for (path, content_type, body) in [
        (
            "/app.js",
            "text/javascript; charset=utf-8",
            "export default 1",
        ),
        ("/sub/", "text/html; charset=utf-8", "sub home"),
        ("/sub/note.txt", "text/plain; charset=utf-8", "note"),
        ("/%73ub/note.txt", "text/plain; charset=utf-8", "note"),
    ] {
        let reply = server.send("GET", path, &[], b"").await;
        assert_eq!((reply.status, reply.text().as_str()), (200, body), "{path}");
        assert_eq!(reply.header("content-type"), Some(content_type), "{path}");
        assert_isolation_headers(&reply);
    }

    let wasm = server.send("GET", "/engine_bg.wasm", &[], b"").await;
    assert_eq!(wasm.header("content-type"), Some("application/wasm"));
    assert_eq!(wasm.body, (0..=255u8).collect::<Vec<_>>());

    let head = server.send("HEAD", "/app.js", &[], b"").await;
    assert_eq!(
        (head.status, head.header("content-length")),
        (200, Some("16"))
    );
    assert!(head.body.is_empty());

    for missing in ["/missing.js", "/sub", "/sub/missing/"] {
        let reply = server.send("GET", missing, &[], b"").await;
        assert_eq!(reply.status, 404, "{missing}");
        assert_isolation_headers(&reply);
    }
    for method in ["POST", "PUT", "DELETE"] {
        assert_eq!(
            server.send(method, "/app.js", &[], b"x").await.status,
            405,
            "{method}"
        );
    }
    server.stop().await;
}

#[tokio::test]
async fn every_file_the_app_does_not_have_is_answered_and_counted() {
    let server = TestServer::start("missing-count").await;
    for n in 0..12 {
        let reply = server
            .send("GET", &format!("/missing-{n}.js"), &[], b"")
            .await;
        assert_eq!(reply.status, 404, "{n}");
    }
    assert_eq!(server.shared.missing.total(), 12);
    server.stop().await;
}

#[tokio::test]
async fn ranges_are_served_as_the_browser_asks_for_them() {
    let server = TestServer::start("ranges").await;
    site(&server);
    let all: Vec<u8> = (0..=255u8).collect();

    let part = server
        .send("GET", "/engine_bg.wasm", &[("Range", "bytes=10-19")], b"")
        .await;
    assert_eq!(part.status, 206);
    assert_eq!(part.header("content-range"), Some("bytes 10-19/256"));
    assert_eq!(part.body, all[10..20]);

    let tail = server
        .send("GET", "/engine_bg.wasm", &[("Range", "bytes=250-")], b"")
        .await;
    assert_eq!((tail.status, tail.body), (206, all[250..].to_vec()));

    let last = server
        .send("GET", "/engine_bg.wasm", &[("Range", "bytes=-4")], b"")
        .await;
    assert_eq!((last.status, last.body), (206, all[252..].to_vec()));

    let past = server
        .send("GET", "/engine_bg.wasm", &[("Range", "bytes=300-400")], b"")
        .await;
    assert_eq!(past.status, 416);
    assert_eq!(past.header("content-range"), Some("bytes */256"));

    let many = server
        .send("GET", "/engine_bg.wasm", &[("Range", "bytes=0-1,5-6")], b"")
        .await;
    assert_eq!((many.status, many.body.len()), (200, 256));
    server.stop().await;
}

#[tokio::test]
async fn no_path_leaves_the_apps_folder() {
    let server = TestServer::start("traversal").await;
    site(&server);

    for path in [
        "/../secret.txt",
        "/sub/../../secret.txt",
        "/%2e%2e/secret.txt",
        "/%2E%2E%2Fsecret.txt",
        "/..%2fsecret.txt",
        "/..%5csecret.txt",
        "/..\\secret.txt",
        "//..//secret.txt",
        "/%2e%2e%5c%2e%2e%5csecret.txt",
        "/sub/%2e%2e/%2e%2e/secret.txt",
        "/%252e%252e/secret.txt",
        "/secret.txt%00.html",
        "/C:/Windows/win.ini",
        "/C%3A/Windows/win.ini",
        "/index.html::$DATA",
        "/index.html:stream",
        "/nul",
        "/CON.txt",
        "/sub/com1",
        "/%ff",
    ] {
        let reply = server.send("GET", path, &[], b"").await;
        assert!(
            reply.status == 400 || reply.status == 404,
            "{path}: status {}",
            reply.status
        );
        assert!(
            !reply.text().contains("outside the root"),
            "{path} served a file outside the root"
        );
        assert_isolation_headers(&reply);
    }
    server.stop().await;
}

#[tokio::test]
async fn the_server_listens_on_loopback_only() {
    let server = TestServer::start("loopback").await;
    // Every socket the server bound is a loopback one (bind takes no address to listen on).
    let listeners = benchmark::server::bind(0).await.unwrap();
    for address in listeners.addresses() {
        assert!(address.ip().is_loopback(), "{address}");
    }
    assert!(!listeners.addresses().is_empty());
    server.stop().await;
}
