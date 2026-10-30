//! Who may talk to the server: the page this run opened, and nobody else. The run has a random
//! token that is part of the `perfStream` path (and of the reset page's address); the `Host`
//! header must be a loopback name with the port the runner bound.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use rand::{rngs::OsRng, RngCore};

use crate::out;

pub struct Access {
    token: String,
    hosts: [String; 3],
    refused: AtomicU32,
    reset_reported: AtomicBool,
}

impl Access {
    pub fn new(port: u16) -> Self {
        let mut bytes = [0u8; 16]; // 128 bits
        OsRng.fill_bytes(&mut bytes);
        Self {
            token: out::hex(&bytes),
            hosts: [
                format!("localhost:{port}"),
                format!("127.0.0.1:{port}"),
                format!("[::1]:{port}"),
            ],
            refused: AtomicU32::new(0),
            reset_reported: AtomicBool::new(false),
        }
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    /// What the page is told as `perfStream`.
    pub fn stream_path(&self) -> String {
        format!("/__perf/{}", self.token)
    }

    /// Only the loopback names and the port the runner bound: a page on another site whose name
    /// was re-pointed at 127.0.0.1 (DNS rebinding) sends its own name as the `Host` and is turned
    /// away.
    pub fn host_allowed(&self, host: Option<&str>) -> bool {
        host.is_some_and(|host| {
            self.hosts
                .iter()
                .any(|allowed| allowed.eq_ignore_ascii_case(host))
        })
    }

    pub fn token_matches(&self, candidate: &str) -> bool {
        let (a, b) = (candidate.as_bytes(), self.token.as_bytes());
        a.len() == b.len() && a.iter().zip(b).fold(0u8, |diff, (x, y)| diff | (x ^ y)) == 0
    }

    /// Counted, not logged one by one: whoever sends them controls how many.
    pub fn refuse(&self) {
        if self.refused.fetch_add(1, Ordering::Relaxed) == 0 {
            out::log(
                "refused a request that was not this run's (Host or token); later ones are counted",
            );
        }
    }

    pub fn refused(&self) -> u32 {
        self.refused.load(Ordering::Relaxed)
    }

    /// The reset page reported that it ran.
    pub fn report_reset(&self) {
        self.reset_reported.store(true, Ordering::Relaxed);
    }

    pub fn reset_reported(&self) -> bool {
        self.reset_reported.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_is_128_bits_of_lowercase_hex_and_differs_per_run() {
        let a = Access::new(1);
        let b = Access::new(1);
        assert_eq!(a.token().len(), 32);
        assert!(a
            .token()
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)));
        assert_ne!(a.token(), b.token());
    }

    #[test]
    fn only_this_runs_token_matches() {
        let access = Access::new(1);
        let token = access.token().to_string();
        assert!(access.token_matches(&token));
        assert!(!access.token_matches(&token[1..]));
        assert!(!access.token_matches(&format!("{token}0")));
        assert!(!access.token_matches(&"0".repeat(32)));
    }

    #[test]
    fn host_must_be_a_loopback_name_with_the_bound_port() {
        let access = Access::new(18742);
        for host in [
            "localhost:18742",
            "LOCALHOST:18742",
            "127.0.0.1:18742",
            "[::1]:18742",
        ] {
            assert!(access.host_allowed(Some(host)), "{host}");
        }
        for host in [
            "localhost",
            "localhost:80",
            "evil.example:18742",
            "192.168.1.5:18742",
            "localhost.evil.example:18742",
            "0.0.0.0:18742",
            "",
        ] {
            assert!(!access.host_allowed(Some(host)), "{host}");
        }
        assert!(!access.host_allowed(None));
    }
}
