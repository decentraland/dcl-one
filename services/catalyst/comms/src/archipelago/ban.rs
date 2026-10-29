//! Memoized platform bans for connector admission. External wallet lists live in crate::deny_list.

use std::sync::Arc;
use std::time::Duration;

use crate::gatekeeper::moderation::{BanStatus, admission_ban};
use crate::interfaces::AccessPolicy;
use crate::shared::cache::TtlMap;
use crate::shared::identity::Address;

const BAN_VERDICT_TTL: Duration = Duration::from_secs(10);
const BAN_VERDICT_MAX_ENTRIES: usize = 20_000;

pub fn encode_uri_component(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for &b in input.as_bytes() {
        let keep = b.is_ascii_alphanumeric()
            || matches!(
                b,
                b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')'
            );
        if keep {
            out.push(b as char);
        } else {
            out.push('%');
            out.push(hex_upper(b >> 4));
            out.push(hex_upper(b & 0x0f));
        }
    }
    out
}

fn hex_upper(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        _ => (b'A' + (nibble - 10)) as char,
    }
}

/// The platform ban check, always backed by the access policy.
pub struct BanChecker {
    policy: Arc<dyn AccessPolicy>,
    bans: Arc<dyn BanStatus>,
    /// Verdicts keyed on the address as asked; a lookup that failed open is
    /// never stored, so the next caller asks again.
    verdicts: Arc<TtlMap<String, bool>>,
}

impl BanChecker {
    pub fn new(policy: Arc<dyn AccessPolicy>, bans: Arc<dyn BanStatus>) -> Arc<Self> {
        Arc::new(Self {
            policy,
            bans,
            verdicts: Arc::new(TtlMap::bounded(BAN_VERDICT_TTL, BAN_VERDICT_MAX_ENTRIES)),
        })
    }

    /// Memoized for a few seconds; the sweep uses `is_banned_fresh` instead.
    pub async fn is_banned(&self, address: &str) -> bool {
        let policy = self.policy.as_ref();
        self.verdicts
            .get_or_fetch(address.to_string(), || {
                Self::lookup(policy, self.bans.as_ref(), address)
            })
            .await
            .unwrap_or(false)
    }

    /// Always asks the policy and refreshes the memo with what it said.
    pub async fn is_banned_fresh(&self, address: &str) -> bool {
        let policy = self.policy.as_ref();
        match Self::lookup(policy, self.bans.as_ref(), address).await {
            Ok(banned) => {
                self.verdicts.insert(address.to_string(), banned);
                banned
            }
            Err(()) => false,
        }
    }

    /// A malformed address is never banned (it cannot hold a ban row); a store fault fails open
    /// exactly as the old HTTP hop did, logged, and is not memoized.
    async fn lookup(
        policy: &dyn AccessPolicy,
        bans: &dyn BanStatus,
        address: &str,
    ) -> Result<bool, ()> {
        let Ok(address) = Address::parse(address) else {
            return Ok(false);
        };
        match admission_ban(
            policy,
            bans,
            &address,
            None,
            None,
            chrono::Utc::now().naive_utc(),
        )
        .await
        {
            Ok(ban) => Ok(ban.is_some()),
            Err(error) => {
                tracing::warn!(address = %address.as_str(), %error, "ban check failed, allowing connection");
                Err(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gatekeeper::moderation::test_support::StubBanStatus;
    use crate::interfaces::test_support::StubPolicy;
    use crate::shared::identity::AuthorityNotEstablished;
    #[test]
    fn encode_uri_component_matches_js_for_special_chars() {
        assert_eq!(
            encode_uri_component("user/with?special#chars"),
            "user%2Fwith%3Fspecial%23chars"
        );
    }

    #[test]
    fn encode_uri_component_leaves_hex_addresses_untouched() {
        let addr = "0xAbC0123456789def";
        assert_eq!(encode_uri_component(addr), addr);
    }

    #[test]
    fn encode_uri_component_keeps_unreserved_marks() {
        assert_eq!(encode_uri_component("-_.!~*'()"), "-_.!~*'()");
    }

    #[test]
    fn encode_uri_component_escapes_space_and_utf8_bytes() {
        assert_eq!(encode_uri_component("a b"), "a%20b");
        assert_eq!(encode_uri_component("\u{E9}"), "%C3%A9");
    }

    #[tokio::test]
    async fn permissive_policy_allows() {
        let checker = BanChecker::new(
            Arc::new(StubPolicy::default()),
            Arc::new(StubBanStatus::default()),
        );
        assert!(
            !checker
                .is_banned("0x0000000000000000000000000000000000000abc")
                .await
        );
        assert!(
            !checker
                .is_banned_fresh("0x0000000000000000000000000000000000000abc")
                .await
        );
    }

    #[tokio::test]
    async fn required_policy_ban_is_enforced_by_cached_and_fresh_checks() {
        let checker = BanChecker::new(
            Arc::new(StubPolicy::default()),
            Arc::new(StubBanStatus(Ok(Some(crate::access::Ban::Platform)))),
        );
        let address = "0x0000000000000000000000000000000000000abc";
        assert!(checker.is_banned(address).await);
        assert!(checker.is_banned_fresh(address).await);
    }

    struct FailingBans(std::sync::atomic::AtomicUsize);
    #[async_trait::async_trait]
    impl BanStatus for FailingBans {
        async fn ban(
            &self,
            _: &Address,
            _: Option<&crate::world_identity::SceneId>,
            _: Option<&str>,
            _: chrono::NaiveDateTime,
        ) -> Result<Option<crate::access::Ban>, AuthorityNotEstablished> {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Err(AuthorityNotEstablished::store_unavailable(
                "social.user_bans",
                &sqlx::Error::PoolClosed,
            ))
        }
    }
    #[tokio::test]
    async fn raw_authority_failure_is_not_cached_as_an_allow() {
        let raw = Arc::new(FailingBans(std::sync::atomic::AtomicUsize::new(0)));
        let checker = BanChecker::new(Arc::new(StubPolicy::default()), raw.clone());
        let address = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        assert!(!checker.is_banned(address).await);
        assert!(!checker.is_banned(address).await);
        assert!(!checker.is_banned_fresh(address).await);
        assert_eq!(raw.0.load(std::sync::atomic::Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn ban_lookup_fails_open_on_policy_error() {
        let bans = StubBanStatus(Err(AuthorityNotEstablished::UndeterminedStoreUnavailable {
            store: "social.user_bans",
            reason_for_operators: "down".into(),
        }));
        let bans = BanChecker::new(Arc::new(StubPolicy::default()), Arc::new(bans));
        assert!(
            !bans
                .is_banned("0x0000000000000000000000000000000000000abc")
                .await
        ); // the same answer the HTTP-era check gave on an unreachable gatekeeper
    }
}
