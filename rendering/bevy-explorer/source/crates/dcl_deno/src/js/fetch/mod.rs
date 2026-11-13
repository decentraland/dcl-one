use crate::js::errors::{bail, type_error, OpError};
use std::{cell::RefCell, rc::Rc, time::Duration};

#[cfg(test)]
mod cancel_tests;
mod fetch_response_body_resource;
mod request_budget;

use bevy::{asset::AsyncReadExt, prelude::debug};
use common::rpc::{RpcCall, RpcResultSender};
use deno_core::{
    error::AnyError,
    futures::{FutureExt, TryStreamExt},
    op2, v8, AsyncRefCell, BufView, ByteString, CancelFuture, CancelHandle, JsBuffer, OpDecl,
    OpState, Resource, ResourceId,
};
use http::{
    header::{ACCEPT_ENCODING, CONTENT_LENGTH, HOST, RANGE},
    HeaderName, HeaderValue, Method,
};
use serde::Serialize;

use super::plain::Plain;
use fetch_response_body_resource::FetchResponseBodyResource;
use request_budget::SceneRequestBudget;

use dcl_types::{interface::crdt_context::CrdtContext, RpcCalls, SceneResourceCounters};

/// The cancel handle `op_fetch` returns alongside the request.
pub struct FetchCancelHandle(pub Rc<CancelHandle>);

impl Resource for FetchCancelHandle {
    fn name(&self) -> std::borrow::Cow<'_, str> {
        "fetchCancelHandle".into()
    }

    fn close(self: Rc<Self>) {
        self.0.cancel()
    }
}

// DoS caps for the multi-tenant authoritative server, enforced only in is_server mode.
// SERVER_MAX_BODY_BYTES bounds streamed request bodies too.
const SERVER_MAX_BODY_BYTES: u64 = 10 * 1024 * 1024;
const SERVER_MAX_ASSET_BYTES: u64 = 64 * 1024 * 1024;

/// Counts Fetch calls waiting for permission, queued for a slot, or transferring.
pub(super) fn outstanding_calls(state: &OpState) -> u64 {
    state
        .try_borrow::<Rc<SceneRequestBudget>>()
        .map_or(0, |budget| budget.outstanding())
}

// list of op declarations
pub fn ops() -> Vec<OpDecl> {
    vec![
        op_fetch(),
        op_fetch_send(),
        op_fetch_promise_is_settled(),
        op_utf8_to_byte_string(),
        op_signed_fetch_headers(),
    ]
}

struct FetchRequestResource {
    cancel: Rc<CancelHandle>,
    request: reqwest::RequestBuilder,
    request_body: Option<ResourceToBodyAdapter>,
    body_bytes: Option<Vec<u8>>,
    url: String,
    /// URL host is the world-storage service (`storage.decentraland.*`).
    is_storage: bool,
    range_request: bool,
}
impl deno_core::Resource for FetchRequestResource {}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IsahcFetchReturn {
    request_rid: ResourceId,
    cancel_handle_rid: Option<ResourceId>,
}

#[op2]
#[serde]
#[allow(clippy::too_many_arguments)]
pub fn op_fetch(
    state: &mut OpState,
    #[string] method: String,
    #[string] url: String,
    #[scoped] headers: Plain<Vec<(String, String)>>,
    #[smi] client_rid: Option<u32>,
    has_body: bool,
    #[buffer] data: Option<JsBuffer>,
    #[smi] resource: Option<ResourceId>,
) -> Result<IsahcFetchReturn, OpError> {
    debug!("op_fetch");
    // TODO scene permissions

    let is_storage = deno_core::url::Url::parse(&url)
        .ok()
        .and_then(|u| {
            u.host_str().map(|h| {
                h.strip_prefix("storage.decentraland.")
                    .is_some_and(|tld| !tld.contains('.'))
            })
        })
        .unwrap_or(false);

    {
        let counters = state.borrow_mut::<SceneResourceCounters>();
        counters.fetch_started += 1;
        if is_storage {
            counters.storage_requests += 1;
        }
    }

    // Deno transfers ownership of a streaming upload to op_fetch. Take its RID
    // before any fallible setup so rejection and cancellation close the JS producer.
    let (body_bytes, request_body) = match (has_body, data, resource) {
        (false, _, _) => (None, None),
        (true, Some(data), _) => (Some(data.to_vec()), None),
        (true, None, Some(rid)) => {
            let body = crate::js::stream_resource::take_upload(state, rid)?;
            (None, Some(ResourceToBodyAdapter::new(body)))
        }
        (true, None, None) => return Err(type_error("missing fetch body").into()),
    };

    // Scenes cannot create HTTP clients, so a client rid is a forged one.
    if client_rid.is_some() {
        return Err(type_error("custom HTTP clients are not supported").into());
    }
    // Redirects are never followed here: the fetch JS sends each hop back through this op,
    // so the permission, budget and address checks apply to every hop.
    let allow_private = allow_private_network(state);
    let client = match state.try_borrow::<SceneHttpClient>() {
        Some(client) => client.0.clone(),
        None => {
            let client = build_scene_client(allow_private)?;
            state.put(SceneHttpClient(client.clone()));
            client
        }
    };

    if method.len() > 50 {
        debug!("bad method {}", method.len());
        bail!("nope");
    }

    let method = Method::from_bytes(method.as_bytes())?;
    let mut request = client.request(method.clone(), &url);

    if let Some(body) = &request_body {
        let (size, upper) = body.0.size_hint();
        if size > 0 && upper == Some(size) {
            request = request.header(CONTENT_LENGTH, HeaderValue::from(size));
        }
    } else if !has_body && matches!(method, Method::POST | Method::PUT) {
        request = request.header(CONTENT_LENGTH, HeaderValue::from(0));
    }

    let mut range_request = false;
    for (key, value) in headers.0 {
        let name =
            HeaderName::from_bytes(key.as_bytes()).map_err(|err| type_error(err.to_string()))?;
        let v =
            HeaderValue::from_bytes(value.as_bytes()).map_err(|err| type_error(err.to_string()))?;

        if matches!(name, RANGE) {
            range_request = true;
            request = request.header(name, v);
            // https://fetch.spec.whatwg.org/#http-network-or-cache-fetch step 18
            // If httpRequest’s header list contains `Range`, then append (`Accept-Encoding`, `identity`)
            request = request.header(ACCEPT_ENCODING, HeaderValue::from_static("identity"));
        } else if !matches!(name, HOST | CONTENT_LENGTH) {
            request = request.header(name, v);
        }
    }

    request = request.header("User-Agent", USER_AGENT);

    debug!("request {url}");
    let cancel = CancelHandle::new_rc();
    let cancel_handle_rid = state.resource_table.add(FetchCancelHandle(cancel.clone()));
    let request_rid = state.resource_table.add(FetchRequestResource {
        cancel,
        body_bytes,
        request_body,
        request,
        url,
        is_storage,
        range_request,
    });

    debug!("returning {:?}", request_rid);
    Ok(IsahcFetchReturn {
        request_rid,
        cancel_handle_rid: Some(cancel_handle_rid),
    })
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FetchResponse {
    status: u16,
    status_text: String,
    headers: Vec<(ByteString, ByteString)>,
    url: String,
    response_rid: ResourceId,
    content_length: Option<u64>,
    /// The client never decompresses, so the headers always describe the body.
    body_decoded: bool,
    /// A transport failure as `(detail, cause)`; the fetch JS throws `TypeError("fetch failed")`.
    error: Option<(String, String)>,
}

#[op2]
#[serde]
pub async fn op_fetch_send(
    state: Rc<RefCell<OpState>>,
    #[smi] rid: ResourceId,
) -> Result<FetchResponse, OpError> {
    // Copy the flag before taking the resource so failed storage requests count too.
    let is_storage = state
        .borrow()
        .resource_table
        .get::<FetchRequestResource>(rid)
        .map(|r| r.is_storage)
        .unwrap_or(false);
    let started = std::time::Instant::now();
    let result = fetch_send_inner(state.clone(), rid).await;
    let mut op_state = state.borrow_mut();
    let counters = op_state.borrow_mut::<SceneResourceCounters>();
    counters.fetch_duration_us += started.elapsed().as_micros() as u64;
    match &result {
        Ok(response) if response.error.is_none() => {
            counters.fetch_completed += 1;
            match response.status {
                200..=299 => counters.fetch_status_2xx += 1,
                300..=399 => counters.fetch_status_3xx += 1,
                400..=499 => counters.fetch_status_4xx += 1,
                500..=599 => counters.fetch_status_5xx += 1,
                _ => {}
            }
            if is_storage {
                counters.storage_completed += 1;
                if matches!(response.status, 401 | 403) {
                    counters.storage_unauthorized += 1;
                }
            }
        }
        _ => {
            counters.fetch_failed += 1;
            if is_storage {
                counters.storage_failed += 1;
            }
        }
    }
    Ok(result?)
}

async fn fetch_send_inner(
    state: Rc<RefCell<OpState>>,
    rid: ResourceId,
) -> Result<FetchResponse, AnyError> {
    debug!("op_fetch_send");
    let request = state
        .borrow_mut()
        .resource_table
        .take::<FetchRequestResource>(rid)?;

    let Ok(request) = Rc::try_unwrap(request) else {
        bail!("fetch request is already being sent");
    };

    // Take the request first so its upload is dropped even when the signal has
    // already fired and Deno never polls the permission or transfer future.
    let cancel = request.cancel.clone();
    send_with_permission(state, request)
        .or_cancel(cancel)
        .await?
}

async fn send_with_permission(
    state: Rc<RefCell<OpState>>,
    request: FetchRequestResource,
) -> Result<FetchResponse, AnyError> {
    let (budget, is_server, allow_private) = {
        let mut op_state = state.borrow_mut();
        let is_server = op_state.borrow::<CrdtContext>().is_server;
        let allow_private = allow_private_network(&op_state);
        if op_state.try_borrow::<Rc<SceneRequestBudget>>().is_none() {
            op_state.put(Rc::new(SceneRequestBudget::for_scene(is_server)));
        }
        (
            op_state.borrow::<Rc<SceneRequestBudget>>().clone(),
            is_server,
            allow_private,
        )
    };
    let _call = budget.track_call();

    let (sx, rx) = RpcResultSender::channel();
    state
        .borrow_mut()
        .borrow_mut::<RpcCalls>()
        .push(RpcCall::RequestGenericPermission {
            ty: common::structs::PermissionType::Fetch,
            message: Some(request.url.clone()),
            response: sx,
        })?;
    let permit = rx.await?;
    if !permit {
        bail!("User denied fetch request");
    }

    budget
        .run(send_request(state, request, is_server, allow_private))
        .await
        .map_err(|_| type_error("fetch request timed out"))?
}

async fn send_request(
    state: Rc<RefCell<OpState>>,
    resource: FetchRequestResource,
    is_server: bool,
    allow_private: bool,
) -> Result<FetchResponse, AnyError> {
    let FetchRequestResource {
        request,
        body_bytes,
        request_body,
        url,
        range_request,
        ..
    } = resource;

    // Scenes are untrusted code running with the host's network position. Check both
    // here and in the connect-time resolver so DNS rebinding cannot widen the scene's
    // allowance. Link-local and metadata remain forbidden in every mode.
    common::util::assert_public_url(&url, allow_private).await?;

    let request = if let Some(body) = request_body {
        let mut buf = Vec::new();
        if is_server {
            body.into_async_read()
                .take(SERVER_MAX_BODY_BYTES + 1)
                .read_to_end(&mut buf)
                .await?;
            if buf.len() as u64 > SERVER_MAX_BODY_BYTES {
                bail!("request body exceeds {SERVER_MAX_BODY_BYTES} bytes ({url})");
            }
        } else {
            // the client bounds neither request nor response bodies
            body.into_async_read().read_to_end(&mut buf).await?;
        }
        request.body(buf)
    } else if let Some(body) = body_bytes {
        request.body(body)
    } else {
        request
    };
    let mut res = match request.send().await {
        Ok(res) => res,
        Err(err) => return Ok(transport_failure(url, &err)),
    };

    let status = res.status();
    let mut headers = Vec::new();
    for (key, val) in res.headers().iter() {
        headers.push((key.as_str().into(), val.as_bytes().into()));
    }

    let content_length = res.content_length();

    let body_cap = if is_server {
        Some(if range_request {
            SERVER_MAX_ASSET_BYTES
        } else {
            SERVER_MAX_BODY_BYTES
        })
    } else {
        None
    };

    if let (Some(cap), Some(declared)) = (body_cap, content_length) {
        if declared > cap {
            bail!("response body exceeds {cap} bytes (declared {declared}) ({url})");
        }
    }

    let chunk = if let Some(cap) = body_cap {
        let mut collected = bytes::BytesMut::new();
        let mut total: u64 = 0;
        loop {
            match res.chunk().await {
                Ok(Some(c)) => {
                    total += c.len() as u64;
                    if total > cap {
                        bail!("response body exceeds {cap} bytes ({url})");
                    }
                    collected.extend_from_slice(&c);
                }
                Ok(None) => break,
                Err(err) => return Ok(transport_failure(url, &err)),
            }
        }
        collected.freeze()
    } else {
        match res.bytes().await {
            Ok(bytes) => bytes,
            Err(err) => return Ok(transport_failure(url, &err)),
        }
    };

    state
        .borrow_mut()
        .borrow_mut::<SceneResourceCounters>()
        .fetch_bytes_down += chunk.len() as u64;

    let response_rid = state
        .borrow_mut()
        .resource_table
        .add(FetchResponseBodyResource {
            data: AsyncRefCell::new(chunk),
            cancel: CancelHandle::default(),
            size: content_length,
        });

    debug!("request response [{:?} bytes]", content_length);
    Ok(FetchResponse {
        status: status.as_u16(),
        status_text: status.canonical_reason().unwrap_or("").to_string(),
        headers,
        url,
        response_rid,
        content_length,
        body_decoded: false,
        error: None,
    })
}

/// A request or response body that failed in transit; the fetch JS rejects with
/// `TypeError("fetch failed")` and this as its cause.
fn transport_failure(url: String, err: &reqwest::Error) -> FetchResponse {
    let detail = err.to_string();
    let mut cause: &dyn std::error::Error = err;
    while let Some(source) = cause.source() {
        cause = source;
    }
    FetchResponse {
        url,
        error: Some((detail, cause.to_string())),
        ..Default::default()
    }
}

/// Cached default client for a scene's own requests. Its connections follow the scene's
/// private-network allowance (see [`PublicOnlyResolver`]).
pub(crate) struct SceneHttpClient(pub(crate) reqwest::Client);

pub(super) fn allow_private_network(state: &OpState) -> bool {
    // The renderer gives this sender only to its bundled HUD or an operator-selected
    // system scene. Hosted scenes never inherit that native privilege.
    state.try_borrow::<CrdtContext>().is_some_and(|context| {
        context.preview || (!context.is_server && state.has::<dcl_js::SuperUserScene>())
    })
}

/// DNS resolver that only ever hands back public addresses.
///
/// `assert_public_url` runs before the request is sent, but the client resolves the host
/// AGAIN when it connects — nothing ties the two lookups together. A hostile authoritative
/// nameserver can therefore answer the pre-flight check with a public address and the
/// connect with 169.254.169.254 (DNS rebinding), and no scene-side trickery is needed to
/// reach it: plain `fetch()` from the SDK is enough. Enforcing inside the resolver makes
/// the checked answer and the dialled answer the same answer by construction.
struct PublicOnlyResolver {
    allow_private: bool,
}

impl reqwest::dns::Resolve for PublicOnlyResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let allow_private = self.allow_private;
        let host = name.as_str().to_owned();
        Box::pin(async move {
            // port 0: reqwest fills in the real port after resolution
            let addrs = common::util::resolve_public_addrs(&host, 0, allow_private)
                .await
                .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                    e.to_string().into()
                })?;
            Ok(Box::new(addrs.into_iter()) as reqwest::dns::Addrs)
        })
    }
}

/// What scene fetches and websockets send as their User-Agent.
pub(crate) const USER_AGENT: &str = "DCLExplorer/0.1";

/// Shared base for every scene http client: connect timeout, native TLS, UA, and the
/// resolver that refuses any non-public address at connect time, so the address that was
/// checked is the address that is dialled. `allow_private` (preview or a native system
/// scene) widens this to the local network but never to link-local / metadata.
fn scene_client_builder(allow_private: bool) -> reqwest::ClientBuilder {
    platform::http_client_builder()
        .connect_timeout(Duration::from_secs(5))
        .use_native_tls()
        .user_agent(USER_AGENT)
        .dns_resolver(std::sync::Arc::new(PublicOnlyResolver { allow_private }))
}

/// The client for a scene's own requests. It never follows a redirect: the fetch JS does,
/// sending each hop back through `op_fetch`.
fn build_scene_client(allow_private: bool) -> Result<reqwest::Client, reqwest::Error> {
    scene_client_builder(allow_private)
        .redirect(reqwest::redirect::Policy::none())
        .build()
}

#[op2(fast)]
fn op_fetch_promise_is_settled(promise: v8::Local<v8::Promise>) -> bool {
    promise.state() != v8::PromiseState::Pending
}

#[op2]
#[serde]
fn op_utf8_to_byte_string(#[string] input: String) -> ByteString {
    input.into()
}

#[op2]
#[serde]
pub fn op_signed_fetch_headers(
    state: Rc<RefCell<OpState>>,
    #[string] uri: String,
    #[string] method: Option<String>,
    #[string] body: Option<String>,
) -> impl Future<Output = Result<Vec<(String, String)>, OpError>> {
    let rx = dcl_js::fetch::start_signed_fetch_headers(state, uri, method, body);
    async move { Ok(dcl_js::fetch::finish_signed_fetch_headers(rx?).await?) }
}

use core::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

#[allow(clippy::type_complexity)]
pub struct ResourceToBodyAdapter(
    Rc<dyn Resource>,
    Option<Pin<Box<dyn Future<Output = Result<BufView, deno_error::JsErrorBox>>>>>,
);

impl ResourceToBodyAdapter {
    pub fn new(resource: Rc<dyn Resource>) -> Self {
        let future = resource.clone().read(64 * 1024);
        Self(resource, Some(future))
    }
}

impl Drop for ResourceToBodyAdapter {
    fn drop(&mut self) {
        self.0.clone().close();
    }
}

// SAFETY: we only use this on a single-threaded executor
unsafe impl Send for ResourceToBodyAdapter {}
// SAFETY: we only use this on a single-threaded executor
unsafe impl Sync for ResourceToBodyAdapter {}

impl deno_core::futures::Stream for ResourceToBodyAdapter {
    type Item = Result<bytes::Bytes, std::io::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        if let Some(mut fut) = this.1.take() {
            match fut.poll_unpin(cx) {
                Poll::Pending => {
                    this.1 = Some(fut);
                    Poll::Pending
                }
                Poll::Ready(res) => match res {
                    Ok(buf) if buf.is_empty() => Poll::Ready(None),
                    Ok(_) => {
                        this.1 = Some(this.0.clone().read(64 * 1024));
                        Poll::Ready(Some(
                            res.map(|b| b.to_vec().into())
                                .map_err(std::io::Error::other),
                        ))
                    }
                    _ => Poll::Ready(Some(
                        res.map(|b| b.to_vec().into())
                            .map_err(std::io::Error::other),
                    )),
                },
            }
        } else {
            Poll::Ready(None)
        }
    }
}

#[cfg(test)]
mod upload_tests {
    use super::*;
    use dcl_types::SceneId;

    #[tokio::test]
    async fn rejected_upload_closes_its_resource_and_cancels_the_stream() {
        let (mut runtime, _) = crate::js::create_runtime(false, false, Default::default());
        let state = runtime.op_state();
        {
            let mut state = state.borrow_mut();
            state.put(CrdtContext::new(
                SceneId(bevy::prelude::Entity::from_raw(0)),
                "hash".into(),
                "title".into(),
                false,
                false,
                true,
            ));
            state.put(SceneResourceCounters::default());
            state.put(RpcCalls::default());
        }
        let script = runtime
            .load_side_es_module_from_code(
                &deno_core::ModuleSpecifier::parse("file:///test/upload.js").unwrap(),
                r#"
                const { ReadableStream } = Deno.core.loadExtScript("ext:deno_web/06_streams.js");
                globalThis.cancelled = false;
                globalThis.failure = "";
                const body = new ReadableStream({ cancel() { globalThis.cancelled = true; } });
                fetch("https://example.org/upload", {method: "X".repeat(51), body})
                    .catch(e => { globalThis.failure = String(e); });
            "#,
            )
            .await
            .unwrap();
        let evaluated = runtime.mod_evaluate(script);
        let settled = tokio::time::timeout(
            Duration::from_secs(2),
            runtime.run_event_loop(Default::default()),
        )
        .await;
        assert!(
            !state
                .borrow()
                .resource_table
                .names()
                .any(|(_, name)| name == "readableStream"),
            "rejected fetch leaked its upload RID"
        );
        settled.expect("upload stream did not close").unwrap();
        evaluated.await.unwrap();
        runtime
            .execute_script(
                "<test>",
                deno_core::ascii_str!(
                    r#"
            if (!failure) throw new Error("expected overlong method to be rejected");
            if (!cancelled) throw new Error("upload producer was not cancelled");
        "#
                ),
            )
            .unwrap();
    }
}
