//! The stack under test: the addresses a run dials, the parcel grid and the limits it expects the
//! servers to enforce. Limits and grid default to the server crate's own values, so an expectation
//! and the server it checks cannot drift apart.

use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use catalyst_comms::pulse::hardening;
use catalyst_comms::pulse::interest::{ParcelEncoder, ParcelEncoderOptions};
use serde::Deserialize;
use url::Url;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stack {
    /// The API base: the archipelago socket, the gatekeeper and the statistics routes hang off it.
    /// Absent for a Pulse-only stack.
    #[serde(default)]
    pub api: Option<Url>,
    /// Pulse over ENet.
    pub pulse_enet: SocketAddr,
    #[serde(default)]
    pub pulse_webtransport: Option<WebTransportTarget>,
    #[serde(default)]
    pub pulse_metrics: Option<Url>,
    #[serde(default)]
    pub serve_metrics: Option<Url>,
    #[serde(default)]
    pub livekit: Option<LiveKitTarget>,
    /// The environment variable that holds the moderator token.
    #[serde(default)]
    pub moderator_token_env: Option<String>,
    #[serde(default)]
    pub explorer: Option<ExplorerTarget>,
    /// The environment variable that holds the private key of the wallet Pulse admits as a scene
    /// listener (its authoritative server address). Absent: scene listener scenarios are skipped.
    #[serde(default)]
    pub scene_listener_key_env: Option<String>,
    /// A command (program and arguments) that restarts the process running the gatekeeper and
    /// the archipelago connector and returns once that process has been told to restart. Absent:
    /// the scenarios that restart it are skipped.
    #[serde(default)]
    pub restart_command: Option<Vec<String>>,
    #[serde(default)]
    pub grid: Grid,
    #[serde(default)]
    pub limits: Limits,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WebTransportTarget {
    pub url: Url,
    /// SHA-256 of the server certificate, hex. Absent: the system trust store decides.
    #[serde(default)]
    pub certificate_hash: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiveKitTarget {
    /// The room service base, `http://host:port`.
    pub url: Url,
    /// Environment variables that hold the API key and secret.
    pub key_env: String,
    pub secret_env: String,
    /// The LiveKit client binary (the explorer workspace's `harness-util`), run as a child
    /// process to join rooms.
    #[serde(default)]
    pub client: Option<PathBuf>,
}

/// The headless explorer and the addresses it is told to dial. Those are the fault link's fronts:
/// the stack advertises them, the link relays each to the real address behind it.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExplorerTarget {
    pub binary: PathBuf,
    /// Extra arguments every explorer run gets.
    #[serde(default)]
    pub arguments: Vec<String>,
    /// Front address the explorer uses as its realm and API, relayed to [`Stack::api`].
    pub api_front: SocketAddr,
    /// Front address for Pulse over ENet, relayed to [`Stack::pulse_enet`].
    pub pulse_front: SocketAddr,
    /// Front and real address of LiveKit signalling (TCP).
    #[serde(default)]
    pub livekit_signal: Option<Relay>,
    /// Front and real address of LiveKit media (UDP).
    #[serde(default)]
    pub livekit_media: Option<Relay>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Relay {
    pub front: SocketAddr,
    pub real: SocketAddr,
}

/// The parcel grid Pulse encodes positions against.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct Grid {
    pub min_parcel_x: i32,
    pub min_parcel_z: i32,
    pub max_parcel_x: i32,
    pub max_parcel_z: i32,
    pub padding: i32,
    pub parcel_size: i32,
}

impl Default for Grid {
    fn default() -> Self {
        let options = ParcelEncoderOptions::default();
        Self {
            min_parcel_x: options.min_parcel_x,
            min_parcel_z: options.min_parcel_z,
            max_parcel_x: options.max_parcel_x,
            max_parcel_z: options.max_parcel_z,
            padding: options.padding,
            parcel_size: options.parcel_size,
        }
    }
}

impl Grid {
    pub fn encoder(&self) -> ParcelEncoder {
        ParcelEncoder::new(ParcelEncoderOptions {
            min_parcel_x: self.min_parcel_x,
            min_parcel_z: self.min_parcel_z,
            max_parcel_x: self.max_parcel_x,
            max_parcel_z: self.max_parcel_z,
            padding: self.padding,
            parcel_size: self.parcel_size,
        })
    }

    /// A world position as the parcel index and the offset inside that parcel, or `None` outside
    /// the grid.
    pub fn locate(&self, world: [f32; 3]) -> Option<(i32, [f32; 3])> {
        if world.iter().any(|value| !value.is_finite()) {
            return None;
        }
        let size = self.parcel_size as f32;
        let (x, z) = ((world[0] / size).floor(), (world[2] / size).floor());
        if x.abs() > 1e6 || z.abs() > 1e6 {
            return None;
        }
        let (x, z) = (x as i32, z as i32);
        let encoder = self.encoder();
        if !encoder.is_valid_coordinate(x, z) {
            return None;
        }
        let local = [world[0] - x as f32 * size, world[1], world[2] - z as f32 * size];
        Some((encoder.encode(x, z), local))
    }

    /// The world position of a parcel index and an offset inside it.
    pub fn world(&self, parcel_index: i32, local: [f32; 3]) -> [f32; 3] {
        let (x, z) = self.encoder().decode(parcel_index);
        let size = self.parcel_size as f32;
        [x as f32 * size + local[0], local[1], z as f32 * size + local[2]]
    }

    /// The centre of a parcel, at ground level.
    pub fn parcel_centre(&self, x: i32, z: i32) -> [f32; 3] {
        let size = self.parcel_size as f32;
        [(x as f32 + 0.5) * size, 0.0, (z as f32 + 0.5) * size]
    }
}

/// What the servers are expected to enforce. A stack whose servers run with other settings says so
/// in its stack file.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct Limits {
    pub input_max_hz: u32,
    pub input_burst: u32,
    pub discrete_rate_per_sec: u32,
    pub discrete_burst: u32,
    pub max_emote_duration_ms: u32,
    pub max_realm_length: usize,
    pub max_handshake_attempts: u8,
    pub max_concurrent_pre_auth_per_ip: i64,
    pub replay_entries_per_source: usize,
    pub replay_entries_per_wallet: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            input_max_hz: hardening::DEFAULT_INPUT_MAX_HZ,
            input_burst: hardening::DEFAULT_INPUT_BURST,
            discrete_rate_per_sec: hardening::DEFAULT_DISCRETE_RATE_PER_SEC,
            discrete_burst: hardening::DEFAULT_DISCRETE_BURST,
            max_emote_duration_ms: hardening::DEFAULT_MAX_EMOTE_DURATION_MS,
            max_realm_length: hardening::DEFAULT_MAX_REALM_LENGTH,
            max_handshake_attempts: hardening::DEFAULT_MAX_HANDSHAKE_ATTEMPTS,
            max_concurrent_pre_auth_per_ip: hardening::DEFAULT_MAX_CONCURRENT_PRE_AUTH_PER_IP,
            replay_entries_per_source: hardening::DEFAULT_REPLAY_ENTRIES_PER_SOURCE,
            replay_entries_per_wallet: hardening::DEFAULT_REPLAY_ENTRIES_PER_WALLET,
        }
    }
}

impl Stack {
    pub fn load(path: &Path) -> Result<Stack> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading the stack file {}", path.display()))?;
        Stack::parse(&text).with_context(|| format!("in the stack file {}", path.display()))
    }

    pub fn parse(text: &str) -> Result<Stack> {
        Ok(toml::from_str(text)?)
    }

    /// A stack with only Pulse over ENet, for the crate's own tests and for Pulse-only runs.
    pub fn pulse_only(pulse_enet: SocketAddr) -> Stack {
        Stack {
            api: None,
            pulse_enet,
            pulse_webtransport: None,
            pulse_metrics: None,
            serve_metrics: None,
            livekit: None,
            moderator_token_env: None,
            explorer: None,
            scene_listener_key_env: None,
            restart_command: None,
            grid: Grid::default(),
            limits: Limits::default(),
        }
    }

    /// Every host this stack names.
    pub fn hosts(&self) -> Vec<String> {
        let mut hosts = vec![self.pulse_enet.ip().to_string()];
        if let Some(api) = &self.api {
            hosts.push(host_of(api));
        }
        if let Some(target) = &self.pulse_webtransport {
            hosts.push(host_of(&target.url));
        }
        for url in [&self.pulse_metrics, &self.serve_metrics].into_iter().flatten() {
            hosts.push(host_of(url));
        }
        if let Some(target) = &self.livekit {
            hosts.push(host_of(&target.url));
        }
        hosts.sort();
        hosts.dedup();
        hosts
    }

    /// Refuses a hostile run (floods, malformed frames, bans) against a host that is neither
    /// loopback nor private nor named in `allowed_hosts`.
    pub fn ensure_safe_for(&self, hostile: bool, allowed_hosts: &[String]) -> Result<()> {
        if !hostile {
            return Ok(());
        }
        for host in self.hosts() {
            if is_local(&host) || allowed_hosts.iter().any(|allowed| allowed == &host) {
                continue;
            }
            bail!(
                "a hostile scenario will not run against {host}: it is not a loopback or private \
                 address; name it with --allow-host if that is intended"
            );
        }
        Ok(())
    }

    /// The value of a secret named by an environment variable.
    pub fn secret(variable: &str) -> Result<String> {
        std::env::var(variable)
            .with_context(|| format!("the environment variable {variable} is not set"))
    }
}

fn host_of(url: &Url) -> String {
    url.host_str()
        .unwrap_or_default()
        .trim_matches(['[', ']'])
        .to_owned()
}

fn is_local(host: &str) -> bool {
    if host == "localhost" || host.ends_with(".localhost") {
        return true;
    }
    match host.parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) => ip.is_loopback() || ip.is_private() || ip.is_link_local(),
        Ok(IpAddr::V6(ip)) => {
            ip.is_loopback() || (ip.segments()[0] & 0xfe00) == 0xfc00 || (ip.segments()[0] & 0xffc0) == 0xfe80
        }
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stack_at(api: &str) -> Stack {
        Stack::parse(&format!("api = \"{api}\"\npulse_enet = \"127.0.0.1:9000\"\n")).unwrap()
    }

    #[test]
    fn hostile_run_refuses_a_public_host() {
        let public = stack_at("https://example.com");
        let refusal = public.ensure_safe_for(true, &[]).unwrap_err().to_string();
        assert!(refusal.contains("example.com"), "{refusal}");
        public.ensure_safe_for(true, &["example.com".to_owned()]).unwrap();
        // a run that is not hostile may go anywhere
        public.ensure_safe_for(false, &[]).unwrap();

        for local in ["http://127.0.0.1:5210", "http://localhost:5210", "http://10.0.0.1", "http://[::1]:80"] {
            stack_at(local).ensure_safe_for(true, &[]).unwrap();
        }
    }

    #[test]
    fn a_public_pulse_address_is_refused_too() {
        let stack =
            Stack::parse("api = \"http://127.0.0.1:1\"\npulse_enet = \"8.8.8.8:9000\"\n").unwrap();
        assert!(stack.ensure_safe_for(true, &[]).is_err());
    }

    #[test]
    fn limits_default_to_the_server_constants() {
        let limits = stack_at("http://127.0.0.1:1").limits;
        assert_eq!(limits.max_emote_duration_ms, hardening::DEFAULT_MAX_EMOTE_DURATION_MS);
        assert_eq!(limits.input_max_hz, hardening::DEFAULT_INPUT_MAX_HZ);
        assert_eq!(limits.input_burst, hardening::DEFAULT_INPUT_BURST);
        assert_eq!(limits.discrete_rate_per_sec, hardening::DEFAULT_DISCRETE_RATE_PER_SEC);
        assert_eq!(limits.discrete_burst, hardening::DEFAULT_DISCRETE_BURST);
        assert_eq!(limits.max_handshake_attempts, hardening::DEFAULT_MAX_HANDSHAKE_ATTEMPTS);
    }

    #[test]
    fn a_stack_file_overrides_one_limit_and_keeps_the_rest() {
        let stack = Stack::parse(
            "api = \"http://127.0.0.1:1\"\npulse_enet = \"127.0.0.1:9000\"\n[limits]\ndiscrete_burst = 4\n",
        )
        .unwrap();
        assert_eq!(stack.limits.discrete_burst, 4);
        assert_eq!(stack.limits.input_burst, hardening::DEFAULT_INPUT_BURST);
    }

    #[test]
    fn an_unknown_key_is_an_error() {
        assert!(Stack::parse("api = \"http://127.0.0.1:1\"\npulse_enet = \"127.0.0.1:9000\"\npulse = 1\n").is_err());
    }

    #[test]
    fn the_grid_locates_positions_as_the_server_encodes_them() {
        let grid = Grid::default();
        let encoder = grid.encoder();
        let (index, local) = grid.locate([24.0, 1.5, -8.0]).unwrap();
        assert_eq!(index, encoder.encode(1, -1));
        assert_eq!(local, [8.0, 1.5, 8.0]);
        assert_eq!(grid.world(index, local), [24.0, 1.5, -8.0]);
        // one parcel past the padded edge, on each axis, and anything not finite
        let far_x = (grid.max_parcel_x + grid.padding + 1) as f32 * 16.0 + 1.0;
        assert!(grid.locate([far_x, 0.0, 0.0]).is_none());
        let far_z = (grid.min_parcel_z - grid.padding - 1) as f32 * 16.0 + 1.0;
        assert!(grid.locate([0.0, 0.0, far_z]).is_none());
        assert!(grid.locate([f32::NAN, 0.0, 0.0]).is_none());
        assert!(grid.locate([1e30, 0.0, 0.0]).is_none());
        // the last padded parcel is still inside
        let edge_x = (grid.max_parcel_x + grid.padding) as f32 * 16.0 + 1.0;
        assert!(grid.locate([edge_x, 0.0, 0.0]).is_some());
    }
}
