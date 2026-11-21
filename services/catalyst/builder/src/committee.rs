//! The curation committee's current members, as builder-server reads them for
//! `isCommitteeMember` and `GET /v1/committee`, read by shared SQL from the
//! Polygon committee index (`shop.committee_event`) up to the block its
//! cursor (`shop.chain_cursor`, indexer `committee`) has published: a
//! wallet's latest MemberSet across both Committee contracts decides, and
//! rows the walk added past the published block wait for its next publish.
//!
//! Every question reads the index afresh, so a wallet removed from the
//! committee loses the role once the index publishes the block that removed
//! it. An unpublished index, one published more than 30 minutes ago, or a
//! failed read can't tell who is on the committee: [`members`] and
//! [`is_member`] answer 503 then, never an empty committee, and
//! [`known_member`] answers false.

use serde::Serialize;
use sqlx::PgPool;

use crate::http::ApiError;

/// One statement, so the published block and the members are one snapshot.
/// Unlike `chain_walk::is_ready` it does not require `folded_block =
/// last_block`: a pass advances `last_block` before it republishes, and the
/// rows are cut at `folded_block`, so the published membership stays
/// readable during a pass. The window is `chain_walk::READY_WINDOW`.
const MEMBERS_SQL: &str = "\
WITH ready AS (
  SELECT folded_block FROM shop.chain_cursor
  WHERE indexer = 'committee' AND network = 'matic' AND full_history
    AND folded_block IS NOT NULL
    AND folded_at <= timezone('UTC', statement_timestamp())
    AND folded_at >= timezone('UTC', statement_timestamp()) - interval '30 minutes'
)
SELECT (SELECT folded_block FROM ready) IS NOT NULL AS ready,
  ARRAY(
    SELECT member FROM (
      SELECT DISTINCT ON (e.member) e.member, e.is_member
      FROM shop.committee_event e, ready
      WHERE e.network = 'matic' AND e.block_number <= ready.folded_block
      ORDER BY e.member, e.block_number DESC, e.log_index DESC
    ) latest
    WHERE latest.is_member
    ORDER BY member
  ) AS members";

/// One member as `GET /v1/committee` answers it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Member {
    pub id: String,
    pub address: String,
}

/// The current members, lowercase and sorted; `None` while the index is
/// unpublished or stale.
pub async fn current_members<'e>(
    pool: impl sqlx::PgExecutor<'e>,
) -> Result<Option<Vec<String>>, sqlx::Error> {
    let (ready, members): (bool, Vec<String>) = sqlx::query_as(MEMBERS_SQL).fetch_one(pool).await?;
    Ok(ready.then_some(members))
}

/// The current members; 503 when the index can't tell.
pub async fn members(pool: &PgPool) -> Result<Vec<Member>, ApiError> {
    let members = read(pool).await?;
    Ok(members
        .into_iter()
        .map(|address| Member {
            id: address.clone(),
            address,
        })
        .collect())
}

/// Whether `address` is a current member; 503 when the index can't tell.
pub async fn is_member<'e>(
    pool: impl sqlx::PgExecutor<'e>,
    address: &str,
) -> Result<bool, ApiError> {
    Ok(contains(&read(pool).await?, address))
}

/// Whether the index shows `address` on the committee; false when it can't
/// tell, for a view only the committee widens.
pub async fn known_member(pool: &PgPool, address: &str) -> bool {
    read(pool)
        .await
        .is_ok_and(|members| contains(&members, address))
}

async fn read<'e>(pool: impl sqlx::PgExecutor<'e>) -> Result<Vec<String>, ApiError> {
    match current_members(pool).await {
        Ok(Some(members)) => Ok(members),
        Ok(None) => {
            tracing::warn!("committee index unpublished or stale; committee decisions answer 503");
            Err(unavailable())
        }
        Err(error) => {
            tracing::warn!(error = %error, "committee index read failed; committee decisions answer 503");
            Err(unavailable())
        }
    }
}

fn contains(members: &[String], address: &str) -> bool {
    members.iter().any(|m| m.eq_ignore_ascii_case(address))
}

fn unavailable() -> ApiError {
    ApiError::service_unavailable("The curation committee is not available. Try again later.")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::support::{publish_committee_at, record_committee_event};

    const REMOVED: &str = "0x00000000000000000000000000000000000000c1";
    const MEMBER: &str = "0x00000000000000000000000000000000000000c2";
    const PENDING: &str = "0x00000000000000000000000000000000000000c3";
    const OLD: &str = "0x71d9350ef44e1e451f00e447c0dff2d1fb75510a";
    const CURRENT: &str = "0xaeec95a8aa671a6d3fec56594827d7804964fa70";

    #[tokio::test]
    async fn the_committee_is_the_published_index_membership() {
        let Some(pool) = crate::db::scratch::isolated_pool(2).await else {
            eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
            return;
        };
        for (block, contract, wallet, set) in [
            (10, OLD, REMOVED, true),
            (20, CURRENT, REMOVED, false),
            (30, CURRENT, MEMBER, true),
            (150, CURRENT, PENDING, true),
        ] {
            record_committee_event(&pool, block, contract, wallet, set).await;
        }
        assert_eq!(
            is_member(&pool, MEMBER).await.unwrap_err().status(),
            503,
            "an unpublished index can't tell"
        );
        assert!(!known_member(&pool, MEMBER).await);

        publish_committee_at(&pool, 100, "timezone('UTC', now())").await;
        assert_eq!(
            members(&pool).await.unwrap(),
            vec![Member {
                id: MEMBER.into(),
                address: MEMBER.into()
            }]
        );
        assert!(
            is_member(&pool, &MEMBER.to_uppercase().replace("0X", "0x"))
                .await
                .unwrap()
        );
        assert!(
            !is_member(&pool, REMOVED).await.unwrap(),
            "its latest event removed it"
        );
        assert!(
            !is_member(&pool, PENDING).await.unwrap(),
            "past the published block"
        );
        assert!(known_member(&pool, MEMBER).await);

        sqlx::query("UPDATE shop.chain_cursor SET last_block = 200 WHERE indexer = 'committee'")
            .execute(&pool)
            .await
            .unwrap();
        assert!(
            !is_member(&pool, PENDING).await.unwrap(),
            "a walk ahead of its publish keeps the published membership"
        );
        publish_committee_at(&pool, 200, "timezone('UTC', now())").await;
        assert!(is_member(&pool, PENDING).await.unwrap(), "once published");

        record_committee_event(&pool, 210, CURRENT, MEMBER, false).await;
        publish_committee_at(&pool, 210, "timezone('UTC', now())").await;
        assert!(
            !is_member(&pool, MEMBER).await.unwrap(),
            "a removal counts from the publish that holds it"
        );

        publish_committee_at(&pool, 210, "timezone('UTC', now()) - interval '31 minutes'").await;
        assert_eq!(
            is_member(&pool, PENDING).await.unwrap_err().status(),
            503,
            "a stale index can't tell"
        );
        assert_eq!(members(&pool).await.unwrap_err().status(), 503);
        assert!(!known_member(&pool, PENDING).await);

        publish_committee_at(&pool, 210, "timezone('UTC', now())").await;
        sqlx::query("ALTER TABLE shop.committee_event RENAME TO committee_event_gone")
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            is_member(&pool, PENDING).await.unwrap_err().status(),
            503,
            "a failed read can't tell"
        );
        assert!(!known_member(&pool, PENDING).await);
        pool.close().await;
    }
}
