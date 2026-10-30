//! The `/about` `comms` block. The old server probed two loopback HTTP
//! services for it (`COMMS_WS_CONNECTOR_URL`/status, `COMMS_STATS_URL`/core-status),
//! both archipelago; archipelago is a module of this process now, so the
//! probe is a module call ([`CommsCore`]): the ws-connector answers because
//! this process answers, and the core status is the clustering engine's last
//! `engine.discovery` heartbeat (`catalyst pulse`, over NATS). The block is
//! advertised when `COMMS_FIXED_ADAPTER` is set (the old deployment set it
//! beside the two probe URLs on every node).

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::jobs::{Jobs, PeriodicCfg};

pub const OFFLINE_ADAPTER: &str = "offline:offline";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AboutComms {
    pub healthy: bool,
    pub protocol: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub users_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adapter: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fixed_adapter: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub v4: Option<Discovery>,
}

/// What the clustering engine last reported: archipelago's `/core-status`.
pub trait CommsCore: Send + Sync {
    /// `(healthy, userCount)`.
    fn core_status(&self) -> (bool, u64);
    /// Peers per parcel, for `/lambdas/explore/realms`.
    fn parcel_counts(&self) -> Vec<((i32, i32), u32)> {
        Vec::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommsProbe {
    pub healthy: bool,
    pub user_count: u64,
}

/// The `COMMS_*` settings of the block.
#[derive(Clone, Debug)]
pub struct Settings {
    pub protocol: String,
    pub fixed_adapter: String,
    pub version: Option<String>,
    pub commit_hash: Option<String>,
    pub v4: Option<Discovery>,
    pub max_users: Option<u64>,
    pub offline_when_unreachable: bool,
}

pub struct RawSettings<'a> {
    pub protocol: &'a str,
    pub fixed_adapter: Option<&'a str>,
    pub version: Option<&'a str>,
    pub commit_hash: Option<&'a str>,
    pub max_users: Option<u64>,
    pub offline_when_unreachable: bool,
    pub v4_control_url: Option<&'a str>,
    pub v4_control_audience: Option<&'a str>,
    pub v4_pulse_native_endpoint: Option<&'a str>,
    pub v4_pulse_webtransport_url: Option<&'a str>,
    pub v4_pulse_certificate_hash: Option<&'a str>,
    pub v4_pulse_audience: Option<&'a str>,
    pub v4_island_refresh_url: Option<&'a str>,
}

fn present(v: Option<&str>) -> Option<String> {
    v.map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

impl Settings {
    /// `None` when `COMMS_FIXED_ADAPTER` is unset: no block, as the old
    /// server without its probe URLs.
    pub fn resolve(raw: RawSettings<'_>) -> Option<Self> {
        let fixed_adapter = present(raw.fixed_adapter)?;
        Some(Self {
            protocol: present(Some(raw.protocol)).unwrap_or_else(|| "v3".to_string()),
            fixed_adapter,
            version: present(raw.version),
            commit_hash: present(raw.commit_hash),
            // Public V4 authentication is deferred. Stale COMMS_V4 settings
            // cannot advertise endpoints absent from this compatibility branch.
            v4: None,
            max_users: raw.max_users,
            offline_when_unreachable: raw.offline_when_unreachable,
        })
    }
}

/// The comms half of `/about`: the settings, the in-process core, the SFU
/// liveness of the advertised adapter.
pub struct AboutCommsState {
    pub settings: Settings,
    core: OnceLock<Arc<dyn CommsCore>>,
    pub sfu: SfuHealth,
}

impl AboutCommsState {
    pub fn new(settings: Settings) -> Self {
        let sfu = if settings.offline_when_unreachable {
            match probe_target(&settings.fixed_adapter) {
                Some(target) => SfuHealth::probing(target),
                None => SfuHealth::always_alive(),
            }
        } else {
            SfuHealth::always_alive()
        };
        Self {
            settings,
            core: OnceLock::new(),
            sfu,
        }
    }

    /// Attaches archipelago once it exists (it is built after the content
    /// router's state); `false` if one was attached already.
    pub fn attach_core(&self, core: Arc<dyn CommsCore>) -> bool {
        self.core.set(core).is_ok()
    }

    /// The probe the old server made over HTTP: without a core attached the
    /// ws-connector would not have answered, so the block reads unhealthy.
    pub fn probe(&self) -> CommsProbe {
        match self.core.get() {
            Some(core) => {
                let (healthy, user_count) = core.core_status();
                CommsProbe {
                    healthy,
                    user_count,
                }
            }
            None => CommsProbe {
                healthy: false,
                user_count: 0,
            },
        }
    }

    pub fn parcel_counts(&self) -> Vec<((i32, i32), u32)> {
        self.core
            .get()
            .map(|core| core.parcel_counts())
            .unwrap_or_default()
    }

    pub fn under_capacity(&self, probe: CommsProbe) -> bool {
        match self.settings.max_users {
            Some(max) => probe.user_count < max,
            None => true,
        }
    }

    /// The SFU probe loop, one periodic job over the ONE client's raw half
    /// (the target is the operator's own adapter, loopback included).
    pub fn spawn_jobs(self: &Arc<Self>, jobs: &Jobs, client: reqwest::Client) {
        let Some(target) = self.sfu.probe_url.clone() else {
            return;
        };
        let health = self.sfu.clone();
        let strikes = Arc::new(AtomicU64::new(0));
        jobs.spawn_periodic(
            "about-comms-sfu-probe",
            PeriodicCfg::every(SFU_PROBE_INTERVAL),
            move |_ctx| {
                let client = client.clone();
                let target = target.clone();
                let health = health.clone();
                let strikes = strikes.clone();
                async move {
                    let reachable = client
                        .get(&*target)
                        .timeout(SFU_PROBE_TIMEOUT)
                        .send()
                        .await
                        .is_ok();
                    health.record(reachable, &strikes);
                    Ok(())
                }
            },
        );
    }
}

pub fn build_comms_config(probe: CommsProbe, settings: &Settings, online: bool) -> AboutComms {
    let adapter = if online {
        settings.fixed_adapter.clone()
    } else {
        OFFLINE_ADAPTER.to_string()
    };
    AboutComms {
        healthy: probe.healthy,
        protocol: settings.protocol.clone(),
        version: settings.version.clone(),
        commit_hash: settings.commit_hash.clone(),
        users_count: Some(probe.user_count),
        adapter: if adapter.is_empty() {
            None
        } else {
            Some(format!("fixed-adapter:{adapter}"))
        },
        fixed_adapter: if adapter.is_empty() {
            None
        } else {
            Some(adapter)
        },
        // Also suppress explicitly constructed settings at the response boundary.
        v4: None,
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Discovery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub control: Option<ControlEndpoint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pulse: Option<PulseEndpoint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub island_refresh_url: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlEndpoint {
    pub url: String,
    pub audience: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PulseEndpoint {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub native_endpoint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web_transport_url: Option<String>,
    pub audience: String,
    /// Hex SHA-256 of a self-signed WebTransport certificate, pinned by browsers through
    /// `serverCertificateHashes`; advertised only beside a WebTransport URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub certificate_hash: Option<String>,
}

#[cfg(test)]
impl Discovery {
    fn read(get: impl Fn(&str) -> Option<String>) -> Option<Self> {
        let control = get("COMMS_V4_CONTROL_URL").and_then(|endpoint| {
            Some(ControlEndpoint {
                url: public_url(endpoint, &["wss"], &["ws"])?,
                audience: audience(get("COMMS_V4_CONTROL_AUDIENCE")?)?,
            })
        });
        let native_endpoint = get("COMMS_V4_PULSE_NATIVE_ENDPOINT").and_then(native_endpoint);
        let web_transport_url = get("COMMS_V4_PULSE_WEBTRANSPORT_URL")
            .and_then(|endpoint| public_url(endpoint, &["https"], &[]));
        let certificate_hash = web_transport_url
            .as_ref()
            .and(get("COMMS_V4_PULSE_CERTIFICATE_HASH"))
            .filter(|hash| hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()))
            .map(|hash| hash.to_ascii_lowercase());
        let pulse = if native_endpoint.is_some() || web_transport_url.is_some() {
            get("COMMS_V4_PULSE_AUDIENCE")
                .and_then(audience)
                .map(|audience| PulseEndpoint {
                    native_endpoint,
                    web_transport_url,
                    audience,
                    certificate_hash,
                })
        } else {
            None
        };
        let island_refresh_url = get("COMMS_V4_ISLAND_REFRESH_URL")
            .and_then(|endpoint| public_url(endpoint, &["https"], &["http"]));
        (control.is_some() || pulse.is_some()).then_some(Self {
            control,
            pulse,
            island_refresh_url,
        })
    }
}

#[cfg(test)]
fn audience(value: String) -> Option<String> {
    (!value.is_empty()
        && value.len() <= 256
        && value.trim() == value
        && !value.chars().any(char::is_control))
    .then_some(value)
}

#[cfg(test)]
fn public_url(value: String, secure: &[&str], loopback: &[&str]) -> Option<String> {
    let url = url::Url::parse(&value).ok()?;
    let host = url.host_str()?;
    let is_loopback = host == "localhost"
        || host
            .trim_matches(['[', ']'])
            .parse::<std::net::IpAddr>()
            .is_ok_and(|address| address.is_loopback());
    (value.len() <= 2048
        && value.trim() == value
        && !value.chars().any(char::is_control)
        && (secure.contains(&url.scheme()) || (is_loopback && loopback.contains(&url.scheme())))
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none())
    .then_some(value)
}

#[cfg(test)]
fn native_endpoint(value: String) -> Option<String> {
    let url = url::Url::parse(&format!("quic://{value}")).ok()?;
    (value.len() <= 512
        && !value.chars().any(char::is_whitespace)
        && url.host_str().is_some()
        && url.port().is_some_and(|port| port != 0)
        && url.username().is_empty()
        && url.password().is_none()
        && url.path().is_empty()
        && url.query().is_none()
        && url.fragment().is_none())
    .then_some(value)
}

/// Liveness of the endpoint the realm advertises, so `/about` stops pointing
/// clients at comms that will not answer: the desktop client treats the
/// handshake as a hard gate on entry, so advertising `offline:offline`
/// while it is down lets visitors in with comms off. Starts alive and needs
/// [`STRIKES_TO_DOWN`] consecutive failures to flip.
const SFU_PROBE_INTERVAL: Duration = Duration::from_secs(10);
const SFU_PROBE_TIMEOUT: Duration = Duration::from_secs(2);
const STRIKES_TO_DOWN: u64 = 3;

#[derive(Clone)]
pub struct SfuHealth {
    alive: Arc<AtomicBool>,
    down_since_secs: Arc<AtomicU64>,
    /// The origin only, for diagnostics (never the path or credentials).
    target: Arc<str>,
    probe_url: Option<Arc<str>>,
}

// The probe URL may carry credentials, so only the sanitized target prints.
impl std::fmt::Debug for SfuHealth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SfuHealth")
            .field("target", &self.target)
            .field("alive", &self.is_alive())
            .finish()
    }
}

impl SfuHealth {
    pub fn always_alive() -> Self {
        Self {
            alive: Arc::new(AtomicBool::new(true)),
            down_since_secs: Arc::new(AtomicU64::new(0)),
            target: Arc::from(""),
            probe_url: None,
        }
    }

    fn probing(target: String) -> Self {
        Self {
            alive: Arc::new(AtomicBool::new(true)),
            down_since_secs: Arc::new(AtomicU64::new(0)),
            target: Arc::from(diagnostic_target(&target)),
            probe_url: Some(Arc::from(target)),
        }
    }

    /// A probed endpoint already counted down.
    #[cfg(any(test, feature = "test-support"))]
    pub fn down_for_tests() -> Self {
        let health = Self::probing("https://sfu.example.invalid".into());
        health.alive.store(false, Ordering::Relaxed);
        health
    }

    pub fn is_alive(&self) -> bool {
        self.probe_url.is_none() || self.alive.load(Ordering::Relaxed)
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn down_for_secs(&self) -> Option<u64> {
        if self.is_alive() {
            return None;
        }
        Some(now_secs().saturating_sub(self.down_since_secs.load(Ordering::Relaxed)))
    }

    fn record(&self, reachable: bool, strikes: &AtomicU64) {
        if reachable {
            strikes.store(0, Ordering::Relaxed);
            if !self.alive.swap(true, Ordering::Relaxed) {
                tracing::info!(target = %self.target, "SFU is answering again; comms restored");
            }
        } else {
            let n = strikes.fetch_add(1, Ordering::Relaxed).saturating_add(1);
            if n >= STRIKES_TO_DOWN && self.alive.swap(false, Ordering::Relaxed) {
                self.down_since_secs.store(now_secs(), Ordering::Relaxed);
                tracing::warn!(
                    target = %self.target,
                    strikes = n,
                    "SFU unreachable: /about now advertises fixed-adapter:offline:offline so visitors can still enter, with comms off"
                );
            }
        }
    }
}

fn diagnostic_target(target: &str) -> String {
    reqwest::Url::parse(target)
        .ok()
        .filter(|url| matches!(url.scheme(), "http" | "https"))
        .map(|url| url.origin().ascii_serialization())
        .unwrap_or_else(|| "configured-sfu".to_string())
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// A LiveKit signalling URL (`wss://host`, `ws://host:7880`), a bare host, or
/// a `signed-login:`/`fixed-adapter:` adapter string pointing at a
/// gatekeeper; an `archipelago:` adapter (the ws-connector of this process)
/// or `offline:offline` has nothing to probe.
pub fn probe_target(raw: &str) -> Option<String> {
    let raw = raw.trim();
    let raw = raw
        .strip_prefix("fixed-adapter:")
        .unwrap_or(raw)
        .trim_start_matches("signed-login:")
        .trim();
    if raw.is_empty() || raw == OFFLINE_ADAPTER {
        return None;
    }
    let url = match raw.split_once("://") {
        Some(("wss", rest)) => format!("https://{rest}"),
        Some(("ws", rest)) => format!("http://{rest}"),
        Some(("https", rest)) => format!("https://{rest}"),
        Some(("http", rest)) => format!("http://{rest}"),
        Some(_) => return None,
        None => format!("https://{raw}"),
    };
    Some(url.trim_end_matches('/').to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(fixed_adapter: &str) -> Settings {
        Settings {
            protocol: "v3".into(),
            fixed_adapter: fixed_adapter.into(),
            version: None,
            commit_hash: None,
            v4: None,
            max_users: None,
            offline_when_unreachable: true,
        }
    }

    fn read(values: &[(&str, &str)]) -> Option<Discovery> {
        Discovery::read(|key| {
            values
                .iter()
                .find(|(name, _)| *name == key)
                .map(|(_, value)| (*value).into())
        })
    }

    #[test]
    fn advertised_adapter_preserves_the_transport_after_unwrapping() {
        for (configured, online, expected) in [
            (
                "archipelago:wss://realm.example/ws",
                true,
                Some("archipelago:wss://realm.example/ws"),
            ),
            (
                "livekit:wss://realm.example?token=test",
                true,
                Some("livekit:wss://realm.example?token=test"),
            ),
            (
                "archipelago:wss://realm.example/ws",
                false,
                Some("offline:offline"),
            ),
            ("", true, None),
        ] {
            let response = build_comms_config(
                CommsProbe {
                    healthy: true,
                    user_count: 0,
                },
                &settings(configured),
                online,
            );
            let selected = response
                .adapter
                .as_deref()
                .and_then(|adapter| adapter.split_once(':').map(|(_, transport)| transport));
            assert_eq!(selected, expected);
            assert_eq!(response.fixed_adapter.as_deref(), expected);
        }
    }

    #[test]
    fn online_and_offline_realms_do_not_advertise_removed_v4_endpoints() {
        let mut s = settings("archipelago:wss://realm.example/ws");
        s.v4 = Some(Discovery {
            control: Some(ControlEndpoint {
                url: "wss://realm.example/ws/v4".into(),
                audience: "realm".into(),
            }),
            pulse: None,
            island_refresh_url: None,
        });
        let probe = CommsProbe {
            healthy: true,
            user_count: 0,
        };
        let online = serde_json::to_value(build_comms_config(probe, &s, true)).unwrap();
        assert_eq!(online["protocol"], "v3");
        assert_eq!(online["fixedAdapter"], "archipelago:wss://realm.example/ws");
        assert!(online.get("v4").is_none());
        let offline = serde_json::to_value(build_comms_config(probe, &s, false)).unwrap();
        assert!(offline.get("v4").is_none());
        assert_eq!(offline["fixedAdapter"], OFFLINE_ADAPTER);
    }

    #[test]
    fn no_fixed_adapter_no_block_and_the_defaults() {
        let raw = |fixed: Option<&'static str>| RawSettings {
            protocol: "",
            fixed_adapter: fixed,
            version: Some(" "),
            commit_hash: None,
            max_users: None,
            offline_when_unreachable: true,
            v4_control_url: Some("wss://realm.example/ws/v4"),
            v4_control_audience: Some("realm"),
            v4_pulse_native_endpoint: Some("pulse.example:7777"),
            v4_pulse_webtransport_url: Some("https://pulse.example:7743"),
            v4_pulse_certificate_hash: Some(
                "abababababababababababababababababababababababababababababababab",
            ),
            v4_pulse_audience: Some("realm-pulse"),
            v4_island_refresh_url: Some("https://realm.example/comms-gatekeeper/island-refresh"),
        };
        assert!(Settings::resolve(raw(None)).is_none());
        assert!(Settings::resolve(raw(Some("  "))).is_none());
        let s = Settings::resolve(raw(Some("archipelago:wss://realm.example/ws"))).unwrap();
        assert_eq!(s.protocol, "v3");
        assert!(s.version.is_none());
        assert!(
            s.v4.is_none(),
            "every obsolete discovery setting is ignored"
        );
        let response = serde_json::to_value(build_comms_config(
            CommsProbe {
                healthy: true,
                user_count: 17,
            },
            &s,
            true,
        ))
        .unwrap();
        assert!(response.get("v4").is_none());
        assert_eq!(response["healthy"], true);
        assert_eq!(response["usersCount"], 17);
        assert_eq!(
            response["adapter"],
            "fixed-adapter:archipelago:wss://realm.example/ws"
        );
        assert_eq!(
            response["fixedAdapter"],
            "archipelago:wss://realm.example/ws"
        );
    }

    struct Core(bool, u64);
    impl CommsCore for Core {
        fn core_status(&self) -> (bool, u64) {
            (self.0, self.1)
        }
    }

    #[test]
    fn the_probe_reads_the_attached_core_and_the_capacity() {
        let mut s = settings("archipelago:wss://realm.example/ws");
        s.max_users = Some(10);
        let state = AboutCommsState::new(s);
        assert_eq!(
            state.probe(),
            CommsProbe {
                healthy: false,
                user_count: 0
            },
            "no core attached: unhealthy"
        );
        assert!(state.attach_core(Arc::new(Core(true, 10))));
        assert!(!state.attach_core(Arc::new(Core(true, 1))));
        let probe = state.probe();
        assert_eq!(
            probe,
            CommsProbe {
                healthy: true,
                user_count: 10
            }
        );
        assert!(!state.under_capacity(probe));
        assert!(state.under_capacity(CommsProbe {
            healthy: true,
            user_count: 9
        }));
        assert!(
            state.sfu.is_alive(),
            "an archipelago adapter has nothing to probe"
        );
    }

    #[test]
    fn three_strikes_flip_the_sfu_down_and_one_answer_restores_it() {
        let health =
            SfuHealth::probing("https://user:pw@sfu.example.invalid/private?token=x".into());
        assert_eq!(health.target(), "https://sfu.example.invalid");
        let strikes = AtomicU64::new(0);
        health.record(false, &strikes);
        health.record(false, &strikes);
        assert!(health.is_alive());
        health.record(false, &strikes);
        assert!(!health.is_alive());
        assert!(health.down_for_secs().is_some());
        health.record(true, &strikes);
        assert!(health.is_alive());
    }

    #[test]
    fn maps_signalling_schemes_onto_http() {
        assert_eq!(
            probe_target("wss://livekit.example.org").as_deref(),
            Some("https://livekit.example.org")
        );
        assert_eq!(
            probe_target("ws://127.0.0.1:7880").as_deref(),
            Some("http://127.0.0.1:7880")
        );
        assert_eq!(
            probe_target("livekit.local").as_deref(),
            Some("https://livekit.local")
        );
        assert_eq!(
            probe_target("example.org/livekit/").as_deref(),
            Some("https://example.org/livekit")
        );
        assert_eq!(
            probe_target(
                "fixed-adapter:signed-login:https://realm.example.org/comms/get-scene-adapter"
            )
            .as_deref(),
            Some("https://realm.example.org/comms/get-scene-adapter")
        );
        assert!(probe_target("fixed-adapter:offline:offline").is_none());
        assert!(probe_target("archipelago:wss://realm.example/ws").is_none());
        assert!(probe_target("   ").is_none());
    }

    #[test]
    fn absent_or_incomplete_configuration_is_not_advertised() {
        assert!(read(&[]).is_none());
        assert!(read(&[("COMMS_V4_CONTROL_URL", "wss://realm.example/ws/v4")]).is_none());
        assert!(read(&[("COMMS_V4_PULSE_AUDIENCE", "realm")]).is_none());
        assert!(
            read(&[
                ("COMMS_V4_CONTROL_URL", "wss://realm.example/ws/v4"),
                ("COMMS_V4_CONTROL_AUDIENCE", " ")
            ])
            .is_none()
        );
    }

    #[test]
    fn endpoints_have_independent_audiences_and_camel_case_names() {
        let config = read(&[
            ("COMMS_V4_CONTROL_URL", "wss://realm.example/ws/v4"),
            ("COMMS_V4_CONTROL_AUDIENCE", "realm-control"),
            ("COMMS_V4_PULSE_NATIVE_ENDPOINT", "pulse.example:7777"),
            (
                "COMMS_V4_PULSE_WEBTRANSPORT_URL",
                "https://pulse.example:7743",
            ),
            ("COMMS_V4_PULSE_AUDIENCE", "realm-pulse"),
        ])
        .unwrap();
        assert_eq!(
            serde_json::to_value(config).unwrap(),
            serde_json::json!({
                "control": {"url": "wss://realm.example/ws/v4", "audience": "realm-control"},
                "pulse": {"nativeEndpoint": "pulse.example:7777", "webTransportUrl": "https://pulse.example:7743", "audience": "realm-pulse"}
            })
        );
    }

    #[test]
    fn a_certificate_hash_is_advertised_only_beside_a_webtransport_url() {
        let upper = "AB".repeat(32);
        let bad = "zz".repeat(32);
        let hash = |url: &str, hash: &str| {
            read(&[
                ("COMMS_V4_PULSE_WEBTRANSPORT_URL", url),
                ("COMMS_V4_PULSE_AUDIENCE", "realm"),
                ("COMMS_V4_PULSE_CERTIFICATE_HASH", hash),
            ])
            .and_then(|d| d.pulse)
            .and_then(|p| p.certificate_hash)
        };
        assert_eq!(
            hash("https://127.0.0.1:5240", &upper),
            Some("ab".repeat(32))
        );
        assert_eq!(
            hash("https://pulse.example:7743", &upper),
            Some("ab".repeat(32))
        );
        assert_eq!(hash("http://pulse.example:7743", &upper), None);
        assert_eq!(hash("https://127.0.0.1:5240", "ab"), None);
        assert_eq!(hash("https://127.0.0.1:5240", &bad), None);
        let native_only = read(&[
            ("COMMS_V4_PULSE_NATIVE_ENDPOINT", "pulse.example:7777"),
            ("COMMS_V4_PULSE_AUDIENCE", "realm"),
            ("COMMS_V4_PULSE_CERTIFICATE_HASH", &upper),
        ]);
        assert_eq!(
            native_only
                .and_then(|d| d.pulse)
                .and_then(|p| p.certificate_hash),
            None
        );
    }

    #[test]
    fn public_endpoints_exclude_credentials_and_insecure_remote_urls() {
        for endpoint in [
            "wss://user:secret@realm.example/ws/v4",
            "wss://realm.example/ws/v4?token=secret",
            "wss://realm.example/ws/v4#secret",
            "ws://realm.example/ws/v4",
            "wss://realm.example/ws/v4\n",
        ] {
            assert!(public_url(endpoint.into(), &["wss"], &["ws"]).is_none());
        }
        assert!(public_url("ws://127.0.0.1:1234/ws/v4".into(), &["wss"], &["ws"]).is_some());
        assert!(public_url("ws://[::1]:1234/ws/v4".into(), &["wss"], &["ws"]).is_some());
    }

    #[test]
    fn native_endpoints_require_a_host_and_nonzero_port_only() {
        for endpoint in ["pulse.example:7777", "[::1]:7777"] {
            assert!(native_endpoint(endpoint.into()).is_some());
        }
        for endpoint in [
            "pulse.example",
            "pulse.example:0",
            "secret@pulse.example:7777",
            "pulse.example:7777/path",
            "pulse.example:7777?token=secret",
        ] {
            assert!(native_endpoint(endpoint.into()).is_none());
        }
    }

    #[test]
    fn island_refresh_has_an_explicit_secure_endpoint_independent_of_control() {
        let config = read(&[
            ("COMMS_V4_CONTROL_URL", "wss://control.example/ws"),
            ("COMMS_V4_CONTROL_AUDIENCE", "realm-control"),
            (
                "COMMS_V4_ISLAND_REFRESH_URL",
                "https://comms.example/comms/island-refresh",
            ),
        ])
        .unwrap();
        assert_eq!(
            serde_json::to_value(config).unwrap()["islandRefreshUrl"],
            "https://comms.example/comms/island-refresh"
        );
        let config = read(&[
            ("COMMS_V4_CONTROL_URL", "wss://control.example/ws"),
            ("COMMS_V4_CONTROL_AUDIENCE", "realm-control"),
            (
                "COMMS_V4_ISLAND_REFRESH_URL",
                "http://comms.example/island-refresh",
            ),
        ])
        .unwrap();
        assert!(config.island_refresh_url.is_none());
        assert!(
            read(&[(
                "COMMS_V4_ISLAND_REFRESH_URL",
                "https://comms.example/island-refresh"
            )])
            .is_none()
        );
    }
}
