use std::{cell::RefCell, rc::Rc, time::Duration};

#[cfg(test)]
mod cancel_tests;
mod fetch_response_body_resource;
mod request_budget;

use bevy::{asset::AsyncReadExt, prelude::debug};
use common::rpc::{RpcCall, RpcResultSender};
use deno_core::{
    anyhow,
    error::{type_error, AnyError},
    futures::{FutureExt, TryStreamExt},
    op2, AsyncRefCell, BufView, ByteString, CancelFuture, CancelHandle, JsBuffer, OpDecl, OpState,
    Resource, ResourceId,
};
use deno_fetch::{FetchCancelHandle, FetchPermissions};
use deno_net::NetPermissions;
use deno_web::TimersPermission;
use http::{
    header::{ACCEPT_ENCODING, CONTENT_LENGTH, HOST, RANGE},
    HeaderName, HeaderValue, Method,
};
use serde::{Deserialize, Serialize};

use fetch_response_body_resource::FetchResponseBodyResource;
use request_budget::SceneRequestBudget;

use dcl_types::{interface::crdt_context::CrdtContext, RpcCalls, SceneResourceCounters};

// We have to provide these perm structs for the deno extensions even though the ops we
// actually expose don't route through them. They DENY rather than panic: the ops that
// consult them (`deno_net`'s socket ops in particular) are registered on the runtime and
// callable from JS, and a `panic!()` there unwinds across the V8 boundary -- which is
// `panic_cannot_unwind`, i.e. an immediate process abort, not a catchable error. Scene
// input must never be able to reach that, so refusing is the only safe answer.
pub struct FP;
impl FetchPermissions for FP {
    fn check_net_url(&mut self, _: &deno_core::url::Url, _: &str) -> Result<(), AnyError> {
        anyhow::bail!("network access is not available to scenes through this API")
    }

    fn check_read(&mut self, _: &std::path::Path, _: &str) -> Result<(), AnyError> {
        anyhow::bail!("file access is not available to scenes")
    }
}

pub struct TP;
impl TimersPermission for TP {
    fn allow_hrtime(&mut self) -> bool {
        false
    }
}

pub struct NP;
impl NetPermissions for NP {
    fn check_net<T: AsRef<str>>(
        &mut self,
        _host: &(T, Option<u16>),
        _api_name: &str,
    ) -> Result<(), AnyError> {
        anyhow::bail!("raw socket access is not available to scenes")
    }

    fn check_read(&mut self, _p: &std::path::Path, _api_name: &str) -> Result<(), AnyError> {
        anyhow::bail!("file access is not available to scenes")
    }

    fn check_write(&mut self, _p: &std::path::Path, _api_name: &str) -> Result<(), AnyError> {
        anyhow::bail!("file access is not available to scenes")
    }
}

// DoS caps for the multi-tenant authoritative server, enforced only in is_server mode.
const SERVER_MAX_BODY_BYTES: u64 = 10 * 1024 * 1024;
const SERVER_MAX_ASSET_BYTES: u64 = 64 * 1024 * 1024;

/// Counts Fetch calls waiting for permission, queued for a slot, or transferring.
pub(super) fn outstanding_calls(state: &OpState) -> u64 {
    state
        .try_borrow::<Rc<SceneRequestBudget>>()
        .map_or(0, |budget| budget.outstanding())
}

// list of op declarations
pub fn override_ops() -> Vec<OpDecl> {
    vec![op_fetch::<FP>(), op_fetch_send(), op_fetch_custom_client()]
}

// list of op declarations
pub fn ops() -> Vec<OpDecl> {
    vec![op_signed_fetch_headers()]
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
pub fn op_fetch<FP>(
    state: &mut OpState,
    #[string] method: String,
    #[string] url: String,
    #[serde] headers: Vec<(String, String)>,
    #[smi] client_rid: Option<u32>,
    has_body: bool,
    #[buffer] data: Option<JsBuffer>,
    #[smi] resource: Option<ResourceId>,
) -> Result<IsahcFetchReturn, AnyError>
where
    FP: FetchPermissions + 'static,
{
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
            let body = state.resource_table.take_any(rid)?;
            (None, Some(ResourceToBodyAdapter::new(body)))
        }
        (true, None, None) => return Err(type_error("missing fetch body")),
    };

    // On the authoritative server redirects are never auto-followed, so a 3xx onto a private
    // host can't bypass the per-request SSRF check or leak signed headers. The desktop/web
    // client still follows them, but under the public-only redirect policy (see
    // `build_scene_client` / `public_only_redirect`).
    let is_server = state.borrow::<CrdtContext>().is_server;
    let allow_private = allow_private_network(state);
    let client = if let Some(rid) = client_rid {
        let r = state.resource_table.get::<ClientResource>(rid)?;
        r.0.clone()
    } else {
        match state.try_borrow::<SceneHttpClient>() {
            Some(client) => client.0.clone(),
            None => {
                let client = build_scene_client(allow_private, is_server);
                state.put(SceneHttpClient(client.clone()));
                client
            }
        }
    };

    if method.len() > 50 {
        debug!("bad method {}", method.len());
        anyhow::bail!("nope");
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
    for (key, value) in headers {
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

    request = request.header("User-Agent", "DCLExplorer/0.1");

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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchResponse {
    status: u16,
    status_text: String,
    headers: Vec<(ByteString, ByteString)>,
    url: String,
    response_rid: ResourceId,
    content_length: Option<u64>,
    pub remote_addr_ip: Option<String>,
    pub remote_addr_port: Option<u16>,
    pub error: Option<String>,
}

#[op2(async)]
#[serde]
pub async fn op_fetch_send(
    state: Rc<RefCell<OpState>>,
    #[smi] rid: ResourceId,
) -> Result<FetchResponse, AnyError> {
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
        Ok(response) => {
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
        Err(_) => {
            counters.fetch_failed += 1;
            if is_storage {
                counters.storage_failed += 1;
            }
        }
    }
    result
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

    let request = Rc::try_unwrap(request)
        .ok()
        .expect("multiple op_fetch_send ongoing");

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
        anyhow::bail!("User denied fetch request");
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

    // Check both here and in the connect-time resolver so DNS rebinding cannot widen
    // the scene's allowance. Link-local and metadata remain forbidden in every mode.
    common::util::assert_public_url(&url, allow_private).await?;

    let request = if let Some(body) = request_body {
        let mut buf = Vec::new();
        let mut reader = body.into_async_read();
        reader.read_to_end(&mut buf).await?;
        request.body(buf)
    } else if let Some(body) = body_bytes {
        request.body(body)
    } else {
        request
    };
    let mut res = request
        .send()
        .await
        .map_err(|err| type_error(err.to_string()))?;

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
            anyhow::bail!("response body exceeds {cap} bytes (declared {declared}) ({url})");
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
                        anyhow::bail!("response body exceeds {cap} bytes ({url})");
                    }
                    collected.extend_from_slice(&c);
                }
                Ok(None) => break,
                Err(err) => return Err(type_error(err.to_string())),
            }
        }
        collected.freeze()
    } else {
        res.bytes().await?
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
        remote_addr_ip: None,
        remote_addr_port: None,
        error: None,
    })
}

// copy out the args struct so we can access the members...
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CreateHttpClientOptions {
    ca_certs: Vec<String>,
    proxy: Option<Proxy>,
    cert_chain: Option<String>,
    private_key: Option<String>,
}

#[derive(Deserialize, Default, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct Proxy {
    pub url: String,
    pub basic_auth: Option<BasicAuth>,
}

#[derive(Deserialize, Default, Debug, Clone)]
#[serde(default)]
pub struct BasicAuth {
    pub username: String,
    pub password: String,
}

pub struct ClientResource(reqwest::Client);
impl deno_core::Resource for ClientResource {}

/// Cached default client for a scene's own requests, in both server and client mode. Its
/// connections follow the scene's private-network allowance (see [`PublicOnlyResolver`]);
/// on the server, redirects are disabled too.
struct SceneHttpClient(reqwest::Client);

fn allow_private_network(state: &OpState) -> bool {
    let context = state.borrow::<CrdtContext>();
    // The renderer gives this sender only to its bundled HUD or an operator-selected
    // system scene. Hosted scenes never inherit that native privilege.
    context.preview || (!context.is_server && state.has::<dcl_js::SuperUserScene>())
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

/// Shared base for every scene http client: connect timeout, native TLS, UA, and the
/// resolver that refuses any non-public address at connect time, so the address that was
/// checked is the address that is dialled. `allow_private` (preview or a native system
/// scene) widens this to the local network but never to link-local / metadata.
fn scene_client_builder(allow_private: bool) -> reqwest::ClientBuilder {
    platform::http_client_builder()
        .connect_timeout(Duration::from_secs(5))
        .use_native_tls()
        .user_agent("DCLExplorer/0.1")
        .dns_resolver(std::sync::Arc::new(PublicOnlyResolver { allow_private }))
}

/// The default client for a scene's own requests. On the authoritative server redirects are
/// disabled entirely (a 3xx must not silently re-target or forward signed headers). The
/// desktop client keeps following them — matching the browser — but through
/// [`public_only_redirect`]: a hostname hop is re-resolved by [`PublicOnlyResolver`] at
/// connect, and an IP-literal hop (dialled with no DNS lookup, so the resolver never sees it)
/// is vetted by the policy.
fn build_scene_client(allow_private: bool, is_server: bool) -> reqwest::Client {
    scene_client_builder(allow_private)
        .redirect(if is_server {
            reqwest::redirect::Policy::none()
        } else {
            public_only_redirect(allow_private)
        })
        .build()
        .unwrap()
}

/// Parse a URL host component as an IP literal, tolerating the brackets the `url` crate keeps
/// around an IPv6 literal (`[::1]`). Returns `None` for a hostname.
fn host_ip_literal(host: &str) -> Option<std::net::IpAddr> {
    host.strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .unwrap_or(host)
        .parse()
        .ok()
}

/// Redirect policy for the desktop client. Redirects are followed (as the browser would), but
/// a hop onto a non-public IP *literal* is refused: reqwest dials an IP literal with no DNS
/// lookup, so [`PublicOnlyResolver`] — which vets every hostname hop at connect — never sees
/// it. This is the redirect-time twin of [`reject_non_public_proxy`]. `allow_private`
/// (preview or a native system scene) permits the local network but never link-local / metadata.
fn public_only_redirect(allow_private: bool) -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(move |attempt| {
        // Policy::custom replaces reqwest's built-in hop cap, so re-impose the default of 10.
        if attempt.previous().len() >= 10 {
            return attempt.error("too many redirects".to_string());
        }
        if let Some(ip) = attempt.url().host_str().and_then(host_ip_literal) {
            let permitted = allow_private && common::util::is_private_lan(&ip);
            if !permitted && common::util::is_forbidden_ip(&ip) {
                return attempt.error(format!(
                    "redirect to non-public address {ip} is not allowed"
                ));
            }
        }
        attempt.follow()
    })
}

/// Vet a scene-supplied proxy endpoint. Only an IP *literal* needs checking here: reqwest's
/// proxy connector is an `HttpConnector<DynResolver>`, which dials an IP literal directly but
/// routes a *hostname* endpoint through [`PublicOnlyResolver`] at connect — so a hostname
/// proxy is already egress-checked there and a literal is the one case that skips it. (A
/// `socks*://` proxy needs no handling: the `socks` feature is off, so `reqwest::Proxy::http`
/// rejects it before we get here.) `allow_private` (preview or a native system scene)
/// permits the local network but never link-local / metadata.
fn reject_non_public_proxy(proxy_url: &str, allow_private: bool) -> Result<(), AnyError> {
    let url = deno_core::url::Url::parse(proxy_url)?;
    if let Some(ip) = url.host_str().and_then(host_ip_literal) {
        let permitted = allow_private && common::util::is_private_lan(&ip);
        if !permitted && common::util::is_forbidden_ip(&ip) {
            anyhow::bail!("custom fetch client proxy may not target a non-public address");
        }
    }
    Ok(())
}

#[op2]
#[serde]
pub fn op_fetch_custom_client(
    state: &mut OpState,
    #[serde] args: CreateHttpClientOptions,
) -> Result<ResourceId, AnyError> {
    debug!("op_fetch_custom_client");

    // A custom client is scene-supplied transport configuration, and on the shared
    // authoritative server none of it may be honoured:
    //
    // * `proxy` re-targets the connection at an address the SSRF guard never sees. The
    //   guard inspects the request URL; the proxy is what actually gets dialled. Pointing
    //   it at 169.254.169.254 reaches cloud metadata with a perfectly public-looking URL,
    //   and disabling redirects does nothing about it.
    // * `ca_certs` makes the scene a trust root for the server's outbound TLS.
    // * `cert_chain`/`private_key` let a scene present a client identity as the server.
    //
    // Refused rather than ignored so a scene that tries gets an error it can see.
    // Everything else (the plain `createHttpClient()` case) still works.
    let is_server = state.borrow::<CrdtContext>().is_server;
    let allow_private = allow_private_network(state);
    if is_server {
        if args.proxy.is_some() {
            anyhow::bail!("custom fetch clients may not set a proxy on the authoritative server");
        }
        if !args.ca_certs.is_empty() {
            anyhow::bail!(
                "custom fetch clients may not add root certificates on the authoritative server"
            );
        }
        if args.cert_chain.is_some() || args.private_key.is_some() {
            anyhow::bail!(
                "custom fetch clients may not set a client identity on the authoritative server"
            );
        }
        // same transport rules as the default server client: no redirects, public-only DNS
        return Ok(state
            .resource_table
            .add(ClientResource(build_scene_client(allow_private, true))));
    }

    // Client mode: the scene may still tune TLS trust/identity for its own machine, but its
    // connections use the scene's private-network allowance, and redirects are held
    // to the same egress policy as the default client (see `public_only_redirect`). A proxy
    // endpoint given as an IP literal skips that resolver, so it is vetted synchronously by
    // `reject_non_public_proxy` (a hostname endpoint is resolved through the resolver).
    let mut builder =
        scene_client_builder(allow_private).redirect(public_only_redirect(allow_private));
    if let Some(proxy_def) = args.proxy {
        reject_non_public_proxy(&proxy_def.url, allow_private)?;
        let mut proxy = reqwest::Proxy::http(proxy_def.url)?;
        if let Some(creds) = proxy_def.basic_auth {
            proxy = proxy.basic_auth(&creds.username, &creds.password);
        }
        builder = builder.proxy(proxy);
    }
    if !args.ca_certs.is_empty() {
        for ca_cert in &args.ca_certs {
            builder =
                builder.add_root_certificate(reqwest::Certificate::from_pem(ca_cert.as_bytes())?);
        }
    }
    if let (Some(chain), Some(key)) = (args.cert_chain, args.private_key) {
        builder = builder.identity(reqwest::Identity::from_pkcs12_der(chain.as_bytes(), &key)?);
    }

    Ok(state.resource_table.add(ClientResource(builder.build()?)))
}

#[op2(async)]
#[serde]
pub fn op_signed_fetch_headers(
    state: Rc<RefCell<OpState>>,
    #[string] uri: String,
    #[string] method: Option<String>,
    #[string] body: Option<String>,
) -> impl Future<Output = Result<Vec<(String, String)>, AnyError>> {
    let rx = dcl_js::fetch::start_signed_fetch_headers(state, uri, method, body);
    async move { dcl_js::fetch::finish_signed_fetch_headers(rx?).await }
}

use core::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

#[allow(clippy::type_complexity)]
pub struct ResourceToBodyAdapter(
    Rc<dyn Resource>,
    Option<Pin<Box<dyn Future<Output = Result<BufView, anyhow::Error>>>>>,
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
        let (mut runtime, _) = crate::js::create_runtime(
            false,
            false,
            "test-rejected-upload",
            false,
            Default::default(),
        );
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
                &deno_core::ModuleSpecifier::parse("ext:test/upload.js").unwrap(),
                r#"
                import { ReadableStream } from "ext:deno_web/06_streams.js";
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
