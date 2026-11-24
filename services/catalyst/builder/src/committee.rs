//! The curation committee's current members, read by shared SQL from the
//! Polygon committee index (`shop.committee_event`) up to the block its
//! cursor (`shop.chain_cursor`, indexer `committee`) has published: a
//! wallet's latest MemberSet across both Committee contracts decides. Rows
//! the walk has added past the published block wait for its next publish.
//! An unpublished index, or one published more than 30 minutes ago, reads
//! as `None`, and callers admit only the configured admin addresses then.

use sqlx::PgPool;

/// One statement, so the published block and the members are one snapshot.
/// Unlike `chain_walk::is_ready` it does not require `folded_block =
/// last_block`: a pass advances `last_block` before it republishes, and the
/// rows are cut at `folded_block`, so the published membership stays
/// readable during a pass. The window is `chain_walk::READY_WINDOW`.
const MEMBERS_SQL: &str = "\
WITH ready AS (
  SELECT folded_block FROM shop.chain_cursor
  WHERE indexer = 'committee' AND network = 'matic' AND full_history
    AND chain_id = 137 AND quarantined_at IS NULL
    AND folded_block IS NOT NULL
    AND folded_at <= timezone('UTC', now())
    AND folded_at >= timezone('UTC', now()) - interval '30 minutes'
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

/// The current members, lowercase and sorted; `None` while the index is
/// not published.
pub async fn current_members(pool: &PgPool) -> Result<Option<Vec<String>>, sqlx::Error> {
    let (ready, members): (bool, Vec<String>) = sqlx::query_as(MEMBERS_SQL).fetch_one(pool).await?;
    Ok(ready.then_some(members))
}
