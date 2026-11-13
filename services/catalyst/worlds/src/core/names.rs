//! The DAO name deny list: the `banned-names` list the explorer-api mirror
//! keeps in `explore_api.lists` (plus its local overlay), read through
//! `explore::api::lists::values` and memoized for the old checker's hour,
//! plus the operator's `BLOCKED_WORLD_NAMES` (upstream
//! `name-deny-list-checker.ts`). List entries are lowercased and ONLY
//! lowercased, not suffix-stripped: a name is compared as
//! `normalize_world_name(world_name)`. A configured block is a full world
//! name, compared with the lowercased name, and holds while the list cannot
//! be read.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use sqlx::PgPool;

use crate::shared::cache::TtlMap;

const NAME_DENYLIST_TTL: Duration = Duration::from_secs(60 * 60);
/// How long a failed list read answers the configured blocks alone before the list is read
/// again, so an unhealthy database is not asked by every admission and read.
const FAILED_READ_TTL: Duration = Duration::from_secs(5);
pub const BANNED_NAMES_LIST: &str = "banned-names";

/// `BLOCKED_WORLD_NAMES`: comma-separated, each entry trimmed and lowercased,
/// empty entries and repeats dropped, first occurrence order kept.
#[derive(Debug, Default)]
pub struct BlockedNames {
    ordered: Vec<String>,
    set: HashSet<String>,
}

impl BlockedNames {
    pub fn parse(raw: &str) -> Self {
        let mut blocked = Self::default();
        for name in raw.split(',').map(|n| n.trim().to_lowercase()) {
            if !name.is_empty() && blocked.set.insert(name.clone()) {
                blocked.ordered.push(name);
            }
        }
        blocked
    }
}

pub struct BannedNames {
    blocked: Arc<BlockedNames>,
    raw: Vec<String>,
    normalized: HashSet<String>,
}

impl BannedNames {
    /// Whether `world_name` passes the configured blocks and the deny list
    /// this snapshot holds (upstream `checkNameDenyList`).
    pub fn allows(&self, world_name: &str) -> bool {
        !self.blocked.set.contains(&world_name.to_lowercase())
            && !is_name_banned(&self.normalized, world_name)
    }

    fn new(blocked: Arc<BlockedNames>, names: Vec<String>) -> Self {
        let normalized = names.iter().map(|n| n.to_lowercase()).collect();
        Self {
            blocked,
            raw: names,
            normalized,
        }
    }

    /// Upstream `getBannedNames`: the configured blocks as they are, then
    /// each list entry without its first `.eth` and `.dcl`, repeats dropped.
    pub fn banned_names(&self) -> Vec<String> {
        let mut seen = HashSet::new();
        self.blocked
            .ordered
            .iter()
            .cloned()
            .chain(self.raw.iter().map(|n| strip_suffixes(n)))
            .filter(|n| seen.insert(n.clone()))
            .collect()
    }
}

pub struct NameDenyList {
    blocked: Arc<BlockedNames>,
    cache: TtlMap<(), Arc<BannedNames>>,
    /// One list read at a time: callers that miss the memo wait for it instead of each reading.
    reading: tokio::sync::Mutex<()>,
    /// When the last read failed, kept for [`FAILED_READ_TTL`].
    failed_at: std::sync::Mutex<Option<std::time::Instant>>,
}

impl Default for NameDenyList {
    fn default() -> Self {
        Self::new("")
    }
}

impl NameDenyList {
    /// A checker with `BLOCKED_WORLD_NAMES` (`raw`, see [`BlockedNames::parse`]).
    pub fn new(blocked_world_names: &str) -> Self {
        Self {
            blocked: Arc::new(BlockedNames::parse(blocked_world_names)),
            cache: TtlMap::bounded(NAME_DENYLIST_TTL, 1),
            reading: tokio::sync::Mutex::new(()),
            failed_at: std::sync::Mutex::new(None),
        }
    }

    /// The current snapshot, read by one caller at a time. A failed list read
    /// answers the configured blocks alone for [`FAILED_READ_TTL`], then the
    /// list is read again (upstream caches `[]` for its hour); an expired
    /// snapshot is never served (WR-35).
    pub async fn snapshot(&self, pool: &PgPool) -> Arc<BannedNames> {
        if let Some(cached) = self.fresh() {
            return cached;
        }
        let _reading = self.reading.lock().await;
        if let Some(cached) = self.fresh() {
            return cached;
        }
        self.read(crate::explore::api::lists::values(pool, BANNED_NAMES_LIST).await)
    }

    /// The memoized list, or the configured blocks alone shortly after a failed read.
    fn fresh(&self) -> Option<Arc<BannedNames>> {
        if let Some(cached) = self.cache.get_fresh(&()) {
            return Some(cached);
        }
        let failed_at = *self.failed_at.lock().unwrap_or_else(|e| e.into_inner());
        failed_at
            .filter(|at| at.elapsed() < FAILED_READ_TTL)
            .map(|_| Arc::new(BannedNames::new(self.blocked.clone(), Vec::new())))
    }

    fn read<E: std::fmt::Display>(&self, list: Result<Vec<String>, E>) -> Arc<BannedNames> {
        match list {
            Ok(names) => {
                let fetched = Arc::new(BannedNames::new(self.blocked.clone(), names));
                self.cache.insert((), fetched.clone());
                *self.failed_at.lock().unwrap_or_else(|e| e.into_inner()) = None;
                fetched
            }
            Err(e) => {
                *self.failed_at.lock().unwrap_or_else(|e| e.into_inner()) =
                    Some(std::time::Instant::now());
                tracing::warn!(error = %e, "failed to read the name denylist; only BLOCKED_WORLD_NAMES applies until it reads");
                Arc::new(BannedNames::new(self.blocked.clone(), Vec::new()))
            }
        }
    }

    pub async fn check_name_deny_list(&self, pool: &PgPool, world_name: &str) -> bool {
        self.snapshot(pool).await.allows(world_name)
    }

    #[cfg(test)]
    pub fn seed(&self, names: Vec<String>) {
        self.cache
            .insert((), Arc::new(BannedNames::new(self.blocked.clone(), names)));
    }
}

fn normalize_world_name(name: &str) -> String {
    name.to_lowercase()
        .replacen(".eth", "", 1)
        .replacen(".dcl", "", 1)
}

fn strip_suffixes(name: &str) -> String {
    name.replacen(".eth", "", 1).replacen(".dcl", "", 1)
}

fn is_name_banned(banned: &HashSet<String>, world_name: &str) -> bool {
    banned.contains(&normalize_world_name(world_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_suffixes_and_lowercases() {
        assert_eq!(normalize_world_name("Foo.dcl.eth"), "foo");
        assert_eq!(normalize_world_name("BAR.eth"), "bar");
        assert_eq!(normalize_world_name("baz.dcl"), "baz");
        assert_eq!(normalize_world_name("Qux"), "qux");
    }

    #[test]
    fn is_name_banned_matches_case_insensitively_after_stripping() {
        let banned = BannedNames::new(Arc::default(), vec!["foo".to_string(), "Evil".to_string()]);
        assert!(is_name_banned(&banned.normalized, "foo.dcl.eth"));
        assert!(is_name_banned(&banned.normalized, "FOO.DCL.ETH"));
        assert!(is_name_banned(&banned.normalized, "evil.dcl.eth"));
        assert!(!is_name_banned(&banned.normalized, "good.dcl.eth"));
    }

    #[test]
    fn empty_banlist_allows_everything() {
        let banned = BannedNames::new(Arc::default(), Vec::new());
        assert!(!is_name_banned(&banned.normalized, "anything.dcl.eth"));
    }

    #[test]
    fn is_name_banned_hashset_matches_linear_scan_reference() {
        let lists = [
            vec![
                "foo".to_string(),
                "Evil".to_string(),
                "Bar.dcl".to_string(),
                "MiXeD.eth".to_string(),
            ],
            Vec::new(),
        ];
        let names = [
            "foo.dcl.eth",
            "FOO.DCL.ETH",
            "evil.dcl.eth",
            "EVIL",
            "bar.dcl",
            "BAR.DCL.ETH",
            "mixed",
            "MiXeD.dcl.eth",
            "good.dcl.eth",
        ];
        for list in &lists {
            let set = BannedNames::new(Arc::default(), list.clone());
            for name in &names {
                let reference = list
                    .iter()
                    .any(|n| n.to_lowercase() == normalize_world_name(name));
                assert_eq!(
                    is_name_banned(&set.normalized, name),
                    reference,
                    "mismatch for list {list:?} name {name:?}"
                );
            }
        }
    }

    /// `name-deny-list-checker.spec.ts`: "blocks exact configured world names
    /// without a remote service, preserving unrelated names".
    #[test]
    fn configured_blocks_refuse_exact_names_and_lead_the_banned_names() {
        let checker = NameDenyList::new(" Blocked.dcl.eth, , Other.eth,blocked.dcl.eth, ");
        let snapshot = checker.read(Ok::<_, String>(Vec::new()));
        assert!(!snapshot.allows("BLOCKED.dcl.eth"));
        assert!(!snapshot.allows("other.eth"));
        assert!(snapshot.allows("blocked.eth"));
        assert!(snapshot.allows("good.dcl.eth"));
        assert_eq!(
            snapshot.banned_names(),
            vec!["blocked.dcl.eth", "other.eth"]
        );
    }

    /// An unset or blank `BLOCKED_WORLD_NAMES` blocks nothing.
    #[test]
    fn blank_configured_blocks_are_empty() {
        for raw in ["", " ", " , ,"] {
            let blocked = BlockedNames::parse(raw);
            assert!(
                blocked.ordered.is_empty() && blocked.set.is_empty(),
                "{raw:?}"
            );
        }
    }

    /// "keeps configured blocks effective when the remote list is unavailable".
    #[test]
    fn configured_blocks_hold_while_the_list_cannot_be_read() {
        let checker = NameDenyList::new("blocked.dcl.eth");
        let snapshot = checker.read(Err::<Vec<String>, _>("offline"));
        assert!(!snapshot.allows("blocked.dcl.eth"));
        assert!(
            snapshot
                .banned_names()
                .contains(&"blocked.dcl.eth".to_string())
        );
        assert!(
            checker.cache.get_fresh(&()).is_none(),
            "a failed read is not memoized for the list's hour"
        );
        let held = checker
            .fresh()
            .expect("the blocks alone, for a few seconds");
        assert!(!held.allows("blocked.dcl.eth") && held.allows("banned.dcl.eth"));
        *checker.failed_at.lock().unwrap() = Some(std::time::Instant::now() - FAILED_READ_TTL);
        assert!(checker.fresh().is_none(), "then the list is read again");
        // The next read that succeeds applies the list at once (upstream would
        // serve its cached `[]` until the hour ends).
        let snapshot = checker.read(Ok::<_, String>(vec!["Banned".to_string()]));
        assert!(!snapshot.allows("banned.dcl.eth"));
        assert!(checker.cache.get_fresh(&()).is_some());
        assert!(checker.failed_at.lock().unwrap().is_none());
    }

    /// "adds configured blocks without replacing the remote denylist".
    #[test]
    fn configured_blocks_add_to_the_list() {
        let checker = NameDenyList::new("blocked.dcl.eth");
        let snapshot = checker.read(Ok::<_, String>(vec!["remote-banned".to_string()]));
        assert!(!snapshot.allows("remote-banned.dcl.eth"));
        assert!(!snapshot.allows("blocked.dcl.eth"));
        assert_eq!(
            snapshot.banned_names(),
            vec!["blocked.dcl.eth", "remote-banned"]
        );
    }

    #[tokio::test]
    async fn seeded_checker_blocks_banned_name() {
        let checker = NameDenyList::default();
        checker.seed(vec!["banned".to_string()]);
        let Some(pool) = crate::db::scratch::pool(1).await else {
            return;
        };
        assert!(!checker.check_name_deny_list(&pool, "Banned.dcl.eth").await);
        assert!(checker.check_name_deny_list(&pool, "fine.dcl.eth").await);
        assert_eq!(
            checker.snapshot(&pool).await.banned_names(),
            vec!["banned".to_string()]
        );
    }

    #[tokio::test]
    async fn an_unseeded_checker_reads_the_mirrored_list() {
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let name = format!("denied{}", uuid::Uuid::new_v4().simple());
        crate::explore::api::lists::set_local(&pool, BANNED_NAMES_LIST, &name, true)
            .await
            .unwrap();
        let checker = NameDenyList::default();
        assert!(
            !checker
                .check_name_deny_list(&pool, &format!("{name}.dcl.eth"))
                .await
        );
        assert!(
            checker
                .check_name_deny_list(&pool, "whatever.dcl.eth")
                .await
        );
        crate::explore::api::lists::clear_local(&pool, BANNED_NAMES_LIST, &name)
            .await
            .unwrap();
    }
}
