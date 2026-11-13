//! The deployment of a CLI run as a resource, in the shape `@dcl/sdk-commands`
//! serves so a program driving the deploy (the Creator Hub, a script) needs
//! no page: `POST /api/deploy` signs and starts, `GET` follows it (JSON, or
//! server-sent events), `DELETE` cancels it with the run's deploy token.
//! `/api/info` and `/api/files` describe what is about to be signed.

use super::AppState;
use super::deploy_page::{cli_signer, signing_blocked};
use crate::linker::{self, CancelOutcome, DeploymentState, LinkerState, SignOutcome, Signed};
use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{ConnectInfo, State};
use axum::http::{HeaderMap, HeaderName, StatusCode, header};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use dcl_scene_authoring::jsjson::JsValue;
use futures::Stream;
use serde_json::{Value, json};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub(crate) const RESOURCE_PATH: &str = "/api/deploy";
pub(crate) const TOKEN_HEADER: &str = "x-deploy-token";
const HEARTBEAT: Duration = Duration::from_secs(15);
/// Progress events come at most this close; a phase change goes out at once.
const PROGRESS_THROTTLE: Duration = Duration::from_millis(250);

fn message(status: StatusCode, text: impl Into<String>) -> Response {
    (status, Json(json!({ "message": text.into() }))).into_response()
}

fn state_json(status: StatusCode, signer: &LinkerState) -> Response {
    (status, Json(signer.deployment_state())).into_response()
}

/// The answer while this server signs for no run.
fn no_pending() -> Response {
    message(
        StatusCode::NOT_FOUND,
        "no deployment is pending on this server",
    )
}

fn header_text<'a>(headers: &'a HeaderMap, name: &str) -> &'a str {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
}

/// `Prefer: respond-async`, among any other preferences and parameters.
pub(crate) fn prefers_async(headers: &HeaderMap) -> bool {
    header_text(headers, "prefer").split(',').any(|preference| {
        preference
            .split(['=', ';'])
            .next()
            .unwrap_or_default()
            .trim()
            .eq_ignore_ascii_case("respond-async")
    })
}

pub(crate) fn accepts_event_stream(headers: &HeaderMap) -> bool {
    header_text(headers, "accept")
        .to_ascii_lowercase()
        .contains("text/event-stream")
}

fn is_true(v: Option<&JsValue>) -> bool {
    matches!(v, Some(JsValue::Bool(true)))
}

/// `GET /api/info`: what the signing page shows, for a program. `rootCID`
/// is the entity id to sign; a world overwrite also signs
/// `deleteScenesFromWorldPayload`.
pub(super) async fn info(State(st): State<Arc<AppState>>) -> Response {
    let Some(signer) = cli_signer(&st) else {
        return no_pending();
    };
    let (root_cid, delete_payload) = match signer.minted() {
        Ok(minted) => minted,
        Err(e) => {
            return message(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("could not build the entity: {e:#}"),
            );
        }
    };
    let dep = signer.dep();
    let metadata = &dep.prepared.metadata;
    Json(json!({
        "baseParcel": dep.base_parcel,
        "parcels": dep.prepared.pointers,
        "rootCID": root_cid,
        "title": dep.scene_title,
        "description": metadata
            .get("display")
            .and_then(|d| d.get("description"))
            .and_then(|d| d.as_str()),
        "skipValidations": dep.skip_validations,
        "isPortableExperience": is_true(metadata.get("isPortableExperience")),
        "isWorld": dep.world.is_some(),
        "world": dep.world,
        "deleteScenesFromWorldPayload": delete_payload,
        "targetContent": dep.target_content,
        "multiScene": dep.multi_scene,
        "debug": false,
        "landRegistry": Value::Null,
        "estateRegistry": Value::Null,
    }))
    .into_response()
}

/// `GET /api/files`: the payload, name and size.
pub(super) async fn files(State(st): State<Arc<AppState>>) -> Response {
    let Some(signer) = cli_signer(&st) else {
        return no_pending();
    };
    let files: Vec<Value> = signer
        .dep()
        .prepared
        .files
        .iter()
        .map(|(name, _, bytes)| json!({ "name": name, "size": bytes.len() }))
        .collect();
    Json(files).into_response()
}

/// `GET /api/deploy`: the state as JSON, or every change as server-sent
/// `state` events with `Accept: text/event-stream`. The stream ends after a
/// final state, so an EventSource must close() then or it reconnects.
pub(super) async fn get(State(st): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    let Some(signer) = cli_signer(&st) else {
        return no_pending();
    };
    signer.follow();
    if !accepts_event_stream(&headers) {
        return state_json(StatusCode::OK, &signer);
    }
    Sse::new(events(signer))
        .keep_alive(KeepAlive::new().interval(HEARTBEAT).text("ping"))
        .into_response()
}

/// Every change of the run, the final state last and the stream done after
/// it. Progress-only changes are coalesced to one per [`PROGRESS_THROTTLE`].
pub(crate) fn follow(signer: Arc<LinkerState>) -> impl Stream<Item = DeploymentState> {
    struct Follow {
        signer: Arc<LinkerState>,
        changes: tokio::sync::watch::Receiver<u64>,
        last: Option<DeploymentState>,
        emitted: Instant,
        done: bool,
    }
    let changes = signer.changes();
    let start = Follow {
        signer,
        changes,
        last: None,
        emitted: Instant::now(),
        done: false,
    };
    futures::stream::unfold(start, |mut f| async move {
        if f.done {
            return None;
        }
        loop {
            let now = f.signer.deployment_state();
            if f.last.as_ref() == Some(&now) {
                if f.changes.changed().await.is_err() {
                    return None;
                }
                continue;
            }
            let same_phase = f.last.as_ref().is_some_and(|l| l.phase == now.phase);
            if same_phase
                && let Some(wait) = PROGRESS_THROTTLE.checked_sub(f.emitted.elapsed())
                && !wait.is_zero()
            {
                tokio::time::sleep(wait).await;
                continue;
            }
            f.done = now.is_final();
            f.emitted = Instant::now();
            f.last = Some(now.clone());
            return Some((now, f));
        }
    })
}

fn events(signer: Arc<LinkerState>) -> impl Stream<Item = Result<Event, Infallible>> {
    use futures::StreamExt;
    follow(signer).map(|state| {
        Ok(Event::default()
            .event("state")
            .data(serde_json::to_string(&state).unwrap_or_default()))
    })
}

/// The payload upstream's linker sends: `{ address, authChain, chainId?,
/// deleteSignature? }`. The entity id is the last link's payload.
fn signed_from(value: &Value) -> Option<Signed> {
    let address = value.get("address")?.as_str().filter(|a| !a.is_empty())?;
    let chain = value
        .get("authChain")?
        .as_array()
        .filter(|c| !c.is_empty())?;
    let entity_id = chain
        .last()?
        .get("payload")?
        .as_str()
        .filter(|p| !p.is_empty())?;
    Some(Signed {
        address: address.to_string(),
        entity_id: entity_id.to_string(),
        auth_chain: Value::Array(chain.clone()),
        delete_signature: value
            .get("deleteSignature")
            .and_then(|d| d.as_str())
            .map(str::to_string),
    })
}

fn refusal(outcome: SignOutcome, signer: &LinkerState) -> Response {
    match outcome {
        SignOutcome::Published(_) => state_json(StatusCode::OK, signer),
        SignOutcome::Stale => message(
            StatusCode::BAD_REQUEST,
            "unknown or stale entity id: GET /api/info for the current rootCID and sign that",
        ),
        SignOutcome::Retry(why) => message(StatusCode::BAD_REQUEST, why),
        SignOutcome::Fatal(why) => message(StatusCode::BAD_REQUEST, why),
        SignOutcome::Refused => state_json(StatusCode::CONFLICT, signer),
    }
}

/// `POST /api/deploy`: sign and start. With `Prefer: respond-async` it
/// answers 202 at once and the run is followed through `GET`; otherwise it
/// answers when the run ends. A payload that cannot run is refused before
/// either, so a 202 always means the upload is under way.
pub(super) async fn post(
    State(st): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    body: Result<Json<Value>, JsonRejection>,
) -> Response {
    if let Some(blocked) = signing_blocked(&st, peer, &headers) {
        return blocked;
    }
    let Some(signer) = cli_signer(&st) else {
        return no_pending();
    };
    let Ok(Json(value)) = body else {
        return message(
            StatusCode::BAD_REQUEST,
            "Invalid payload: the body must be JSON",
        );
    };
    if !signer.can_start() {
        return state_json(StatusCode::CONFLICT, &signer);
    }
    let Some(signed) = signed_from(&value) else {
        let keys = value
            .as_object()
            .map(|o| o.keys().cloned().collect::<Vec<_>>().join(" - "))
            .unwrap_or_default();
        tracing::error!("invalid deploy payload: {keys}");
        return message(StatusCode::BAD_REQUEST, format!("Invalid payload: {keys}"));
    };
    if let Some(stopped) = linker::precheck(&signer, &signed) {
        return refusal(stopped, &signer);
    }
    if prefers_async(&headers) {
        signer.follow();
        let run = signer.clone();
        tokio::spawn(async move {
            linker::run_signed(&run, signed).await;
        });
        return (
            StatusCode::ACCEPTED,
            [
                (header::LOCATION, RESOURCE_PATH),
                (
                    HeaderName::from_static("preference-applied"),
                    "respond-async",
                ),
            ],
            Json(signer.deployment_state()),
        )
            .into_response();
    }
    let outcome = linker::run_signed(&signer, signed).await;
    refusal(outcome, &signer)
}

/// `DELETE /api/deploy`: cancel, with the run's token in `X-Deploy-Token`.
/// 409 while the world's scenes are being removed, or once the run ended.
pub(super) async fn cancel(State(st): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    if let Some(why) = super::cross_origin_refusal(&headers) {
        return message(StatusCode::FORBIDDEN, why);
    }
    let Some(signer) = cli_signer(&st) else {
        return no_pending();
    };
    if !linker::token_matches(header_text(&headers, TOKEN_HEADER), signer.deploy_token()) {
        return message(StatusCode::FORBIDDEN, "Invalid or missing deploy token");
    }
    match signer.cancel() {
        CancelOutcome::Final | CancelOutcome::Refused => state_json(StatusCode::CONFLICT, &signer),
        CancelOutcome::Cancelled => {
            tracing::warn!("Deployment cancelled.");
            state_json(StatusCode::ACCEPTED, &signer)
        }
        CancelOutcome::Cancelling => {
            tracing::warn!("Cancelling the deployment...");
            state_json(StatusCode::ACCEPTED, &signer)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::start::deploy_page::adopt_cli_signing;
    use crate::start::testkit;
    use axum::body::to_bytes;
    use futures::StreamExt;

    const TOKEN: &str = "0123456789abcdef";
    const LOCAL: SocketAddr =
        SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), 4000);
    const LAN: SocketAddr = SocketAddr::new(
        std::net::IpAddr::V4(std::net::Ipv4Addr::new(10, 0, 0, 7)),
        4000,
    );

    fn signer(world: Option<&str>) -> Arc<LinkerState> {
        let dep = crate::linker::LinkerDeploy {
            dir: std::env::temp_dir(),
            prepared: crate::deploy::Prepared {
                files: vec![("scene.json".into(), "h1".into(), b"{}".to_vec())],
                pointers: vec!["0,0".into()],
                metadata: dcl_scene_authoring::jsjson::parse(
                    r#"{"display":{"title":"Gather","description":"a square"},"isPortableExperience":false}"#,
                )
                .unwrap(),
            },
            target_content: "http://127.0.0.1:9".into(),
            world: world.map(str::to_string),
            needs_delete: false,
            timestamp_override: None,
            entity_out: None,
            scene_title: "Gather".into(),
            base_parcel: "0,0".into(),
            multi_scene: true,
            gate: crate::deploy::PermissionGate::off(),
            deploy_token: TOKEN.into(),
            skip_validations: false,
        };
        crate::linker::new_state(dep).0
    }

    fn app(tag: &str) -> (testkit::Tmp, Arc<AppState>) {
        let tmp = testkit::Tmp::new(tag);
        let project = testkit::scene(&tmp.0, "gather", &["0,0"], "module.exports={}");
        (tmp, Arc::new(testkit::state(vec![project])))
    }

    async fn json_of(resp: Response) -> Value {
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    }

    fn with(pairs: &[(&'static str, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            h.insert(*k, v.parse().unwrap());
        }
        h
    }

    fn chain_for(entity_id: &str) -> Value {
        json!({
            "address": "0x0000000000000000000000000000000000000001",
            "chainId": 1,
            "authChain": [
                { "type": "SIGNER", "payload": "0x0000000000000000000000000000000000000001", "signature": "" },
                { "type": "ECDSA_SIGNED_ENTITY", "payload": entity_id, "signature": "0xdead" }
            ]
        })
    }

    #[tokio::test]
    async fn without_a_pending_run_every_route_is_a_404() {
        let (_tmp, st) = app("api-none");
        for resp in [
            get(State(st.clone()), HeaderMap::new()).await,
            info(State(st.clone())).await,
            files(State(st.clone())).await,
            cancel(State(st.clone()), with(&[(TOKEN_HEADER, TOKEN)])).await,
            post(
                State(st.clone()),
                ConnectInfo(LOCAL),
                HeaderMap::new(),
                Ok(Json(json!({}))),
            )
            .await,
        ] {
            assert_eq!(resp.status(), StatusCode::NOT_FOUND);
            assert!(
                json_of(resp).await["message"]
                    .as_str()
                    .unwrap()
                    .contains("no deployment")
            );
        }
    }

    #[tokio::test]
    async fn the_resource_tells_the_run_and_what_to_sign() {
        let (_tmp, st) = app("api-info");
        adopt_cli_signing(&st, signer(Some("w.dcl.eth")));

        let state = json_of(get(State(st.clone()), HeaderMap::new()).await).await;
        assert_eq!(state["phase"], "awaiting-signature");
        assert_eq!(state["cancelRequested"], false);
        assert!(state["upload"].is_null() && state["result"].is_null() && state["error"].is_null());

        let first = json_of(info(State(st.clone())).await).await;
        let again = json_of(info(State(st.clone())).await).await;
        assert_eq!(
            first["rootCID"], again["rootCID"],
            "one id while it is fresh"
        );
        assert!(
            first["rootCID"].as_str().unwrap().starts_with("bafkrei"),
            "{first}"
        );
        assert_eq!(first["isWorld"], true);
        assert_eq!(first["world"], "w.dcl.eth");
        assert_eq!(first["title"], "Gather");
        assert_eq!(first["description"], "a square");
        assert_eq!(first["parcels"], json!(["0,0"]));
        assert_eq!(first["baseParcel"], "0,0");
        assert_eq!(first["targetContent"], "http://127.0.0.1:9");
        assert_eq!(first["multiScene"], true);
        assert_eq!(first["skipValidations"], false);
        assert_eq!(first["isPortableExperience"], false);
        assert!(
            first["deleteScenesFromWorldPayload"].is_null(),
            "additive: nothing removed"
        );

        let files = json_of(files(State(st.clone())).await).await;
        assert_eq!(files, json!([{ "name": "scene.json", "size": 2 }]));
    }

    #[tokio::test]
    async fn a_payload_is_checked_before_anything_runs() {
        let (_tmp, st) = app("api-post");
        let signer = signer(None);
        adopt_cli_signing(&st, signer.clone());

        let lan = post(
            State(st.clone()),
            ConnectInfo(LAN),
            HeaderMap::new(),
            Ok(Json(json!({}))),
        )
        .await;
        assert_eq!(
            lan.status(),
            StatusCode::FORBIDDEN,
            "signing happens on this machine"
        );

        let not_object = post(
            State(st.clone()),
            ConnectInfo(LOCAL),
            HeaderMap::new(),
            Ok(Json(json!("x"))),
        )
        .await;
        assert_eq!(not_object.status(), StatusCode::BAD_REQUEST);
        let partial = post(
            State(st.clone()),
            ConnectInfo(LOCAL),
            HeaderMap::new(),
            Ok(Json(json!({ "address": "0x1", "chainId": 1 }))),
        )
        .await;
        assert_eq!(partial.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            json_of(partial).await["message"],
            "Invalid payload: address - chainId"
        );

        for headers in [HeaderMap::new(), with(&[("prefer", "respond-async")])] {
            let stale = post(
                State(st.clone()),
                ConnectInfo(LOCAL),
                headers,
                Ok(Json(chain_for("bogus"))),
            )
            .await;
            assert_eq!(
                stale.status(),
                StatusCode::BAD_REQUEST,
                "a stale id never earns a 202"
            );
            assert!(
                json_of(stale).await["message"]
                    .as_str()
                    .unwrap()
                    .contains("rootCID")
            );
        }
        assert!(signer.can_start(), "nothing ran");
        assert_eq!(signer.deployment_state().phase, "awaiting-signature");
    }

    #[tokio::test]
    async fn a_cancel_needs_the_token_and_ends_the_run() {
        let (_tmp, st) = app("api-cancel");
        let signer = signer(None);
        adopt_cli_signing(&st, signer.clone());

        let missing = cancel(State(st.clone()), HeaderMap::new()).await;
        assert_eq!(missing.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            json_of(missing).await["message"],
            "Invalid or missing deploy token"
        );
        let wrong = cancel(
            State(st.clone()),
            with(&[(TOKEN_HEADER, "0123456789abcdeF")]),
        )
        .await;
        assert_eq!(wrong.status(), StatusCode::FORBIDDEN);
        let foreign = cancel(
            State(st.clone()),
            with(&[
                (TOKEN_HEADER, TOKEN),
                ("origin", "https://evil.example"),
                ("host", "localhost:8000"),
            ]),
        )
        .await;
        assert_eq!(
            foreign.status(),
            StatusCode::FORBIDDEN,
            "a page elsewhere cannot cancel"
        );
        assert_eq!(signer.deployment_state().phase, "awaiting-signature");

        let done = cancel(State(st.clone()), with(&[(TOKEN_HEADER, TOKEN)])).await;
        assert_eq!(done.status(), StatusCode::ACCEPTED);
        let body = json_of(done).await;
        assert_eq!(body["phase"], "cancelled");
        assert_eq!(body["cancelRequested"], true);
        assert!(body["error"].is_null());

        let after = json_of(get(State(st.clone()), HeaderMap::new()).await).await;
        assert_eq!(after["phase"], "cancelled");
        let again = cancel(State(st.clone()), with(&[(TOKEN_HEADER, TOKEN)])).await;
        assert_eq!(
            again.status(),
            StatusCode::CONFLICT,
            "final states answer 409 with the state"
        );
        assert_eq!(json_of(again).await["phase"], "cancelled");
        let id = signer.minted().unwrap().0;
        let late = post(
            State(st.clone()),
            ConnectInfo(LOCAL),
            HeaderMap::new(),
            Ok(Json(chain_for(&id))),
        )
        .await;
        assert_eq!(
            late.status(),
            StatusCode::CONFLICT,
            "nothing starts after a cancel"
        );
    }

    #[tokio::test]
    async fn the_event_stream_ends_after_the_final_state() {
        let signer = signer(None);
        let mut states = Box::pin(follow(signer.clone()));
        let first = states.next().await.expect("the current state comes first");
        assert_eq!(first.phase, "awaiting-signature");
        assert_eq!(signer.cancel(), CancelOutcome::Cancelled);
        let last = tokio::time::timeout(Duration::from_secs(5), states.next())
            .await
            .expect("the change wakes the stream")
            .expect("the final state is sent");
        assert_eq!(last.phase, "cancelled");
        assert!(last.is_final());
        assert!(
            tokio::time::timeout(Duration::from_secs(5), states.next())
                .await
                .unwrap()
                .is_none(),
            "the stream ends after a final state"
        );
    }

    #[test]
    fn request_headers_are_read_as_upstream_reads_them() {
        assert!(prefers_async(&with(&[("prefer", "respond-async")])));
        assert!(prefers_async(&with(&[(
            "prefer",
            "wait=10, Respond-Async; foo=bar"
        )])));
        assert!(!prefers_async(&with(&[("prefer", "wait=10")])));
        assert!(!prefers_async(&HeaderMap::new()));
        assert!(accepts_event_stream(&with(&[(
            "accept",
            "text/event-stream"
        )])));
        assert!(accepts_event_stream(&with(&[(
            "accept",
            "Text/Event-Stream, */*"
        )])));
        assert!(!accepts_event_stream(&with(&[(
            "accept",
            "application/json"
        )])));
        assert!(signed_from(&json!({ "address": "0x1", "authChain": [] })).is_none());
        let signed = signed_from(&chain_for("bafyid")).unwrap();
        assert_eq!(signed.entity_id, "bafyid");
        assert_eq!(signed.auth_chain.as_array().unwrap().len(), 2);
        assert!(signed.delete_signature.is_none());
    }
}
