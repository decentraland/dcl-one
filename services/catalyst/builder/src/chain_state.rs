//! Chain facts of a standard collection, read by shared SQL from the shop's
//! copy of the Polygon collections subgraph (`shop.collections`, kept by the
//! shop's `collections-mirror` job; never written here), and the published
//! and approved state of a builder collection derived from that copy and the
//! node's own publication records.
//!
//! Every read starts from a view that only this module builds. [`for_read`]
//! answers 503 while the copy has no complete pass; its [`ReadView`] hands out
//! the SQL that fills listings and response bodies. [`for_decision`] also
//! answers 503 once that pass is older than [`STALE_AFTER`]; only its
//! [`DecisionView`] reaches the facts ([`collection`]) and the write guard
//! ([`published`]), which also refuse a row, or an absence, older than the
//! bound.
//!
//! A view can't be built outside this module:
//!
//! ```compile_fail
//! let forged = catalyst_builder::chain_state::ReadView { pass: todo!() };
//! ```
//!
//! The facts and the guard take a decision view, never a read view:
//!
//! ```no_run
//! async fn grant(conn: &mut sqlx::PgConnection) {
//!     let view = catalyst_builder::chain_state::for_decision(conn).await.unwrap();
//!     let _ = catalyst_builder::chain_state::collection(conn, &view, "0x").await;
//!     let _ = catalyst_builder::chain_state::published(conn, &view, uuid::Uuid::nil()).await;
//! }
//! ```
//!
//! ```compile_fail
//! async fn grant(conn: &mut sqlx::PgConnection) {
//!     let view = catalyst_builder::chain_state::for_read(conn).await.unwrap();
//!     let _ = catalyst_builder::chain_state::collection(conn, &view, "0x").await;
//! }
//! ```
//!
//! ```compile_fail
//! async fn guard(conn: &mut sqlx::PgConnection) {
//!     let view = catalyst_builder::chain_state::for_read(conn).await.unwrap();
//!     let _ = catalyst_builder::chain_state::published(conn, &view, uuid::Uuid::nil()).await;
//! }
//! ```

use std::ops::Deref;
use std::sync::atomic::{AtomicU8, Ordering};

use chrono::{DateTime, NaiveDateTime, Utc};
use sqlx::PgConnection;

use crate::http::ApiError;

/// How old the copy's data may be for a decision that grants access or
/// allows a write: the last complete pass, a row's own `mirrored_at`, and the
/// start of the pass an absence comes from (twice the collections mirror's
/// default period).
pub const STALE_AFTER: chrono::TimeDelta = chrono::TimeDelta::minutes(30);

#[derive(Debug, Clone)]
struct Pass {
    completed_at: DateTime<Utc>,
    started_at: Option<DateTime<Utc>>,
    cursor: Option<String>,
}

impl Pass {
    /// When the copy last saw `contract` absent: the start of the pass in
    /// progress once its walk is past `contract`, else the start of the last
    /// complete pass, unknown while another pass has overwritten it.
    fn absent_since(&self, contract: &str) -> Option<DateTime<Utc>> {
        let started = self.started_at?;
        if started <= self.completed_at {
            return Some(started);
        }
        let walked = self
            .cursor
            .as_deref()
            .is_some_and(|cursor| contract.to_ascii_lowercase().as_str() <= cursor);
        walked.then_some(started)
    }
}

/// The copy had a complete pass: enough to read derived state into a
/// listing or a response body.
#[derive(Debug, Clone)]
pub struct ReadView {
    pass: Pass,
}

/// A [`ReadView`] whose pass is at most [`STALE_AFTER`] old: the only view
/// the facts and the write guard take.
#[derive(Debug, Clone)]
pub struct DecisionView {
    read: ReadView,
}

impl Deref for DecisionView {
    type Target = ReadView;
    fn deref(&self) -> &ReadView {
        &self.read
    }
}

/// One collection as the copy holds it. Addresses are lowercase;
/// `reviewed_at` is the subgraph's unix seconds; `mirrored_at` is when the
/// copy last wrote the row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectionFacts {
    pub is_approved: bool,
    pub creator: String,
    pub owner: String,
    pub managers: Vec<String>,
    pub minters: Vec<String>,
    pub reviewed_at: i64,
    pub mirrored_at: DateTime<Utc>,
}

macro_rules! copy_row_of_c {
    () => {
        "shop.collections chain WHERE chain.network = 'matic' \
         AND chain.contract_address = lower(c.contract_address)"
    };
}

macro_rules! own_records {
    () => {
        "EXISTS (SELECT 1 FROM builder.collection_publications receipt \
         WHERE receipt.collection_id = c.id AND receipt.status = 'published') \
         OR EXISTS (SELECT 1 FROM builder.items minted \
         WHERE minted.collection_id = c.id AND minted.blockchain_item_id IS NOT NULL) \
         OR EXISTS (SELECT 1 FROM builder.items curated_item \
         JOIN builder.item_curations curated ON curated.item_id = curated_item.id \
         WHERE curated_item.collection_id = c.id)"
    };
}

macro_rules! chain_join {
    () => {
        "LEFT JOIN shop.collections chain \
         ON chain.network = 'matic' AND chain.contract_address = lower(c.contract_address)"
    };
}

const CHAIN_JOIN: &str = chain_join!();
const FROM_COLLECTIONS: &str = concat!("builder.collections c ", chain_join!());
const PUBLISHED: &str = concat!(
    "(chain.contract_address IS NOT NULL OR ",
    own_records!(),
    ")"
);
const APPROVED: &str = "COALESCE(chain.is_approved, false)";
const PUBLISHED_SUBQUERY: &str = concat!(
    "(EXISTS (SELECT 1 FROM ",
    copy_row_of_c!(),
    ") OR ",
    own_records!(),
    ")"
);

impl ReadView {
    /// When the copy's last pass completed.
    pub fn as_of(&self) -> DateTime<Utc> {
        self.pass.completed_at
    }

    /// The `FROM` items the next two expressions need: `builder.collections
    /// c` with the copy joined once as `chain`.
    pub fn collections(&self) -> &'static str {
        FROM_COLLECTIONS
    }

    /// The copy joined once as `chain` to a `FROM` item aliased `c` that has
    /// `builder.collections`' columns.
    pub fn chain_join(&self) -> &'static str {
        CHAIN_JOIN
    }

    /// A boolean over [`Self::collections`]: the collection is published (the
    /// copy holds its contract, or the node holds its verified receipt, an
    /// item's on-chain id or an item curation,
    /// [`crate::curation::COLLECTION_ITEMS_CURATED`]).
    pub fn published(&self) -> &'static str {
        PUBLISHED
    }

    /// A boolean over [`Self::collections`]: the copy holds the contract as
    /// approved. A linked collection, which has no contract, is never
    /// approved here.
    pub fn approved(&self) -> &'static str {
        APPROVED
    }

    /// [`Self::published`] over `builder.collections c` alone, for a
    /// statement that can't join the copy.
    pub fn published_subquery(&self) -> &'static str {
        PUBLISHED_SUBQUERY
    }
}

/// [`ReadView::chain_join`] and [`ReadView::published`] without a view, for
/// a grant that only "published" opens: the copy never deletes a collection,
/// so a row of any age shows it on chain, and a stale copy, or one without a
/// complete pass, can only leave a collection the node holds no record of
/// unpublished a while longer.
pub(crate) fn published_by_any_pass() -> (&'static str, &'static str) {
    (CHAIN_JOIN, PUBLISHED)
}

/// What the copy can tell, logged when it changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum Condition {
    Fresh = 0,
    NoPass = 1,
    Stale = 2,
}

/// The last [`Condition`] logged, so a log line marks each change and no
/// caller can repeat it.
#[derive(Default)]
pub(crate) struct ConditionLog(AtomicU8);

impl ConditionLog {
    pub(crate) fn changed(&self, condition: Condition) -> bool {
        self.0.swap(condition as u8, Ordering::Relaxed) != condition as u8
    }
}

static CONDITION: ConditionLog = ConditionLog(AtomicU8::new(Condition::Fresh as u8));

fn note(condition: Condition) {
    if !CONDITION.changed(condition) {
        return;
    }
    match condition {
        Condition::Fresh => {
            tracing::info!("builder chain state: the Polygon collections copy is fresh")
        }
        Condition::NoPass => tracing::warn!(
            "builder chain state: shop.mirror_state has no complete pass of the collections mirror on matic; \
             builder routes that read published or approved state answer 503. Run the collections mirror \
             (MARKETPLACE_MIRRORS_ENABLED=true, MATIC_COLLECTIONS_SUBGRAPH_URL) on this database"
        ),
        Condition::Stale => tracing::warn!(
            "builder chain state: the last complete pass of the collections mirror on matic is older than 30 minutes; \
             builder writes answer 503 until the next pass. Check the collections mirror \
             (MARKETPLACE_MIRRORS_ENABLED, MATIC_COLLECTIONS_SUBGRAPH_URL, MARKETPLACE_MIRROR_INTERVAL_SECS) \
             and shop.mirror_state.last_error"
        ),
    }
}

fn unavailable() -> ApiError {
    ApiError::service_unavailable("Chain state is not available yet. Try again later.")
}

fn too_old(at: DateTime<Utc>) -> bool {
    Utc::now() - at > STALE_AFTER
}

type PassRow = (Option<NaiveDateTime>, Option<NaiveDateTime>, Option<String>);

/// A view for a read; 503 while the copy has no complete pass.
pub async fn for_read(conn: &mut PgConnection) -> Result<ReadView, ApiError> {
    let row: Option<PassRow> = sqlx::query_as(
        "SELECT last_complete_pass_at, pass_started_at, cursor_id FROM shop.mirror_state \
         WHERE mirror = 'collections' AND network = 'matic'",
    )
    .fetch_optional(conn)
    .await?;
    let Some((Some(completed), started, cursor)) = row else {
        note(Condition::NoPass);
        return Err(unavailable());
    };
    let pass = Pass {
        completed_at: completed.and_utc(),
        started_at: started.map(|s| s.and_utc()),
        cursor,
    };
    note(if too_old(pass.completed_at) {
        Condition::Stale
    } else {
        Condition::Fresh
    });
    Ok(ReadView { pass })
}

/// Whether the copy holds `contract` on Polygon, by its last complete pass
/// of any age: for a read whose answer that fact alone decides.
pub async fn holds(
    conn: &mut PgConnection,
    _view: &ReadView,
    contract: &str,
) -> Result<bool, ApiError> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM shop.collections WHERE network = 'matic' AND contract_address = lower($1))",
    )
    .bind(contract)
    .fetch_one(conn)
    .await?)
}

/// A view for a decision; 503 also while the last pass is stale.
pub async fn for_decision(conn: &mut PgConnection) -> Result<DecisionView, ApiError> {
    let read = for_read(conn).await?;
    if too_old(read.pass.completed_at) {
        return Err(unavailable());
    }
    Ok(DecisionView { read })
}

fn absence(view: &DecisionView, contract: &str) -> Result<(), ApiError> {
    match view.pass.absent_since(contract) {
        Some(since) if !too_old(since) => Ok(()),
        _ => Err(unavailable()),
    }
}

#[derive(sqlx::FromRow)]
struct FactsRow {
    is_approved: bool,
    creator: String,
    owner: String,
    managers: Vec<String>,
    minters: Vec<String>,
    reviewed_at: i64,
    mirrored_at: NaiveDateTime,
}

fn lowercase(addresses: Vec<String>) -> Vec<String> {
    addresses
        .into_iter()
        .map(|a| a.to_ascii_lowercase())
        .collect()
}

/// The copy's row for `contract` on Polygon; `None` is "not on chain". 503
/// when the row, or the absence, is older than [`STALE_AFTER`].
pub async fn collection(
    conn: &mut PgConnection,
    view: &DecisionView,
    contract: &str,
) -> Result<Option<CollectionFacts>, ApiError> {
    let row: Option<FactsRow> = sqlx::query_as(
        "SELECT is_approved, creator, owner, managers, minters, reviewed_at, mirrored_at \
         FROM shop.collections WHERE network = 'matic' AND contract_address = lower($1)",
    )
    .bind(contract)
    .fetch_optional(conn)
    .await?;
    let Some(r) = row else {
        absence(view, contract)?;
        return Ok(None);
    };
    let mirrored_at = r.mirrored_at.and_utc();
    if too_old(mirrored_at) {
        return Err(unavailable());
    }
    Ok(Some(CollectionFacts {
        is_approved: r.is_approved,
        creator: r.creator.to_ascii_lowercase(),
        owner: r.owner.to_ascii_lowercase(),
        managers: lowercase(r.managers),
        minters: lowercase(r.minters),
        reviewed_at: r.reviewed_at,
        mirrored_at,
    }))
}

/// Whether the builder collection `id` is published, for a guard; `false`
/// when no such row exists. A collection the copy doesn't hold yet answers
/// 503 when that absence is older than [`STALE_AFTER`].
pub async fn published(
    conn: &mut PgConnection,
    view: &DecisionView,
    id: uuid::Uuid,
) -> Result<bool, ApiError> {
    let row: Option<(Option<String>, bool)> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT c.contract_address, {PUBLISHED} FROM {FROM_COLLECTIONS} WHERE c.id = $1"
    )))
    .bind(id)
    .fetch_optional(conn)
    .await?;
    match row {
        None => Ok(false),
        Some((_, true)) => Ok(true),
        Some((None, false)) => Ok(false),
        Some((Some(contract), false)) => absence(view, &contract).map(|()| false),
    }
}
