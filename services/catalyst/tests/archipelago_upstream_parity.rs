// from: catalyrst/crates/catalyrst-archipelago/tests/upstream_parity.rs
#[path = "support/archipelago.rs"]
mod archipelago;
use reqwest::Method;
use reqwest::header::ORIGIN;

async fn start_server() -> u16 {
    archipelago::serve(archipelago::state(archipelago::settings()).await).await
}

const ORIGIN_VALUE: &str = "https://play.decentraland.org";

#[tokio::test]
async fn cors_headers_on_every_route() {
    let port = start_server().await;
    let client = reqwest::Client::new();

    let routes = [
        "/status",
        "/ping",
        "/peers",
        "/comms/peers",
        "/peers/0xnope",
        "/islands",
        "/islands/nope",
        "/parcels",
        "/hot-scenes",
        "/core-status",
        "/stats/health",
        "/stats/parcels",
        "/stats/peers",
        "/stats/islands",
    ];
    for path in routes {
        let resp = client
            .get(format!("http://127.0.0.1:{port}{path}"))
            .header(ORIGIN, ORIGIN_VALUE)
            .send()
            .await
            .expect("request");
        assert_eq!(
            resp.headers()
                .get("access-control-allow-origin")
                .and_then(|v| v.to_str().ok()),
            Some("*"),
            "{path} allow-origin"
        );
        assert_eq!(
            resp.headers()
                .get("access-control-expose-headers")
                .and_then(|v| v.to_str().ok()),
            Some("*"),
            "{path} expose-headers"
        );
    }

    let preflight = client
        .request(Method::OPTIONS, format!("http://127.0.0.1:{port}/peers"))
        .header(ORIGIN, ORIGIN_VALUE)
        .header("access-control-request-method", "POST")
        .send()
        .await
        .expect("preflight");
    assert!(
        preflight.status().is_success(),
        "preflight status {}",
        preflight.status()
    );
    assert_eq!(
        preflight
            .headers()
            .get("access-control-allow-origin")
            .and_then(|v| v.to_str().ok()),
        Some("*"),
        "preflight allow-origin"
    );
    assert_eq!(
        preflight
            .headers()
            .get("access-control-allow-methods")
            .and_then(|v| v.to_str().ok()),
        Some("*"),
        "preflight allow-methods"
    );
    assert_eq!(
        preflight
            .headers()
            .get("access-control-allow-headers")
            .and_then(|v| v.to_str().ok()),
        Some("*"),
        "preflight allow-headers"
    );
}
