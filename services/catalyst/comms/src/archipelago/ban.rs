//! Platform bans for connector admission. External wallet lists live in crate::deny_list.

use std::sync::Arc;

use crate::gatekeeper::moderation::{BanStatus, admission_ban};
use crate::interfaces::AccessPolicy;
use crate::shared::identity::Address;

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
}

impl BanChecker {
    pub fn new(policy: Arc<dyn AccessPolicy>, bans: Arc<dyn BanStatus>) -> Arc<Self> {
        Arc::new(Self { policy, bans })
    }

    /// Reads the store on every call, so a ban issued on any replica refuses the next handshake.
    pub async fn is_banned(&self, address: &str) -> bool {
        Self::lookup(self.policy.as_ref(), self.bans.as_ref(), address)
            .await
            .unwrap_or(false)
    }

    /// A malformed address is never banned (it cannot hold a ban row); a store fault fails open
    /// exactly as the old HTTP hop did, logged.
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
    }

    #[tokio::test]
    async fn required_policy_ban_is_enforced() {
        let checker = BanChecker::new(
            Arc::new(StubPolicy::default()),
            Arc::new(StubBanStatus(Ok(Some(crate::access::Ban::Platform)))),
        );
        let address = "0x0000000000000000000000000000000000000abc";
        assert!(checker.is_banned(address).await);
    }

    /// Answers "not banned" until `issued` is set, as the store does around a new ban.
    struct IssuedLater {
        issued: std::sync::atomic::AtomicBool,
    }
    #[async_trait::async_trait]
    impl BanStatus for IssuedLater {
        async fn ban(
            &self,
            _: &Address,
            _: Option<&crate::access::SceneBanKey>,
            _: Option<&str>,
            _: chrono::NaiveDateTime,
        ) -> Result<Option<crate::access::Ban>, AuthorityNotEstablished> {
            Ok(self
                .issued
                .load(std::sync::atomic::Ordering::SeqCst)
                .then_some(crate::access::Ban::Platform))
        }
    }
    #[tokio::test]
    async fn a_ban_issued_after_a_clean_check_refuses_the_next_check() {
        let bans = Arc::new(IssuedLater {
            issued: false.into(),
        });
        let checker = BanChecker::new(Arc::new(StubPolicy::default()), bans.clone());
        let address = "0x0000000000000000000000000000000000000abc";
        assert!(!checker.is_banned(address).await);
        bans.issued.store(true, std::sync::atomic::Ordering::SeqCst);
        assert!(checker.is_banned(address).await);
    }

    struct FailingBans(std::sync::atomic::AtomicUsize);
    #[async_trait::async_trait]
    impl BanStatus for FailingBans {
        async fn ban(
            &self,
            _: &Address,
            _: Option<&crate::access::SceneBanKey>,
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
        assert_eq!(raw.0.load(std::sync::atomic::Ordering::SeqCst), 2);
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
