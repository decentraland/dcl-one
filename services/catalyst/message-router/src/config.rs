//! What `catalyst-message-router` reads from its environment. The defaults are upstream's
//! (`.env.default`, `adapters/livekit.ts`); the names carry a `MESSAGE_ROUTER_` prefix so they
//! cannot collide with `serve`'s in a shared environment file. `MESSAGE_ROUTER_KEY` (or
//! `MESSAGE_ROUTER_KEY_FILE`) is read apart from the flags, never from the command line.

use std::net::SocketAddr;
use std::time::Duration;

use crate::connection::Retry;
use crate::shared::limits::RateLimiter;

#[derive(clap::Parser, Debug, Clone)]
#[command(
    name = "catalyst-message-router",
    about = "Relays community chat on the private messages room to the community's members"
)]
pub struct Cli {
    /// The node's database; the router reads `social.communities` and
    /// `social.community_members` and writes nothing.
    #[arg(long, env = "DATABASE_URL", hide_env_values = true)]
    pub database_url: String,

    /// The gatekeeper's token route, `<serve>/internal/message-router/token/v1`: https, or http
    /// to a loopback or private address. The gatekeeper holds the same `MESSAGE_ROUTER_KEY`.
    #[arg(long, env = "MESSAGE_ROUTER_TOKEN_URL")]
    pub token_url: String,

    /// The LiveKit signaling URL to dial instead of the one the gatekeeper hands out (its
    /// `LIVEKIT_WS_URL` / `LIVEKIT_HOST`), e.g. a private address of the SFU.
    #[arg(long, env = "MESSAGE_ROUTER_LIVEKIT_URL")]
    pub livekit_url: Option<String>,

    /// This replica's number: the gatekeeper mints the identity
    /// `<MESSAGE_ROUTER_IDENTITY_PREFIX>-<replica>`. The Explorer addresses replica 0.
    #[arg(long, env = "MESSAGE_ROUTER_REPLICA", default_value_t = 0)]
    pub replica: u16,

    /// Larger data packets are dropped before they are decoded.
    #[arg(
        long,
        env = "MESSAGE_ROUTER_MAX_PACKET_SIZE_BYTES",
        default_value_t = 8192
    )]
    pub max_packet_size_bytes: usize,

    /// Each sender's burst of routed packets.
    #[arg(long, env = "MESSAGE_ROUTER_RATE_LIMIT_MAX_TOKENS", default_value_t = 10, value_parser = clap::value_parser!(u32).range(1..))]
    pub rate_limit_max_tokens: u32,

    /// Each sender's sustained routed packets per second.
    #[arg(long, env = "MESSAGE_ROUTER_RATE_LIMIT_REFILL_PER_SECOND", default_value_t = 5.0, value_parser = positive)]
    pub rate_limit_refill_per_second: f64,

    /// Attempts per connection (each with a fresh token).
    #[arg(long, env = "MESSAGE_ROUTER_CONNECT_ATTEMPTS", default_value_t = 5, value_parser = clap::value_parser!(u32).range(1..))]
    pub connect_attempts: u32,

    /// The wait between two connection attempts, in milliseconds.
    #[arg(long, env = "MESSAGE_ROUTER_CONNECT_DELAY_MS", default_value_t = 1000)]
    pub connect_delay_ms: u64,

    /// Where `/metrics`, `/health/live` and `/health/ready` listen.
    #[arg(
        long,
        env = "MESSAGE_ROUTER_METRICS_BIND",
        default_value = "127.0.0.1:5007"
    )]
    pub metrics_bind: SocketAddr,

    /// The router's database connections, and so the packets it routes at once: each routed
    /// packet holds one connection for its one membership read, and a packet that finds every
    /// slot taken is dropped and counted `overloaded`.
    #[arg(long, env = "MESSAGE_ROUTER_DB_MAX_CONNECTIONS", default_value_t = 4, value_parser = clap::value_parser!(u32).range(1..))]
    pub db_max_connections: u32,
}

fn positive(raw: &str) -> Result<f64, String> {
    match raw.parse::<f64>() {
        Ok(value) if value.is_finite() && value > 0.0 => Ok(value),
        _ => Err(format!("`{raw}` is not a positive number")),
    }
}

impl Cli {
    /// The per-sender token bucket (`shared::limits`): `rate_limit_max_tokens` at once, refilled
    /// at `rate_limit_refill_per_second`.
    pub fn rate_limiter(&self) -> RateLimiter {
        RateLimiter::new(self.rate_limit_max_tokens, self.rate_limit_window())
    }

    /// How long an empty bucket takes to fill again.
    pub fn rate_limit_window(&self) -> Duration {
        Duration::from_secs_f64(
            f64::from(self.rate_limit_max_tokens) / self.rate_limit_refill_per_second,
        )
    }

    /// Packets routed at once: one per database connection.
    pub fn max_in_flight(&self) -> usize {
        self.db_max_connections as usize
    }

    pub fn retry(&self) -> Retry {
        Retry {
            attempts: self.connect_attempts,
            delay: Duration::from_millis(self.connect_delay_ms),
        }
    }

    /// Under `NODE_POSTURE` island or journey, the token and LiveKit URLs name loopback or this
    /// node's own `PUBLIC_URL`, as serve's and Pulse's own URLs must.
    pub fn check_posture(&self, env: &catalyst_config::posture::Lookup) -> anyhow::Result<()> {
        let Some(posture) = catalyst_config::posture::from_env(env)? else {
            return Ok(());
        };
        let mut values = vec![("MESSAGE_ROUTER_TOKEN_URL", self.token_url.clone())];
        if let Some(url) = &self.livekit_url {
            values.push(("MESSAGE_ROUTER_LIVEKIT_URL", url.clone()));
        }
        if let Some(public) = env("PUBLIC_URL") {
            values.push(("PUBLIC_URL", public));
        }
        catalyst_config::posture::check_values(posture, values)
    }
}

/// `MESSAGE_ROUTER_KEY`, else `MESSAGE_ROUTER_KEY_FILE`'s trimmed contents: 32 base64url bytes.
pub fn key_from_env() -> anyhow::Result<[u8; 32]> {
    let raw = catalyst_config::config::Config::secret_from_env("MESSAGE_ROUTER_KEY")?.ok_or_else(
        || anyhow::anyhow!("MESSAGE_ROUTER_KEY (or MESSAGE_ROUTER_KEY_FILE) is required"),
    )?;
    Ok(crate::shared::message_router::parse_key(&raw, None)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    const TOKEN_URL: &str = "http://127.0.0.1:5210/internal/message-router/token/v1";

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        let mut argv = vec![
            "catalyst-message-router",
            "--database-url",
            "postgres://x/y",
            "--token-url",
            TOKEN_URL,
        ];
        argv.extend_from_slice(args);
        Cli::try_parse_from(argv)
    }

    #[test]
    fn defaults_are_upstream_s() {
        let cli = parse(&[]).unwrap();
        assert_eq!(cli.replica, 0);
        assert_eq!(cli.max_packet_size_bytes, 8192);
        assert_eq!(
            (cli.rate_limit_max_tokens, cli.rate_limit_refill_per_second),
            (10, 5.0)
        );
        assert_eq!(cli.rate_limit_window(), Duration::from_secs(2));
        assert_eq!(cli.max_in_flight(), 4);
        assert_eq!(cli.retry(), Retry::default());
        assert_eq!(cli.metrics_bind.to_string(), "127.0.0.1:5007");
    }

    #[test]
    fn limits_must_be_positive() {
        for bad in ["0", "-1", "1.5", "x"] {
            assert!(parse(&["--rate-limit-max-tokens", bad]).is_err(), "{bad}");
        }
        for bad in ["0", "-1", "NaN", "inf", "x"] {
            assert!(
                parse(&["--rate-limit-refill-per-second", bad]).is_err(),
                "{bad}"
            );
        }
        assert!(parse(&["--connect-attempts", "0"]).is_err());
    }

    /// Upstream's bucket: a burst of 10, then 5 per second.
    #[test]
    fn the_default_bucket_takes_a_burst_of_ten_per_sender() {
        use crate::shared::limits::RateLimitDecision::{Allow, Deny};
        let limiter = parse(&[]).unwrap().rate_limiter();
        for _ in 0..10 {
            assert_eq!(limiter.check("0xa"), Allow);
        }
        assert_eq!(limiter.check("0xa"), Deny);
        assert_eq!(limiter.check("0xb"), Allow);
    }

    #[test]
    fn island_holds_the_urls_to_this_node() {
        let cli = parse(&["--livekit-url", "wss://sfu.example.org"]).unwrap();
        let island = |key: &str| match key {
            "NODE_POSTURE" => Some("island".to_owned()),
            "PUBLIC_URL" => Some("https://node.example.org".to_owned()),
            _ => None,
        };
        let error = cli.check_posture(&island).unwrap_err().to_string();
        assert!(error.contains("MESSAGE_ROUTER_LIVEKIT_URL"), "{error}");
        let local = parse(&["--livekit-url", "wss://node.example.org/rtc"]).unwrap();
        local.check_posture(&island).unwrap();
        cli.check_posture(&|_| None).unwrap();
    }
}
