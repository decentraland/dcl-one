//! The DAO name deny list: the `banned-names` list the explorer-api mirror
//! keeps in `explore_api.lists` (plus its local overlay), read through
//! `explore::api::lists::values` and memoized for the old checker's hour.
//! Entries are lowercased and ONLY lowercased, not suffix-stripped: the
//! pre-existing semantics compared `entry.to_lowercase()` against
//! `normalize_world_name(world_name)`.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use sqlx::PgPool;

use crate::shared::cache::TtlMap;

const NAME_DENYLIST_TTL: Duration = Duration::from_secs(60 * 60);
pub const BANNED_NAMES_LIST: &str = "banned-names";

pub struct BannedNames {
    raw: Vec<String>,
    normalized: HashSet<String>,
}

impl BannedNames {
    /// Whether `world_name` passes the deny list this snapshot holds.
    pub fn allows(&self, world_name: &str) -> bool {
        !is_name_banned(&self.normalized, world_name)
    }

    pub fn from_list(names: Vec<String>) -> Self {
        let normalized = names.iter().map(|n| n.to_lowercase()).collect();
        Self {
            raw: names,
            normalized,
        }
    }

    pub fn banned_names(&self) -> Vec<String> {
        self.raw.iter().map(|n| strip_suffixes(n)).collect()
    }
}

pub struct NameDenyList {
    cache: TtlMap<(), Arc<BannedNames>>,
}

impl Default for NameDenyList {
    fn default() -> Self {
        Self {
            cache: TtlMap::bounded(NAME_DENYLIST_TTL, 1),
        }
    }
}

impl NameDenyList {
    /// The current snapshot; a database fault keeps the last snapshot when
    /// there is one and otherwise reads as an empty list (the old checker
    /// kept its last good list across fetch failures).
    pub async fn snapshot(&self, pool: &PgPool) -> Arc<BannedNames> {
        if let Some(cached) = self.cache.get_fresh(&()) {
            return cached;
        }
        match crate::explore::api::lists::values(pool, BANNED_NAMES_LIST).await {
            Ok(names) => {
                let fetched = Arc::new(BannedNames::from_list(names));
                self.cache.insert((), fetched.clone());
                fetched
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to read the name denylist; allowing every name until it reads");
                Arc::new(BannedNames::from_list(Vec::new()))
            }
        }
    }

    pub async fn check_name_deny_list(&self, pool: &PgPool, world_name: &str) -> bool {
        self.snapshot(pool).await.allows(world_name)
    }

    #[cfg(test)]
    pub fn seed(&self, names: Vec<String>) {
        self.cache
            .insert((), Arc::new(BannedNames::from_list(names)));
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
        let banned = BannedNames::from_list(vec!["foo".to_string(), "Evil".to_string()]);
        assert!(is_name_banned(&banned.normalized, "foo.dcl.eth"));
        assert!(is_name_banned(&banned.normalized, "FOO.DCL.ETH"));
        assert!(is_name_banned(&banned.normalized, "evil.dcl.eth"));
        assert!(!is_name_banned(&banned.normalized, "good.dcl.eth"));
    }

    #[test]
    fn empty_banlist_allows_everything() {
        let banned = BannedNames::from_list(Vec::new());
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
            let set = BannedNames::from_list(list.clone());
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
