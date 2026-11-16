use anyhow::{Context, Result};
use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub http_host: String,
    pub http_port: u16,
    pub public_base_url: Option<String>,
    pub tokens: Vec<String>,
    /// Accept trunks without a token; only for a local or test relay.
    pub allow_anonymous: bool,
    pub allow_ids: Vec<String>,
    /// Tunnel ids held at once, connected or in their reconnect grace.
    pub max_agents: usize,
    /// Open channels per connected agent.
    pub max_channels_per_agent: usize,
    pub grace_secs: u64,
    pub ping_secs: u64,
    pub open_timeout_secs: u64,
    pub body_max_bytes: usize,
    /// Seconds a proxied request body may take to arrive.
    pub body_timeout_secs: u64,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            http_host: "127.0.0.1".into(),
            http_port: 5167,
            public_base_url: None,
            tokens: Vec::new(),
            allow_anonymous: false,
            allow_ids: Vec::new(),
            max_agents: 256,
            max_channels_per_agent: 256,
            grace_secs: 120,
            ping_secs: 20,
            open_timeout_secs: 15,
            body_max_bytes: 64 * 1024 * 1024,
            body_timeout_secs: 60,
        }
    }
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Self::from_lookup(&|key| env::var(key).ok())
    }

    pub fn from_lookup(lookup: &dyn Fn(&str) -> Option<String>) -> Result<Self> {
        let defaults = Config::default();
        let opt = |k: &str| lookup(k).filter(|s| !s.is_empty());
        let http_port = match opt("HTTP_SERVER_PORT") {
            Some(raw) => raw.parse::<u16>().context("invalid HTTP_SERVER_PORT")?,
            None => defaults.http_port,
        };
        let cfg = Config {
            http_host: opt("HTTP_SERVER_HOST").unwrap_or(defaults.http_host),
            http_port,
            public_base_url: opt("PUBLIC_BASE_URL").map(|s| s.trim_end_matches('/').to_string()),
            tokens: parse_list(opt("TUNNEL_TOKENS")),
            allow_anonymous: opt("TUNNEL_ALLOW_ANONYMOUS")
                .is_some_and(|v| matches!(v.trim(), "1" | "true")),
            allow_ids: parse_list(opt("TUNNEL_ALLOW_IDS")),
            max_agents: parse_or(opt("TUNNEL_MAX_AGENTS"), defaults.max_agents).max(1),
            max_channels_per_agent: parse_or(
                opt("TUNNEL_MAX_CHANNELS"),
                defaults.max_channels_per_agent,
            )
            .max(1),
            grace_secs: parse_or(opt("TUNNEL_GRACE_SECS"), defaults.grace_secs),
            ping_secs: parse_or(opt("TUNNEL_PING_SECS"), defaults.ping_secs).max(1),
            open_timeout_secs: parse_or(
                opt("TUNNEL_OPEN_TIMEOUT_SECS"),
                defaults.open_timeout_secs,
            )
            .max(1),
            body_max_bytes: parse_or(opt("TUNNEL_BODY_MAX_BYTES"), defaults.body_max_bytes),
            body_timeout_secs: parse_or(
                opt("TUNNEL_BODY_TIMEOUT_SECS"),
                defaults.body_timeout_secs,
            )
            .max(1),
        };
        cfg.validate()?;
        Ok(cfg)
    }

    /// Refuses an open relay: trunks need a token unless anonymous use is opted into.
    pub fn validate(&self) -> Result<()> {
        if self.tokens.is_empty() && !self.allow_anonymous {
            anyhow::bail!(
                "TUNNEL_TOKENS is empty; set it, or TUNNEL_ALLOW_ANONYMOUS=true for a local relay"
            );
        }
        Ok(())
    }

    pub fn public_base(&self) -> String {
        self.public_base_url
            .clone()
            .unwrap_or_else(|| format!("http://{}:{}", self.http_host, self.http_port))
    }
}

fn parse_list(raw: Option<String>) -> Vec<String> {
    raw.map(|s| {
        s.split(',')
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(str::to_string)
            .collect()
    })
    .unwrap_or_default()
}

fn parse_or<T: std::str::FromStr>(raw: Option<String>, default: T) -> T {
    raw.and_then(|s| s.parse().ok()).unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_public_base_is_the_bind_address() {
        let cfg = Config::default();
        assert_eq!(cfg.public_base(), "http://127.0.0.1:5167");
        let cfg = Config {
            public_base_url: Some("https://tunnel.example".into()),
            ..Config::default()
        };
        assert_eq!(cfg.public_base(), "https://tunnel.example");
    }

    #[test]
    fn an_open_relay_needs_an_explicit_opt_in() {
        assert!(Config::default().validate().is_err());
        let anonymous = Config {
            allow_anonymous: true,
            ..Config::default()
        };
        assert!(anonymous.validate().is_ok());
        let tokened = Config {
            tokens: vec!["t".into()],
            ..Config::default()
        };
        assert!(tokened.validate().is_ok());
    }

    fn lookup(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        move |key| {
            pairs
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| v.to_string())
        }
    }

    #[test]
    fn env_parses_the_caps_and_the_anonymous_opt_in() {
        assert!(Config::from_lookup(&lookup(&[])).is_err());
        let cfg = Config::from_lookup(&lookup(&[
            ("TUNNEL_ALLOW_ANONYMOUS", "true"),
            ("TUNNEL_MAX_AGENTS", "3"),
            ("TUNNEL_MAX_CHANNELS", "0"),
        ]))
        .unwrap();
        assert!(cfg.allow_anonymous);
        assert_eq!((cfg.max_agents, cfg.max_channels_per_agent), (3, 1));
        let cfg = Config::from_lookup(&lookup(&[("TUNNEL_TOKENS", "a, b")])).unwrap();
        assert!(!cfg.allow_anonymous);
        assert_eq!(cfg.tokens, vec!["a".to_string(), "b".to_string()]);
        assert_eq!((cfg.max_agents, cfg.max_channels_per_agent), (256, 256));
        assert!(
            Config::from_lookup(&lookup(&[("TUNNEL_ALLOW_ANONYMOUS", "yes")])).is_err(),
            "only true or 1 opts in"
        );
    }

    #[test]
    fn parse_list_splits_and_trims() {
        assert_eq!(
            parse_list(Some("a, b ,,c".into())),
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
        assert!(parse_list(None).is_empty());
    }
}
