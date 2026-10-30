//! The operator's content denylist, made durable: `content.denylist`
//! (`schemas/00_content.sql`).
//!
//! The old live node kept the list in a process `HashSet` that a restart
//! emptied. Here the table is the authority and the in-memory
//! [`super::Denylist`] is its projection: loaded at boot
//! ([`ContentState::load_denylist`]), replaced after every console edit
//! (in the edit's transaction's wake, [`add`]/[`remove`]) and by the
//! `content-denylist-reload` job every [`RELOAD_INTERVAL`] (the freshness
//! budget for an edit made outside this process, `psql` or a second node on
//! the same database). The read routes consult the projection exactly as
//! the old node consulted its set: `/contents/{hash}` answers 404 for a
//! listed hash, the active-entity reads drop a listed entity id,
//! `/pointer-changes` drops its deltas. A local deploy is refused when its
//! entity id, a content hash it declares or one of its pointers is listed
//! ([`refusal`]).
//!
//! An id is any non-blank string of at most 512 bytes without
//! whitespace or control characters (the old console's rule); entity ids
//! and hashes are compared exactly (CIDv0 is case-sensitive), pointers
//! case-insensitively (a pointer is stored lowercased).

use std::sync::Arc;
use std::time::Duration;

use sqlx::PgExecutor;

use super::ContentState;
use super::entity::Entity;
use crate::jobs::{Jobs, PeriodicCfg};

/// How long an edit made outside this process can take to reach the reads.
pub const RELOAD_INTERVAL: Duration = Duration::from_secs(60);

/// The load's bound. The list is curated by hand (expected hundreds of
/// rows); a table past this is a mistake the load reports instead of
/// holding in memory.
pub const MAX_ENTRIES: i64 = 100_000;

pub fn valid_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= 512 && s.chars().all(|c| !c.is_control() && !c.is_whitespace())
}

/// Every listed id, in id order, at most [`MAX_ENTRIES`].
pub async fn load<'e, E: PgExecutor<'e>>(executor: E) -> sqlx::Result<Vec<String>> {
    let ids: Vec<String> =
        sqlx::query_scalar("SELECT id FROM content.denylist ORDER BY id LIMIT $1")
            .bind(MAX_ENTRIES + 1)
            .fetch_all(executor)
            .await?;
    if ids.len() as i64 > MAX_ENTRIES {
        return Err(sqlx::Error::Protocol(format!(
            "content.denylist holds more than {MAX_ENTRIES} rows; refusing to project it"
        )));
    }
    Ok(ids)
}

/// Inserts `id`; `false` when it was already listed. `actor` is the console
/// wallet recorded with the row.
pub async fn add<'e, E: PgExecutor<'e>>(executor: E, id: &str, actor: &str) -> sqlx::Result<bool> {
    let inserted: Option<String> = sqlx::query_scalar(
        "INSERT INTO content.denylist (id, added_by) VALUES ($1, $2) ON CONFLICT (id) DO NOTHING RETURNING id",
    )
    .bind(id)
    .bind(actor)
    .fetch_optional(executor)
    .await?;
    Ok(inserted.is_some())
}

/// Deletes `id`; `false` when it was not listed.
pub async fn remove<'e, E: PgExecutor<'e>>(executor: E, id: &str) -> sqlx::Result<bool> {
    Ok(sqlx::query("DELETE FROM content.denylist WHERE id = $1")
        .bind(id)
        .execute(executor)
        .await?
        .rows_affected()
        == 1)
}

/// Why a local deploy of `entity` is refused, one message per kind of
/// listed id; `None` when nothing it names is listed.
pub fn refusal(entity: &Entity, denylist: &super::Denylist) -> Option<Vec<String>> {
    let mut errors = Vec::new();
    if denylist.contains(&entity.id) {
        errors.push(format!(
            "The entity {} is denylisted on this node.",
            entity.id
        ));
    }
    let mut hashes: Vec<&str> = entity
        .content
        .iter()
        .map(|c| c.hash.as_str())
        .filter(|h| denylist.contains(h))
        .collect();
    hashes.sort_unstable();
    hashes.dedup();
    if !hashes.is_empty() {
        errors.push(format!(
            "These content files are denylisted on this node: {}",
            hashes.join(", ")
        ));
    }
    let mut pointers: Vec<String> = entity
        .pointers
        .iter()
        .map(|p| p.to_lowercase())
        .filter(|p| denylist.denies_pointer(p))
        .collect();
    pointers.sort_unstable();
    pointers.dedup();
    if !pointers.is_empty() {
        errors.push(format!(
            "These pointers are denylisted on this node: {}",
            pointers.join(", ")
        ));
    }
    (!errors.is_empty()).then_some(errors)
}

impl ContentState {
    /// Replaces the operator half of the projection with the table.
    pub async fn load_denylist(&self) -> sqlx::Result<usize> {
        let ids = load(&self.pool).await?;
        let n = ids.len();
        self.denylist.set_operator(ids);
        Ok(n)
    }

    /// The reload job ([`RELOAD_INTERVAL`], the out-of-process edit's
    /// freshness budget). A failed pass keeps the previous projection.
    pub fn spawn_jobs(self: &Arc<Self>, jobs: &Jobs) {
        let state = self.clone();
        jobs.spawn_periodic(
            "content-denylist-reload",
            PeriodicCfg::every(RELOAD_INTERVAL)
                .after_first_interval()
                .with_error_backoff(Duration::from_secs(600)),
            move |ctx| {
                let state = state.clone();
                async move {
                    let n = state.load_denylist().await?;
                    ctx.progress(format!("{n} operator ids"));
                    Ok(())
                }
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Denylist;
    use crate::entity::{ContentMapping, EntityType};
    use crate::tests::support::require_test_db;

    fn entity(id: &str, pointers: &[&str], hashes: &[&str]) -> Entity {
        Entity {
            id: id.to_string(),
            entity_type: EntityType::Scene,
            pointers: pointers.iter().map(|p| p.to_string()).collect(),
            timestamp: 1,
            content: hashes
                .iter()
                .map(|h| ContentMapping {
                    file: format!("{h}.bin"),
                    hash: h.to_string(),
                })
                .collect(),
            version: "v3".into(),
            metadata: None,
        }
    }

    #[test]
    fn ids_follow_the_old_console_rule() {
        assert!(valid_id("bafkreiabc"));
        assert!(valid_id("0,0"));
        assert!(valid_id(&"a".repeat(512)));
        assert!(!valid_id(&"a".repeat(513)));
        assert!(!valid_id(""));
        assert!(!valid_id("a b"));
        assert!(!valid_id("a\u{0}"));
    }

    #[test]
    fn a_deploy_naming_a_listed_id_hash_or_pointer_is_refused() {
        let denylist = Denylist::default();
        let clean = entity("bafyentity", &["0,0"], &["bafyfile"]);
        assert!(refusal(&clean, &denylist).is_none());

        denylist.set_operator([
            "bafyentity".to_string(),
            "bafyfile".to_string(),
            "10,-3".to_string(),
            "0xABC".to_string(),
        ]);
        let named = entity(
            "bafyentity",
            &["0,0", "10,-3", "0xabc"],
            &["bafyfile", "bafyother", "bafyfile"],
        );
        assert_eq!(
            refusal(&named, &denylist).unwrap(),
            vec![
                "The entity bafyentity is denylisted on this node.".to_string(),
                "These content files are denylisted on this node: bafyfile".to_string(),
                "These pointers are denylisted on this node: 0xabc, 10,-3".to_string(),
            ]
        );
        assert!(refusal(&entity("bafyother", &["1,1"], &["bafyother"]), &denylist).is_none());
    }

    #[test]
    fn the_operator_half_never_lifts_a_withheld_blob_and_back() {
        let denylist = Denylist::default();
        denylist.add("bafyphoto");
        denylist.set_operator(["bafyphoto".to_string(), "bafyillegal".to_string()]);
        denylist.set_operator(Vec::<String>::new());
        assert!(
            denylist.contains("bafyphoto"),
            "the camera reel's entry survives an operator removal"
        );
        assert!(!denylist.contains("bafyillegal"));
        denylist.operator_insert("bafyillegal");
        denylist.remove("bafyillegal");
        assert!(
            denylist.contains("bafyillegal"),
            "a camera-reel un-reject never lifts an operator entry"
        );
        assert_eq!(
            denylist.list(),
            vec!["bafyillegal".to_string(), "bafyphoto".to_string()]
        );
        assert_eq!(denylist.operator_ids(), vec!["bafyillegal".to_string()]);
        assert!(denylist.operator_remove("bafyillegal"));
        assert!(!denylist.contains("bafyillegal"));
    }

    /// The table is the authority: an edit lands as a row, a second add is
    /// not an insert, the load projects every row, a remove deletes it.
    #[tokio::test]
    async fn the_table_round_trips_and_projects() {
        require_test_db!(Some(pool) = crate::db::scratch::pool(2).await);
        let tag = uuid::Uuid::new_v4().simple().to_string();
        let (a, b) = (format!("bafy-deny-{tag}-a"), format!("bafy-deny-{tag}-b"));
        assert!(add(&pool, &a, "0xadmin").await.unwrap());
        assert!(!add(&pool, &a, "0xadmin").await.unwrap(), "already listed");
        assert!(add(&pool, &b, "0xadmin").await.unwrap());
        let loaded = load(&pool).await.unwrap();
        assert!(loaded.contains(&a) && loaded.contains(&b));
        let row: (String, chrono::NaiveDateTime) =
            sqlx::query_as("SELECT added_by, added_at FROM content.denylist WHERE id = $1")
                .bind(&a)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(row.0, "0xadmin");
        assert!(
            (chrono::Utc::now().naive_utc() - row.1).num_seconds().abs() < 60,
            "added_at is UTC"
        );
        assert!(remove(&pool, &a).await.unwrap());
        assert!(!remove(&pool, &a).await.unwrap());
        let err = sqlx::query("INSERT INTO content.denylist (id, added_by) VALUES ('', 'x')")
            .execute(&pool)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("denylist_id_check"), "{err}");
        remove(&pool, &b).await.unwrap();
        pool.close().await;
    }
}
