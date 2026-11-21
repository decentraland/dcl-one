//! The stack under test: the addresses a run dials, the parcel grid and the limits it expects the
//! servers to enforce. Limits and grid default to the server crate's own values, so an expectation
//! and the server it checks cannot drift apart.

use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use catalyst_comms::archipelago::config::{ClusterConfig, DEFAULT_MAX_SOCKETS_PER_IP};
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
    /// The LiveKit client (`harness-util`) for a stack that gives no room-service keys, so no
    /// `[livekit]` section: clients join the rooms their island assignments name. `[livekit]
    /// client` is used when both are set. Read through [`Stack::livekit_client`].
    #[serde(default)]
    livekit_client: Option<PathBuf>,
    #[serde(default)]
    pub grid: Grid,
    #[serde(default)]
    pub limits: Limits,
    /// Secrets given by name, in place of the environment variables that hold them.
    #[serde(skip)]
    given: GivenSecrets,
}

/// Values for secrets, each under the name of the environment variable that would hold it. It
/// prints as the names alone.
#[derive(Clone, Default)]
struct GivenSecrets(std::collections::BTreeMap<String, String>);

impl std::fmt::Debug for GivenSecrets {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_set().entries(self.0.keys()).finish()
    }
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
    /// process to join rooms. Read through [`Stack::livekit_client`].
    #[serde(default)]
    client: Option<PathBuf>,
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
        let local = [
            world[0] - x as f32 * size,
            world[1],
            world[2] - z as f32 * size,
        ];
        Some((encoder.encode(x, z), local))
    }

    /// The world position of a parcel index and an offset inside it.
    pub fn world(&self, parcel_index: i32, local: [f32; 3]) -> [f32; 3] {
        let (x, z) = self.encoder().decode(parcel_index);
        let size = self.parcel_size as f32;
        [
            x as f32 * size + local[0],
            local[1],
            z as f32 * size + local[2],
        ]
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
    /// Pulse handshakes one address may have in flight at once; 0 or less is unlimited.
    pub max_concurrent_pre_auth_per_ip: i64,
    /// Sockets one address may hold on the archipelago connector (`WS_MAX_SOCKETS_PER_IP`);
    /// 0 is unlimited.
    pub max_sockets_per_ip: u32,
    pub replay_entries_per_source: usize,
    pub replay_entries_per_wallet: usize,
    /// How often the archipelago connector sweeps its connected wallets for bans.
    pub ban_sweep_interval_ms: u64,
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
            max_sockets_per_ip: DEFAULT_MAX_SOCKETS_PER_IP,
            replay_entries_per_source: hardening::DEFAULT_REPLAY_ENTRIES_PER_SOURCE,
            replay_entries_per_wallet: hardening::DEFAULT_REPLAY_ENTRIES_PER_WALLET,
            ban_sweep_interval_ms: ClusterConfig::default().ban_sweep_interval_ms,
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
            livekit_client: None,
            grid: Grid::default(),
            limits: Limits::default(),
            given: GivenSecrets::default(),
        }
    }

    /// The LiveKit client (`harness-util`): `[livekit] client`, else the top-level
    /// `livekit_client`. Every reader of either key reads it here.
    pub fn livekit_client(&self) -> Option<&Path> {
        self.livekit
            .as_ref()
            .and_then(|target| target.client.as_deref())
            .or(self.livekit_client.as_deref())
    }

    /// Every host the stack file names: the servers, and every address of the explorer section
    /// (the fronts and the real addresses behind them).
    ///
    /// This is what [`Self::ensure_safe_for`] checks, and it is only what the file says. It does
    /// not cover an address a server hands back at run time: the LiveKit URL inside a connection
    /// string the gatekeeper issues, a realm's `about` URLs, a redirect. A hostile scenario that
    /// follows such an address goes where the server sent it.
    pub fn hosts(&self) -> Vec<String> {
        let mut hosts = vec![self.pulse_enet.ip().to_string()];
        if let Some(api) = &self.api {
            hosts.push(host_of(api));
        }
        if let Some(target) = &self.pulse_webtransport {
            hosts.push(host_of(&target.url));
        }
        for url in [&self.pulse_metrics, &self.serve_metrics]
            .into_iter()
            .flatten()
        {
            hosts.push(host_of(url));
        }
        if let Some(target) = &self.livekit {
            hosts.push(host_of(&target.url));
        }
        if let Some(explorer) = &self.explorer {
            hosts.push(explorer.api_front.ip().to_string());
            hosts.push(explorer.pulse_front.ip().to_string());
            for relay in [&explorer.livekit_signal, &explorer.livekit_media]
                .into_iter()
                .flatten()
            {
                hosts.push(relay.front.ip().to_string());
                hosts.push(relay.real.ip().to_string());
            }
        }
        hosts.sort();
        hosts.dedup();
        hosts
    }

    /// Whether a scenario may run against every host of the stack file. A loopback host (a
    /// loopback address or the name `localhost`) admits every scenario. Any other host admits a
    /// scenario only if `allowed_hosts` (`--allow-host`) names it, and a hostile one (floods,
    /// malformed frames, bans, restarts) only if `hostile_hosts` (`--allow-hostile-host`) names
    /// it as well. These lists keep a run from reaching a host by accident; they are not access
    /// control.
    pub fn ensure_safe_for(
        &self,
        hostile: bool,
        allowed_hosts: &[String],
        hostile_hosts: &[String],
    ) -> Result<(), Refusal> {
        let names = |list: &[String], host: &str| list.iter().any(|named| named == host);
        for host in self.hosts() {
            if is_loopback_host(&host) {
                continue;
            }
            let allowed = names(allowed_hosts, &host);
            if allowed && (!hostile || names(hostile_hosts, &host)) {
                continue;
            }
            return Err(Refusal {
                host,
                hostile,
                allowed,
            });
        }
        Ok(())
    }

    /// The names of the environment variables the stack file says hold secrets: the moderator
    /// token, the LiveKit key and secret and the scene listener key. A child process that has no
    /// use for them is started without them.
    pub fn secret_variables(&self) -> Vec<&str> {
        [
            self.moderator_token_env.as_deref(),
            self.scene_listener_key_env.as_deref(),
            self.livekit.as_ref().map(|target| target.key_env.as_str()),
            self.livekit
                .as_ref()
                .map(|target| target.secret_env.as_str()),
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    /// The value of a secret the stack file names by its environment variable.
    pub fn secret(&self, variable: &str) -> Result<String> {
        match self.given.0.get(variable) {
            Some(value) => Ok(value.clone()),
            None => secret_from_environment(variable),
        }
    }

    /// Gives the secret the variable `variable` would hold, so that nothing has to write the
    /// environment: a program that embeds the tool, and the crate's own tests, where a write to
    /// the environment races every thread that reads it.
    pub fn give_secret(&mut self, variable: &str, value: &str) {
        self.given.0.insert(variable.to_owned(), value.to_owned());
    }
}

/// Why a scenario may not run against the stack: a host of the stack file that is not loopback,
/// and what the run did not name it with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub host: String,
    /// The scenario is hostile.
    pub hostile: bool,
    /// `--allow-host` names the host.
    pub allowed: bool,
}

impl Refusal {
    /// The refusal as text, naming the host or, when `name_host` is false, leaving it out.
    pub fn describe(&self, name_host: bool) -> String {
        let target = if name_host {
            format!("{}, which is not a loopback address,", self.host)
        } else {
            "a host of the stack file that is not a loopback address (--report-hosts names it)"
                .to_owned()
        };
        match (self.hostile, self.allowed) {
            (false, _) => format!("no scenario runs against {target} unless --allow-host names it"),
            (true, false) => format!(
                "a hostile scenario runs against {target} only if both --allow-host and \
                 --allow-hostile-host name it"
            ),
            (true, true) => format!(
                "a hostile scenario runs against {target} only if --allow-hostile-host names it \
                 as well as --allow-host"
            ),
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.describe(true))
    }
}

impl std::error::Error for Refusal {}

/// The value of the environment variable `variable`, which holds a secret.
pub fn secret_from_environment(variable: &str) -> Result<String> {
    std::env::var(variable)
        .with_context(|| format!("the environment variable {variable} is not set"))
}

/// Whether a token may be sent to `url`, or handed to a client that dials it: over TLS (`https`,
/// `wss`) to any host, over a plain scheme only to loopback. The error is the reason, naming the
/// `token` and the stack file's `key` that gives the address.
pub fn token_may_travel_to(url: &Url, token: &str, key: &str) -> Result<(), String> {
    let host = url.host_str().unwrap_or_default().trim_matches(['[', ']']);
    if matches!(url.scheme(), "https" | "wss") || is_loopback_host(host) {
        Ok(())
    } else {
        Err(format!(
            "the {token} is not sent over plain {} to a host that is not loopback: give the \
             stack file an https `{key}`",
            url.scheme()
        ))
    }
}

fn host_of(url: &Url) -> String {
    url.host_str()
        .unwrap_or_default()
        .trim_matches(['[', ']'])
        .to_owned()
}

/// Whether `host` is this machine: a loopback address or the name `localhost`. Any other name,
/// one under `.localhost` included, is whatever the resolver makes of it.
pub fn is_loopback_host(host: &str) -> bool {
    if host == "localhost" {
        return true;
    }
    // a private or link-local address can be anything on a network this machine reaches
    match host.parse::<IpAddr>() {
        Ok(ip) => ip.is_loopback(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stack_at(api: &str) -> Stack {
        Stack::parse(&format!(
            "api = \"{api}\"\npulse_enet = \"127.0.0.1:9000\"\n"
        ))
        .unwrap()
    }

    fn named(host: &str) -> Vec<String> {
        vec![host.to_owned()]
    }

    #[test]
    fn a_host_that_is_not_loopback_admits_nothing_unnamed() {
        let public = stack_at("https://example.com");
        for hostile in [false, true] {
            let refusal = public.ensure_safe_for(hostile, &[], &[]).unwrap_err();
            assert_eq!(refusal.host, "example.com");
            assert!(refusal.to_string().contains("example.com"), "{refusal}");
            assert!(refusal.to_string().contains("--allow-host"), "{refusal}");
        }
        // named with --allow-host: an ordinary scenario runs, a hostile one does not
        public
            .ensure_safe_for(false, &named("example.com"), &[])
            .unwrap();
        let refusal = public
            .ensure_safe_for(true, &named("example.com"), &[])
            .unwrap_err();
        assert!(
            refusal.to_string().contains("--allow-hostile-host"),
            "{refusal}"
        );
        // a hostile scenario needs both
        public
            .ensure_safe_for(true, &named("example.com"), &named("example.com"))
            .unwrap();
        assert!(
            public
                .ensure_safe_for(true, &[], &named("example.com"))
                .is_err()
        );
        assert!(
            public
                .ensure_safe_for(false, &[], &named("example.com"))
                .is_err()
        );
        // another host named does not admit this one
        assert!(
            public
                .ensure_safe_for(false, &named("example.org"), &[])
                .is_err()
        );

        for local in [
            "http://127.0.0.1:5210",
            "http://127.4.5.6:5210",
            "http://localhost:5210",
            "http://[::1]:80",
        ] {
            stack_at(local).ensure_safe_for(true, &[], &[]).unwrap();
            stack_at(local).ensure_safe_for(false, &[], &[]).unwrap();
        }
    }

    #[test]
    fn a_name_under_localhost_needs_allow_host() {
        for (url, host) in [
            ("http://stack.localhost:5210", "stack.localhost"),
            ("http://a.b.localhost", "a.b.localhost"),
            ("http://localhost.example", "localhost.example"),
        ] {
            let stack = stack_at(url);
            let refusal = stack.ensure_safe_for(false, &[], &[]).unwrap_err();
            assert_eq!(refusal.host, host);
            stack.ensure_safe_for(false, &named(host), &[]).unwrap();
        }
        assert!(is_loopback_host("localhost"));
        assert!(!is_loopback_host("stack.localhost"));
    }

    #[test]
    fn a_refusal_can_leave_the_host_out() {
        let refusal = stack_at("https://example.com")
            .ensure_safe_for(false, &[], &[])
            .unwrap_err();
        let unnamed = refusal.describe(false);
        assert!(!unnamed.contains("example.com"), "{unnamed}");
        assert!(unnamed.contains("--report-hosts"), "{unnamed}");
        assert!(refusal.describe(true).contains("example.com"));
    }

    #[test]
    fn a_private_or_link_local_address_needs_allow_host() {
        for (url, host) in [
            ("http://10.0.0.1", "10.0.0.1"),
            ("http://192.168.1.20:5210", "192.168.1.20"),
            ("http://172.16.0.5", "172.16.0.5"),
            ("http://169.254.10.1", "169.254.10.1"),
            ("http://[fd00::1]:80", "fd00::1"),
            ("http://[fe80::1]:80", "fe80::1"),
        ] {
            let stack = stack_at(url);
            let refusal = stack.ensure_safe_for(false, &[], &[]).unwrap_err();
            assert_eq!(refusal.host, host);
            assert!(refusal.to_string().contains("loopback"), "{refusal}");
            stack.ensure_safe_for(false, &named(host), &[]).unwrap();
            stack
                .ensure_safe_for(true, &named(host), &named(host))
                .unwrap();
        }
    }

    #[test]
    fn a_given_secret_stands_in_for_its_variable_and_is_never_printed() {
        let mut stack = stack_at("http://127.0.0.1:1");
        assert!(stack.secret("CE2E_STACK_TEST_NEVER_SET").is_err());
        stack.give_secret("CE2E_STACK_TEST_NEVER_SET", "a-given-secret-value");
        assert_eq!(
            stack.secret("CE2E_STACK_TEST_NEVER_SET").unwrap(),
            "a-given-secret-value"
        );
        let shown = format!("{stack:?}");
        assert!(shown.contains("CE2E_STACK_TEST_NEVER_SET"), "{shown}");
        assert!(!shown.contains("a-given-secret-value"), "{shown}");
    }

    #[test]
    fn the_livekit_client_is_read_from_either_key_in_one_place() {
        let top = Stack::parse(
            "pulse_enet = \"127.0.0.1:9000\"\nlivekit_client = \"top/harness-util\"\n",
        )
        .unwrap();
        assert_eq!(top.livekit_client(), Some(Path::new("top/harness-util")));
        let both = Stack::parse(
            "pulse_enet = \"127.0.0.1:9000\"\nlivekit_client = \"top\"\n[livekit]\n\
             url = \"http://127.0.0.1:5214\"\nkey_env = \"K\"\nsecret_env = \"S\"\n\
             client = \"section\"\n",
        )
        .unwrap();
        assert_eq!(both.livekit_client(), Some(Path::new("section")));
        let neither = Stack::pulse_only("127.0.0.1:9000".parse().unwrap());
        assert_eq!(neither.livekit_client(), None);
    }

    #[test]
    fn the_ban_sweep_is_the_servers_unless_the_stack_file_says_otherwise() {
        let default = stack_at("http://127.0.0.1:1");
        assert_eq!(
            default.limits.ban_sweep_interval_ms,
            ClusterConfig::default().ban_sweep_interval_ms
        );
        let slower = Stack::parse(
            "pulse_enet = \"127.0.0.1:9000\"\n[limits]\nban_sweep_interval_ms = 90000\n",
        )
        .unwrap();
        assert_eq!(slower.limits.ban_sweep_interval_ms, 90_000);
    }

    #[test]
    fn a_public_pulse_address_is_refused_too() {
        let stack =
            Stack::parse("api = \"http://127.0.0.1:1\"\npulse_enet = \"8.8.8.8:9000\"\n").unwrap();
        assert!(stack.ensure_safe_for(true, &[], &[]).is_err());
        assert!(stack.ensure_safe_for(false, &[], &[]).is_err());
    }

    /// A stack file in which every address is loopback, with `public` replacing one line.
    fn stack_with_one_public(replaced: &str, public: &str) -> Stack {
        let lines = [
            "api = \"http://127.0.0.1:5210\"",
            "pulse_enet = \"127.0.0.1:5217\"",
            "pulse_metrics = \"http://127.0.0.1:5217/metrics\"",
            "serve_metrics = \"http://127.0.0.1:5210/metrics\"",
            "[pulse_webtransport]",
            "url = \"https://127.0.0.1:5212/\"",
            "[livekit]",
            "url = \"http://127.0.0.1:5214\"",
            "key_env = \"K\"",
            "secret_env = \"S\"",
            "[explorer]",
            "binary = \"explorer\"",
            "api_front = \"127.0.0.1:6210\"",
            "pulse_front = \"127.0.0.1:6217\"",
            "livekit_signal = { front = \"127.0.0.1:6214\", real = \"127.0.0.1:5214\" }",
            "livekit_media = { front = \"127.0.0.1:6215\", real = \"127.0.0.1:5215\" }",
        ];
        let text = lines
            .iter()
            .map(|line| {
                if line.starts_with(replaced) {
                    // `replaced` is the start of the line; `public` is the whole new line
                    public.to_owned()
                } else {
                    (*line).to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        Stack::parse(&text).unwrap_or_else(|error| panic!("{error:#}\n{text}"))
    }

    #[test]
    fn a_public_address_in_any_field_refuses_a_hostile_run() {
        let cases = [
            ("api = ", "api = \"http://8.8.8.8:5210\""),
            ("pulse_enet", "pulse_enet = \"8.8.8.8:5217\""),
            ("url = \"https://", "url = \"https://8.8.8.8:5212/\""),
            (
                "pulse_metrics",
                "pulse_metrics = \"http://8.8.8.8:5217/metrics\"",
            ),
            (
                "serve_metrics",
                "serve_metrics = \"http://8.8.8.8:5210/metrics\"",
            ),
            ("url = \"http://", "url = \"http://8.8.8.8:5214\""),
            ("api_front", "api_front = \"8.8.8.8:6210\""),
            ("pulse_front", "pulse_front = \"8.8.8.8:6217\""),
            (
                "livekit_signal",
                "livekit_signal = { front = \"8.8.8.8:6214\", real = \"127.0.0.1:5214\" }",
            ),
            (
                "livekit_signal",
                "livekit_signal = { front = \"127.0.0.1:6214\", real = \"8.8.8.8:5214\" }",
            ),
            (
                "livekit_media",
                "livekit_media = { front = \"8.8.8.8:6215\", real = \"127.0.0.1:5215\" }",
            ),
            (
                "livekit_media",
                "livekit_media = { front = \"127.0.0.1:6215\", real = \"8.8.8.8:5215\" }",
            ),
        ];
        // the all-loopback stack is the control: nothing here refuses it
        stack_with_one_public("never matches", "")
            .ensure_safe_for(true, &[], &[])
            .unwrap();
        for (replaced, public) in cases {
            let stack = stack_with_one_public(replaced, public);
            for hostile in [false, true] {
                let refusal = stack
                    .ensure_safe_for(hostile, &[], &[])
                    .expect_err(public)
                    .to_string();
                assert!(refusal.contains("8.8.8.8"), "{public}: {refusal}");
            }
            let both = named("8.8.8.8");
            stack
                .ensure_safe_for(true, &both, &both)
                .unwrap_or_else(|error| panic!("{public}: {error}"));
            stack.ensure_safe_for(false, &both, &[]).unwrap();
        }
    }

    #[test]
    fn limits_default_to_the_server_constants() {
        let limits = stack_at("http://127.0.0.1:1").limits;
        assert_eq!(
            limits.max_emote_duration_ms,
            hardening::DEFAULT_MAX_EMOTE_DURATION_MS
        );
        assert_eq!(limits.input_max_hz, hardening::DEFAULT_INPUT_MAX_HZ);
        assert_eq!(limits.input_burst, hardening::DEFAULT_INPUT_BURST);
        assert_eq!(
            limits.discrete_rate_per_sec,
            hardening::DEFAULT_DISCRETE_RATE_PER_SEC
        );
        assert_eq!(limits.discrete_burst, hardening::DEFAULT_DISCRETE_BURST);
        assert_eq!(
            limits.max_handshake_attempts,
            hardening::DEFAULT_MAX_HANDSHAKE_ATTEMPTS
        );
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
        assert!(
            Stack::parse(
                "api = \"http://127.0.0.1:1\"\npulse_enet = \"127.0.0.1:9000\"\npulse = 1\n"
            )
            .is_err()
        );
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
