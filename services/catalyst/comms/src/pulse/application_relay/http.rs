//! Internal authority client. It never reconstructs or transmits a bearer JWT.
//! The path, frames and limits are the room authority's one copy
//! (`crate::protocol::room_authority`), the gatekeeper's too.
use super::{
    Admission, AdmittedScope, AuthorityError, AuthorityInput, MAX_AUTHORITY_LEASE, RoomAuthority,
};
use crate::protocol::room_authority::{
    self as wire, AUTHORITY_PATH, MAX_BODY, MAX_LEASES, address_bytes,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use futures::StreamExt;
use hmac::{Hmac, KeyInit, Mac};
use prost::Message;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::Semaphore;

type ScopeKey = ([u8; 32], [u8; 32]);

struct Entry {
    serial: u64,
    deadline: Instant,
    lease: Option<wire::Lease>,
    in_flight: bool,
}
#[derive(Default)]
struct Registry {
    entries: HashMap<ScopeKey, Entry>,
    serial: u64,
    last_prune: Option<Instant>,
}
struct Inner {
    client: reqwest::Client,
    endpoint: reqwest::Url,
    service_key: [u8; 32],
    instance: [u8; 16],
    registry: Mutex<Registry>,
    permits: Arc<Semaphore>,
}
#[derive(Clone)]
pub struct HttpAuthority {
    inner: Arc<Inner>,
}

// Dropping an HTTP future (including the relay's outer deadline) cannot
// strand an owned operation or recreate a scope released during the await.
struct FlightGuard {
    inner: Arc<Inner>,
    scope: ScopeKey,
    serial: u64,
}
impl Drop for FlightGuard {
    fn drop(&mut self) {
        if let Ok(mut registry) = self.inner.registry.lock() {
            if let Some(entry) = registry
                .entries
                .get_mut(&self.scope)
                .filter(|entry| entry.serial == self.serial)
            {
                if entry.lease.is_some() && entry.deadline > Instant::now() {
                    entry.in_flight = false;
                } else {
                    registry.entries.remove(&self.scope);
                }
            }
        }
    }
}

impl HttpAuthority {
    pub fn new(
        endpoint: &str,
        encoded_service_key: &str,
        allow_loopback_http: bool,
    ) -> anyhow::Result<Self> {
        let endpoint = reqwest::Url::parse(endpoint)
            .map_err(|_| anyhow::anyhow!("invalid room authority URL"))?;
        let loopback = endpoint
            .host_str()
            .and_then(|host| {
                host.trim_matches(['[', ']'])
                    .parse::<std::net::IpAddr>()
                    .ok()
            })
            .is_some_and(|ip| ip.is_loopback());
        if (endpoint.scheme() != "https"
            && !(endpoint.scheme() == "http" && allow_loopback_http && loopback))
            || endpoint.path() != AUTHORITY_PATH
            || endpoint.query().is_some()
            || endpoint.fragment().is_some()
            || !endpoint.username().is_empty()
            || endpoint.password().is_some()
        {
            anyhow::bail!(
                "room authority requires its exact HTTPS endpoint (explicit loopback HTTP fixtures only)"
            );
        }
        let service_key: [u8; 32] = URL_SAFE_NO_PAD
            .decode(encoded_service_key)
            .ok()
            .and_then(|key| key.try_into().ok())
            .ok_or_else(|| {
                anyhow::anyhow!("room authority service key must be 32 base64url bytes")
            })?;
        let mut instance = [0; 16];
        getrandom::fill(&mut instance)
            .map_err(|_| anyhow::anyhow!("cannot create room authority instance"))?;
        // The pulse process's one HTTP client, built from the one client
        // configuration (`crate::http::HttpClientCfg`: no redirects followed,
        // the shared user agent and pool limits) with a 750 ms budget. Not
        // `Outbound`: its SSRF guard refuses the loopback authority fixture,
        // and this endpoint is operator configuration checked above (exact
        // https URL, or loopback http only when explicitly allowed).
        let client = crate::http::HttpClientCfg::default()
            .with_total_timeout(Duration::from_millis(750))
            .builder()
            .no_proxy()
            .build()?;
        Ok(Self {
            inner: Arc::new(Inner {
                client,
                endpoint,
                service_key,
                instance,
                registry: Mutex::new(Registry::default()),
                permits: Arc::new(Semaphore::new(128)),
            }),
        })
    }
}

fn context(inner: &Inner, nonce: &[u8; 32], lease: &wire::Lease) -> wire::Context {
    wire::Context {
        pulse_instance: inner.instance.to_vec(),
        connection_generation: nonce[..16].to_vec(),
        lease_id: lease.lease_id.clone(),
        authority_incarnation: lease.authority_incarnation.clone(),
    }
}
fn signed_headers(key: &[u8; 32], body: &[u8], timestamp: &str, nonce: &str) -> Result<String, ()> {
    let hash = Sha256::digest(body)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let input = format!("POST\n{AUTHORITY_PATH}\n{timestamp}\n{nonce}\n{hash}");
    let mut signature = Hmac::<Sha256>::new_from_slice(key).map_err(|_| ())?;
    signature.update(input.as_bytes());
    Ok(URL_SAFE_NO_PAD.encode(signature.finalize().into_bytes()))
}
async fn request(
    inner: &Inner,
    operation: wire::authority_request::Operation,
) -> Result<wire::AuthorityResponse, ()> {
    let body = wire::AuthorityRequest {
        operation: Some(operation),
    }
    .encode_to_vec();
    if body.len() > MAX_BODY {
        return Err(());
    }
    let mut nonce = [0; 16];
    getrandom::fill(&mut nonce).map_err(|_| ())?;
    let nonce = URL_SAFE_NO_PAD.encode(nonce);
    let timestamp = chrono::Utc::now().timestamp().to_string();
    let signature = signed_headers(&inner.service_key, &body, &timestamp, &nonce)?;
    let response = inner
        .client
        .post(inner.endpoint.clone())
        .header("Content-Type", "application/x-protobuf")
        .header("Accept", "application/x-protobuf")
        .header("X-Pulse-Authority-Timestamp", timestamp)
        .header("X-Pulse-Authority-Nonce", nonce)
        .header("X-Pulse-Authority-Signature", signature)
        .body(body)
        .send()
        .await
        .map_err(|_| ())?;
    if response.status() != reqwest::StatusCode::OK
        || !response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| {
                value
                    .split(';')
                    .next()
                    .is_some_and(|mime| mime.trim().eq_ignore_ascii_case("application/x-protobuf"))
            })
        || response
            .content_length()
            .is_some_and(|len| len > MAX_BODY as u64)
    {
        return Err(());
    }
    let mut stream = response.bytes_stream();
    let mut body = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| ())?;
        if body.len() + chunk.len() > MAX_BODY {
            return Err(());
        }
        body.extend_from_slice(&chunk);
    }
    wire::AuthorityResponse::decode(body.as_slice()).map_err(|_| ())
}

impl RoomAuthority for HttpAuthority {
    fn authorize(
        &self,
        input: AuthorityInput,
    ) -> Pin<Box<dyn Future<Output = Result<Admission, AuthorityError>> + Send + '_>> {
        Box::pin(async move {
            let _permit = self
                .inner
                .permits
                .clone()
                .try_acquire_owned()
                .map_err(|_| AuthorityError::Unavailable)?;
            let scope = input.key();
            let renewal = matches!(input, AuthorityInput::Renew(_));
            let started = Instant::now();
            let (serial, previous, independent_join) = {
                let mut registry = self
                    .inner
                    .registry
                    .lock()
                    .map_err(|_| AuthorityError::Unavailable)?;
                if registry
                    .last_prune
                    .is_none_or(|last| started.duration_since(last) >= Duration::from_secs(1))
                {
                    registry.entries.retain(|_, entry| entry.deadline > started);
                    registry.last_prune = Some(started);
                }
                if registry
                    .entries
                    .get(&scope)
                    .is_some_and(|entry| entry.deadline <= started)
                {
                    registry.entries.remove(&scope);
                }
                let independent_join = !renewal
                    && registry
                        .entries
                        .get(&scope)
                        .is_some_and(|entry| entry.lease.is_some());
                if let Some(entry) = registry.entries.get(&scope) {
                    if entry.in_flight && !independent_join {
                        return Err(AuthorityError::Unavailable);
                    }
                } else if renewal || registry.entries.len() >= MAX_LEASES {
                    return Err(AuthorityError::Unavailable);
                }
                let previous = registry
                    .entries
                    .get(&scope)
                    .and_then(|entry| entry.lease.clone());
                if independent_join {
                    (registry.entries[&scope].serial, previous, true)
                } else {
                    registry.serial = registry
                        .serial
                        .checked_add(1)
                        .ok_or(AuthorityError::Unavailable)?;
                    let serial = registry.serial;
                    let entry = registry.entries.entry(scope).or_insert(Entry {
                        serial,
                        deadline: started + Duration::from_millis(750),
                        lease: None,
                        in_flight: true,
                    });
                    entry.serial = serial;
                    entry.in_flight = true;
                    (serial, previous, false)
                }
            };
            let _flight = (!independent_join).then(|| FlightGuard {
                inner: self.inner.clone(),
                scope,
                serial,
            });
            let operation = match input {
                AuthorityInput::Renew(_) => wire::authority_request::Operation::Renew(context(
                    &self.inner,
                    &scope.0,
                    previous.as_ref().ok_or(AuthorityError::Unavailable)?,
                )),
                AuthorityInput::Join(room) => {
                    wire::authority_request::Operation::Join(wire::Join {
                        pulse_instance: self.inner.instance.to_vec(),
                        connection_generation: room.nonce[..16].to_vec(),
                        wallet: address_bytes(&room.wallet)
                            .ok_or(AuthorityError::Unavailable)?
                            .to_vec(),
                        session: address_bytes(&room.session)
                            .ok_or(AuthorityError::Unavailable)?
                            .to_vec(),
                        header_payload: room.header_payload,
                        nonce: room.nonce.to_vec(),
                        proof: room.proof,
                    })
                }
            };
            let result = request(&self.inner, operation).await;
            let mut registry = self
                .inner
                .registry
                .lock()
                .map_err(|_| AuthorityError::Unavailable)?;
            let error = match result.as_ref().ok().and_then(|r| r.result.as_ref()) {
                Some(wire::authority_response::Result::Denied(denied))
                    if matches!(
                        wire::denied::Reason::try_from(denied.reason),
                        Ok(wire::denied::Reason::NotAuthorized | wire::denied::Reason::Expired)
                    ) =>
                {
                    AuthorityError::NotAuthorized
                }
                _ => AuthorityError::Unavailable,
            };
            let valid = result
                .ok()
                .and_then(|response| match response.result {
                    Some(wire::authority_response::Result::Lease(lease)) => Some(lease),
                    _ => None,
                })
                .filter(|lease| {
                    lease.is_valid()
                        && lease.ttl_ms > 0
                        && Duration::from_millis(u64::from(lease.ttl_ms)) <= MAX_AUTHORITY_LEASE
                        && started + Duration::from_millis(u64::from(lease.ttl_ms)) > Instant::now()
                        && (!renewal
                            || previous.as_ref().is_none_or(|old| {
                                old.admitted == lease.admitted
                                    && old.lease_id == lease.lease_id
                                    && old.authority_incarnation == lease.authority_incarnation
                            }))
                });
            if independent_join {
                // The new credentials were checked by gatekeeper. Keep renewal
                // ownership and its deadline intact, and retire the redundant lease.
                let Some(lease) = valid else {
                    return Err(error);
                };
                self.release_lease(scope.0, lease.clone());
                let old = previous.as_ref().ok_or(AuthorityError::Unavailable)?;
                let entry = registry
                    .entries
                    .get(&scope)
                    .filter(|entry| {
                        entry.deadline > Instant::now()
                            && entry.lease.as_ref().is_some_and(|current| {
                                current.lease_id == old.lease_id
                                    && current.authority_incarnation == old.authority_incarnation
                                    && current.admitted == old.admitted
                                    && current.admitted == lease.admitted
                            })
                    })
                    .ok_or(AuthorityError::Unavailable)?;
                return Ok(Admission {
                    scope: AdmittedScope {
                        nonce: scope.0,
                        fingerprint: scope.1,
                        admitted: lease.admitted.ok_or(AuthorityError::Unavailable)?,
                    },
                    ttl: entry.deadline.saturating_duration_since(Instant::now()),
                });
            }
            let Some(entry) = registry
                .entries
                .get_mut(&scope)
                .filter(|entry| entry.serial == serial)
            else {
                return Err(AuthorityError::Unavailable);
            };
            if let Some(lease) = valid.filter(|_| entry.deadline > Instant::now()) {
                let ttl = Duration::from_millis(u64::from(lease.ttl_ms));
                entry.deadline = started + ttl;
                let admitted = lease.admitted.clone().ok_or(AuthorityError::Unavailable)?;
                entry.lease = Some(lease);
                entry.in_flight = false;
                Ok(Admission {
                    scope: AdmittedScope {
                        admitted,
                        nonce: scope.0,
                        fingerprint: scope.1,
                    },
                    ttl,
                })
            } else {
                if !renewal && previous.is_some() && entry.deadline > Instant::now() {
                    entry.in_flight = false;
                } else {
                    registry.entries.remove(&scope);
                }
                Err(error)
            }
        })
    }
    fn release(&self, room: AdmittedScope) {
        let lease = self
            .inner
            .registry
            .lock()
            .ok()
            .and_then(|mut registry| registry.entries.remove(&(room.nonce, room.fingerprint)))
            .and_then(|entry| entry.lease);
        let Some(lease) = lease else {
            return;
        };
        self.release_lease(room.nonce, lease);
    }
}

impl HttpAuthority {
    fn release_lease(&self, nonce: [u8; 32], lease: wire::Lease) {
        let Ok(permit) = self.inner.permits.clone().try_acquire_owned() else {
            return;
        };
        let inner = self.inner.clone();
        tokio::spawn(async move {
            let _permit = permit;
            let operation =
                wire::authority_request::Operation::Release(context(&inner, &nonce, &lease));
            let _ = request(&inner, operation).await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pulse::application_relay::PendingJoin;

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    fn credential() -> AuthorityInput {
        AuthorityInput::Join(PendingJoin {
            wallet: "0x1111111111111111111111111111111111111111".into(),
            session: "0x2222222222222222222222222222222222222222".into(),
            header_payload: "header.payload".into(),
            nonce: [3; 32],
            proof: vec![4; 32],
        })
    }
    fn admitted() -> AdmittedScope {
        AdmittedScope {
            admitted: wire::AdmittedRoom {
                room: "scene".into(),
                identity: "0x1111111111111111111111111111111111111111".into(),
                is_guest: Some(true),
            },
            nonce: [3; 32],
            fingerprint: credential().key().1,
        }
    }
    fn lease(id: u8, ttl_ms: u32) -> Vec<u8> {
        wire::AuthorityResponse {
            result: Some(wire::authority_response::Result::Lease(wire::Lease {
                lease_id: vec![id; 16],
                authority_incarnation: vec![8; 16],
                ttl_ms,
                admitted: Some(admitted().admitted),
            })),
        }
        .encode_to_vec()
    }
    /// When the fixture answers a request it has read.
    enum Answer {
        Now,
        Held(
            tokio::sync::oneshot::Sender<()>,
            tokio::sync::oneshot::Receiver<()>,
        ),
        /// Once the test sends on (or drops) the paired sender.
        After(tokio::sync::oneshot::Receiver<()>),
        /// Never: it signals that it holds the request, then waits for the client to hang up.
        Never(tokio::sync::oneshot::Sender<()>),
    }
    async fn fixture(
        responses: Vec<(u16, Vec<u8>, Answer)>,
    ) -> (
        HttpAuthority,
        tokio::task::JoinHandle<Vec<wire::AuthorityRequest>>,
    ) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}{AUTHORITY_PATH}", listener.local_addr().unwrap());
        let handle = tokio::spawn(async move {
            let mut seen = Vec::new();
            let mut nonces = std::collections::HashSet::new();
            let mut answers = Vec::new();
            for (status, response, answer) in responses {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut bytes = Vec::new();
                let mut buffer = [0; 4096];
                let split = loop {
                    let n = stream.read(&mut buffer).await.unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&buffer[..n]);
                    if let Some(i) = bytes.windows(4).position(|v| v == b"\r\n\r\n") {
                        break i + 4;
                    }
                };
                let headers = String::from_utf8(bytes[..split].to_vec()).unwrap();
                assert!(headers.starts_with(&format!("POST {AUTHORITY_PATH} HTTP/1.1\r\n")));
                let header = |name: &str| {
                    headers
                        .lines()
                        .find_map(|line| {
                            line.split_once(':')
                                .filter(|(key, _)| key.eq_ignore_ascii_case(name))
                                .map(|(_, v)| v.trim().to_owned())
                        })
                        .unwrap()
                };
                let len: usize = header("content-length").parse().unwrap();
                while bytes.len() - split < len {
                    let n = stream.read(&mut buffer).await.unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&buffer[..n]);
                }
                let body = &bytes[split..split + len];
                assert_eq!(header("content-type"), "application/x-protobuf");
                assert_eq!(
                    header("x-pulse-authority-signature"),
                    signed_headers(
                        &[5; 32],
                        body,
                        &header("x-pulse-authority-timestamp"),
                        &header("x-pulse-authority-nonce")
                    )
                    .unwrap()
                );
                assert!(nonces.insert(header("x-pulse-authority-nonce")));
                seen.push(wire::AuthorityRequest::decode(body).unwrap());
                answers.push(tokio::spawn(async move {
                match answer {
                    Answer::Now => {}
                    Answer::Held(held, release) => {
                        let _ = held.send(());
                        let _ = release.await;
                    }
                    Answer::After(release) => {
                        let _ = release.await;
                    }
                    Answer::Never(held) => {
                        let _ = held.send(());
                        while stream.read(&mut buffer).await.is_ok_and(|n| n > 0) {}
                        return;
                    }
                }
                let headers = format!(
                    "HTTP/1.1 {status} fixture\r\nContent-Length: {}\r\nContent-Type: application/x-protobuf\r\nLocation: http://127.0.0.1:1/forbidden\r\nConnection: close\r\n\r\n",
                    response.len()
                );
                let _ = stream.write_all(headers.as_bytes()).await;
                let _ = stream.write_all(&response).await;
                }));
            }
            for answer in answers {
                answer.await.unwrap();
            }
            seen
        });
        (
            HttpAuthority::new(&endpoint, &URL_SAFE_NO_PAD.encode([5; 32]), true).unwrap(),
            handle,
        )
    }
    #[tokio::test]
    async fn protobuf_hmac_join_renew_release_preserve_exact_context() {
        let released = wire::AuthorityResponse {
            result: Some(wire::authority_response::Result::Released(
                wire::Released {},
            )),
        }
        .encode_to_vec();
        let (authority, server) = fixture(vec![
            (200, lease(7, 2000), Answer::Now),
            (200, lease(7, 2000), Answer::Now),
            (200, released, Answer::Now),
        ])
        .await;
        let room = credential();
        assert_eq!(
            authority.authorize(room.clone()).await,
            Ok(Admission {
                scope: admitted(),
                ttl: Duration::from_secs(2)
            })
        );
        assert_eq!(
            authority.authorize(AuthorityInput::Renew(admitted())).await,
            Ok(Admission {
                scope: admitted(),
                ttl: Duration::from_secs(2)
            })
        );
        authority.release(admitted());
        let seen = server.await.unwrap();
        let Some(wire::authority_request::Operation::Join(join)) = &seen[0].operation else {
            panic!()
        };
        let Some(wire::authority_request::Operation::Renew(renew)) = &seen[1].operation else {
            panic!()
        };
        let Some(wire::authority_request::Operation::Release(release)) = &seen[2].operation else {
            panic!()
        };
        assert_eq!(join.header_payload, "header.payload");
        assert_eq!(join.proof, vec![4; 32]);
        assert_eq!(join.pulse_instance, renew.pulse_instance);
        assert_eq!(join.connection_generation, renew.connection_generation);
        assert_eq!(renew, release);
        assert_eq!(renew.lease_id, vec![7; 16]);
    }
    #[tokio::test]
    async fn changed_lease_denial_redirect_malformed_oversize_and_timeout_fail_closed() {
        let (authority, server) = fixture(vec![
            (200, lease(7, 2000), Answer::Now),
            (200, lease(9, 2000), Answer::Now),
        ])
        .await;
        assert!(authority.authorize(credential()).await.is_ok());
        assert!(
            authority
                .authorize(AuthorityInput::Renew(admitted()))
                .await
                .is_err()
        );
        assert!(
            authority
                .authorize(AuthorityInput::Renew(admitted()))
                .await
                .is_err()
        );
        server.await.unwrap();
        for (status, body) in [
            (302, lease(7, 2000)),
            (200, vec![255]),
            (200, vec![0; MAX_BODY + 1]),
            (200, lease(7, 2001)),
        ] {
            let (authority, server) = fixture(vec![(status, body, Answer::Now)]).await;
            assert!(authority.authorize(credential()).await.is_err());
            server.await.unwrap();
        }

        // The authority never answers. Once it holds the request the 750 ms budget runs out
        // on a clock the test moves; a request still in flight when real time spends the
        // budget has timed out the same way.
        let (held, holding) = tokio::sync::oneshot::channel();
        let (authority, server) = fixture(vec![(200, lease(7, 2000), Answer::Never(held))]).await;
        let mut join = std::pin::pin!(authority.authorize(credential()));
        let timed_out = tokio::select! {
            ended = &mut join => ended,
            _ = holding => {
                tokio::time::pause();
                tokio::time::advance(Duration::from_millis(750)).await;
                tokio::time::resume();
                join.await
            }
        };
        assert!(timed_out.is_err());
        server.await.unwrap();
    }
    #[tokio::test]
    async fn release_during_pending_join_prevents_late_result_from_recreating_lease() {
        let (answer, answer_held) = tokio::sync::oneshot::channel();
        let (authority, server) =
            fixture(vec![(200, lease(7, 2000), Answer::After(answer_held))]).await;
        let copy = authority.clone();
        let job = tokio::spawn(async move { copy.authorize(credential()).await });
        while authority.inner.registry.lock().unwrap().entries.is_empty() {
            tokio::task::yield_now().await;
        }
        authority.release(admitted());
        // The lease arrives only after the release.
        answer.send(()).unwrap();
        assert!(job.await.unwrap().is_err());
        assert!(authority.inner.registry.lock().unwrap().entries.is_empty());
        server.await.unwrap();
    }
    #[tokio::test]
    async fn missing_or_changed_projection_and_wrong_variants_never_admit() {
        let valid = wire::AuthorityResponse::decode(lease(7, 2000).as_slice()).unwrap();
        let Some(wire::authority_response::Result::Lease(base)) = valid.result else {
            panic!()
        };
        let mut invalid = Vec::new();
        let mut missing = base.clone();
        missing.admitted = None;
        invalid.push(missing);
        let mut guest = base.clone();
        guest.admitted.as_mut().unwrap().is_guest = None;
        invalid.push(guest);
        let mut identity = base.clone();
        identity.admitted.as_mut().unwrap().identity = "untrusted".into();
        invalid.push(identity);
        let mut room = base.clone();
        room.admitted.as_mut().unwrap().room = "x".repeat(257);
        invalid.push(room);
        for lease in invalid {
            let bytes = wire::AuthorityResponse {
                result: Some(wire::authority_response::Result::Lease(lease)),
            }
            .encode_to_vec();
            let (authority, server) = fixture(vec![(200, bytes, Answer::Now)]).await;
            assert_eq!(
                authority.authorize(credential()).await,
                Err(AuthorityError::Unavailable)
            );
            assert!(authority.inner.registry.lock().unwrap().entries.is_empty());
            server.await.unwrap();
        }
        for reason in [0, 1, 2, 3, 4, 5, 99] {
            let bytes = wire::AuthorityResponse {
                result: Some(wire::authority_response::Result::Denied(wire::Denied {
                    reason,
                })),
            }
            .encode_to_vec();
            let (authority, server) = fixture(vec![(200, bytes, Answer::Now)]).await;
            let error = if matches!(reason, 1 | 2) {
                AuthorityError::NotAuthorized
            } else {
                AuthorityError::Unavailable
            };
            assert_eq!(authority.authorize(credential()).await, Err(error));
            server.await.unwrap();
        }
        let mut changed = base;
        changed.admitted.as_mut().unwrap().is_guest = Some(false);
        let bytes = wire::AuthorityResponse {
            result: Some(wire::authority_response::Result::Lease(changed)),
        }
        .encode_to_vec();
        let (authority, server) = fixture(vec![
            (200, lease(7, 2000), Answer::Now),
            (200, bytes, Answer::Now),
        ])
        .await;
        assert!(authority.authorize(credential()).await.is_ok());
        assert_eq!(
            authority.authorize(AuthorityInput::Renew(admitted())).await,
            Err(AuthorityError::Unavailable)
        );
        server.await.unwrap();
        for result in [
            None,
            Some(wire::authority_response::Result::Released(
                wire::Released {},
            )),
        ] {
            let bytes = wire::AuthorityResponse { result }.encode_to_vec();
            let (authority, server) = fixture(vec![(200, bytes, Answer::Now)]).await;
            assert_eq!(
                authority.authorize(credential()).await,
                Err(AuthorityError::Unavailable)
            );
            server.await.unwrap();
        }
    }

    #[tokio::test]
    async fn fresh_join_releases_redundant_lease_and_denied_join_preserves_renewal() {
        let denied = wire::AuthorityResponse {
            result: Some(wire::authority_response::Result::Denied(wire::Denied {
                reason: wire::denied::Reason::NotAuthorized as i32,
            })),
        }
        .encode_to_vec();
        let (authority, server) = fixture(vec![
            (200, lease(7, 2000), Answer::Now),
            (200, denied, Answer::Now),
            (200, lease(7, 2000), Answer::Now),
        ])
        .await;
        assert!(authority.authorize(credential()).await.is_ok());
        let mut changed = credential();
        if let AuthorityInput::Join(join) = &mut changed {
            join.proof[0] ^= 1;
        }
        assert_eq!(
            authority.authorize(changed).await,
            Err(AuthorityError::NotAuthorized)
        );
        assert!(
            authority
                .authorize(AuthorityInput::Renew(admitted()))
                .await
                .is_ok()
        );
        let seen = server.await.unwrap();
        assert!(
            matches!(&seen[1].operation, Some(wire::authority_request::Operation::Join(join)) if join.proof[0] == 5)
        );
        assert!(
            matches!(&seen[2].operation, Some(wire::authority_request::Operation::Renew(ctx)) if ctx.lease_id == vec![7;16])
        );

        let released = wire::AuthorityResponse {
            result: Some(wire::authority_response::Result::Released(
                wire::Released {},
            )),
        }
        .encode_to_vec();
        let (authority, server) = fixture(vec![
            (200, lease(7, 2000), Answer::Now),
            (200, lease(8, 2000), Answer::Now),
            (200, released, Answer::Now),
        ])
        .await;
        assert!(authority.authorize(credential()).await.is_ok());
        assert!(authority.authorize(credential()).await.is_ok());
        let seen = server.await.unwrap();
        assert!(matches!(
            &seen[1].operation,
            Some(wire::authority_request::Operation::Join(_))
        ));
        assert!(
            matches!(&seen[2].operation, Some(wire::authority_request::Operation::Release(ctx)) if ctx.lease_id == vec![8;16])
        );
        assert_eq!(
            authority
                .inner
                .registry
                .lock()
                .unwrap()
                .entries
                .values()
                .next()
                .unwrap()
                .lease
                .as_ref()
                .unwrap()
                .lease_id,
            vec![7; 16]
        );
    }

    #[tokio::test]
    async fn held_fresh_join_does_not_retire_membership_when_renewal_is_due() {
        use super::super::ApplicationRelay;
        use crate::protocol::pulse::decentraland::pulse::{
            ApplicationErrorCode, ApplicationJoin, server_message,
        };
        for valid in [false, true] {
            let (held, holding) = tokio::sync::oneshot::channel();
            let (answer, answering) = tokio::sync::oneshot::channel();
            let denied = wire::AuthorityResponse {
                result: Some(wire::authority_response::Result::Denied(wire::Denied {
                    reason: wire::denied::Reason::NotAuthorized as i32,
                })),
            }
            .encode_to_vec();
            let mut responses = vec![
                (200, lease(7, 2000), Answer::Now),
                (
                    200,
                    if valid { lease(8, 2000) } else { denied },
                    Answer::Held(held, answering),
                ),
                (200, lease(7, 2000), Answer::Now),
            ];
            if valid {
                responses.push((
                    200,
                    wire::AuthorityResponse {
                        result: Some(wire::authority_response::Result::Released(
                            wire::Released {},
                        )),
                    }
                    .encode_to_vec(),
                    Answer::Now,
                ));
            }
            let (authority, server) = fixture(responses).await;
            let mut relay = ApplicationRelay::new(Arc::new(authority));
            relay.authenticated(
                1,
                [3; 32],
                admitted().admitted.identity,
                "0x2222222222222222222222222222222222222222".into(),
            );
            let join = ApplicationJoin {
                request_id: 1,
                jwt_header_payload: "header.payload".into(),
                proof: vec![4; 32],
            };
            assert!(relay.join(1, join.clone(), 0).is_empty());
            relay.completed().await;
            assert!(relay.state.holds_scope(1));
            let mut retry = join;
            retry.request_id = 2;
            if !valid {
                retry.proof[0] ^= 1;
            }
            assert!(relay.join(1, retry, 0).is_empty());
            holding.await.unwrap();
            relay.schedule_renewals(Instant::now() + Duration::from_secs(1));
            let (out, _) = tokio::time::timeout(Duration::from_millis(500), relay.completed())
                .await
                .unwrap();
            assert!(out.is_empty());
            assert!(relay.state.holds_scope(1));
            answer.send(()).unwrap();
            let (out, _) = relay.completed().await;
            let server_message::Message::ApplicationJoinResult(result) =
                out[0].message.message.as_ref().unwrap()
            else {
                panic!()
            };
            assert_eq!(
                result.error,
                if valid {
                    0
                } else {
                    ApplicationErrorCode::ApplicationNotAuthorized as i32
                }
            );
            assert!(relay.state.holds_scope(1));
            let seen = server.await.unwrap();
            assert!(
                matches!(&seen[2].operation, Some(wire::authority_request::Operation::Renew(ctx)) if ctx.lease_id == vec![7;16])
            );
            if valid {
                assert!(
                    matches!(&seen[3].operation, Some(wire::authority_request::Operation::Release(ctx)) if ctx.lease_id == vec![8;16])
                );
            }
        }
    }

    #[tokio::test]
    async fn dropped_and_outer_timeout_fresh_join_leave_established_lease_renewable() {
        for outer_timeout in [false, true] {
            let (held, holding) = tokio::sync::oneshot::channel();
            let (answer, answering) = tokio::sync::oneshot::channel();
            let (authority, server) = fixture(vec![
                (200, lease(7, 2000), Answer::Now),
                (200, lease(8, 2000), Answer::Held(held, answering)),
                (200, lease(7, 2000), Answer::Now),
            ])
            .await;
            assert!(authority.authorize(credential()).await.is_ok());
            let mut pending = Box::pin(tokio::time::timeout(
                Duration::from_millis(if outer_timeout { 750 } else { 5000 }),
                async {
                    // The outer relay budget begins before the HTTP work starts.
                    // Ensure its cancellation wins over reqwest's inner 750 ms timer.
                    if outer_timeout {
                        tokio::time::sleep(Duration::from_millis(25)).await;
                    }
                    authority.authorize(credential()).await
                },
            ));
            tokio::select! {
                _ = holding => {},
                result = &mut pending => panic!("held join completed: {result:?}"),
            }
            if outer_timeout {
                assert!(pending.await.is_err());
            } else {
                drop(pending);
            }
            assert!(
                authority
                    .authorize(AuthorityInput::Renew(admitted()))
                    .await
                    .is_ok()
            );
            drop(answer);
            let seen = server.await.unwrap();
            assert!(
                matches!(&seen[2].operation, Some(wire::authority_request::Operation::Renew(ctx)) if ctx.lease_id == vec![7;16])
            );
        }
    }

    struct AllowPolicy;
    #[async_trait::async_trait]
    impl crate::gatekeeper::admission::relay_authority::RelayPolicy for AllowPolicy {
        async fn authorize(
            &self,
            _: &crate::gatekeeper::admission::relay_authority::proof::VerifiedRoom,
        ) -> Result<(), wire::denied::Reason> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn capacity_rejected_join_cannot_first_admit_from_warm_cache_after_jwt_expiry() {
        use super::super::{ApplicationRelay, state};
        use crate::gatekeeper::admission::relay_authority::{self, RelayAuthority};
        use crate::protocol::pulse::decentraland::pulse::{
            ApplicationErrorCode, ApplicationJoin, server_message,
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}{AUTHORITY_PATH}", listener.local_addr().unwrap());
        let server_authority = Arc::new(
            RelayAuthority::new(Arc::new(AllowPolicy), "key".into(), vec![7; 32], [5; 32]).unwrap(),
        );
        let server = tokio::spawn(async move {
            axum::serve(listener, relay_authority::router::<()>(server_authority))
                .await
                .unwrap();
        });
        let authority = Arc::new(
            HttpAuthority::new(&endpoint, &URL_SAFE_NO_PAD.encode([5; 32]), true).unwrap(),
        );
        let mut relay = ApplicationRelay::new(authority.clone());
        let now = Instant::now();
        for peer in 1..=state::MAX_ROOM_PEERS as u32 {
            let mut scope = admitted();
            scope.admitted.identity = format!("0x{peer:040x}");
            relay
                .state
                .join(peer, 1, scope, now + Duration::from_secs(10), now);
        }
        assert_eq!(relay.state.memberships.len(), state::MAX_ROOM_PEERS);
        let expiry = chrono::Utc::now().timestamp() + 2;
        let claims = serde_json::json!({"iss":"key", "sub":admitted().admitted.identity,
            "exp":expiry, "video":{"room":"scene","roomJoin":true,"canPublishData":true}});
        let hp = format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(br#"{"alg":"HS256"}"#),
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap())
        );
        let mut signature = Hmac::<Sha256>::new_from_slice(&[7; 32]).unwrap();
        signature.update(hp.as_bytes());
        let mut proof = Hmac::<Sha256>::new_from_slice(&signature.finalize().into_bytes()).unwrap();
        proof.update(wire::PROOF_DOMAIN);
        proof.update(&[3; 32]);
        proof.update(&[0x11; 20]);
        proof.update(&[0x22; 20]);
        proof.update(&Sha256::digest(hp.as_bytes()));
        let join = ApplicationJoin {
            request_id: 1,
            jwt_header_payload: hp,
            proof: proof.finalize().into_bytes().to_vec(),
        };
        relay.authenticated(
            1000,
            [3; 32],
            admitted().admitted.identity,
            "0x2222222222222222222222222222222222222222".into(),
        );
        assert!(relay.join(1000, join.clone(), 0).is_empty());
        // A duplicate pending ID stays a noop even when its supplied proof differs.
        let mut duplicate = join.clone();
        duplicate.proof[0] ^= 1;
        assert!(relay.join(1000, duplicate, 0).is_empty());
        let (out, _) = relay.completed().await;
        let server_message::Message::ApplicationJoinResult(result) =
            out[0].message.message.as_ref().unwrap()
        else {
            panic!()
        };
        assert_eq!(result.error, ApplicationErrorCode::ApplicationLimit as i32);
        assert!(!relay.state.holds_scope(1000));
        assert_eq!(authority.inner.registry.lock().unwrap().entries.len(), 1);
        relay.state.disconnect(1);
        while chrono::Utc::now().timestamp() < expiry {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        // JWT expiry and the cache lease use different clocks. Re-arm the cache
        // after the real expiry wait so that wait cannot consume the warm-cache
        // window. Preserve the original lease and the normal lease duration.
        {
            let mut registry = authority.inner.registry.lock().unwrap();
            assert_eq!(registry.entries.len(), 1);
            let entry = registry.entries.values_mut().next().unwrap();
            assert!(entry.lease.is_some());
            assert!(!entry.in_flight);
            entry.deadline = Instant::now() + MAX_AUTHORITY_LEASE;
        }
        assert!(
            authority
                .inner
                .registry
                .lock()
                .unwrap()
                .entries
                .values()
                .all(|entry| entry.deadline > Instant::now())
        );
        let mut retry = join;
        retry.request_id = 2;
        assert!(relay.join(1000, retry, 0).is_empty());
        let (out, _) = relay.completed().await;
        let server_message::Message::ApplicationJoinResult(result) =
            out[0].message.message.as_ref().unwrap()
        else {
            panic!()
        };
        assert_eq!(
            result.error,
            ApplicationErrorCode::ApplicationNotAuthorized as i32
        );
        assert!(!relay.state.holds_scope(1000));
        server.abort();
    }

    #[test]
    fn insecure_urls_and_wrong_service_keys_are_rejected() {
        let key = URL_SAFE_NO_PAD.encode([5; 32]);
        assert!(
            HttpAuthority::new(&format!("http://127.0.0.1{AUTHORITY_PATH}"), &key, false).is_err()
        );
        assert!(
            HttpAuthority::new(&format!("http://192.0.2.1{AUTHORITY_PATH}"), &key, true).is_err()
        );
        assert!(HttpAuthority::new("https://example.test/wrong", &key, false).is_err());
        assert!(
            HttpAuthority::new(
                &format!("https://example.test{AUTHORITY_PATH}"),
                "short",
                false
            )
            .is_err()
        );
    }
}
