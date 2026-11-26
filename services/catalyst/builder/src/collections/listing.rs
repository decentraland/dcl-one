//! `GET /v1/collections` (the committee's listing and the public search) and
//! `GET /v1/{address}/collections`, with builder-server's filters, sorts and
//! pagination envelope.
//!
//! Every filter and sort becomes SQL by matching its value against a fixed
//! list and pushing a constant fragment; values reach the statement only as
//! bound parameters. A row carries the status of the collection's newest
//! curation, and its last activity counts that curation's last change; the
//! review filters, an address's status filter and counts, and the relevance
//! and request sorts read that curation with the chain copy, as
//! builder-server does.
//!
//! A listing is a value only its constructor builds from a grant or from a
//! request that is a public search, and each store function takes its own
//! kind, so a route can't list another wallet's rows, or drafts as public
//! rows:
//!
//! ```compile_fail,E0451
//! let forged = catalyst_builder::collections::listing::PublicSearch { listing: todo!() };
//! ```
//!
//! ```compile_fail,E0308
//! async fn widen(
//!     state: &catalyst_builder::SharedState,
//!     search: &catalyst_builder::collections::listing::PublicSearch,
//! ) {
//!     let _ = state.items.address_collections(search).await;
//! }
//! ```
//!
//! Bounds: a response holds at most [`COMMITTEE_CAP`] rows (the committee's
//! listing and the public search) or [`ADDRESS_CAP`] rows (an address's own
//! listing), with or without pagination; `q` is at most [`Q_MAX`]
//! characters, matched literally; at most 20 assignees and 5 tags of at most
//! 64 characters.

use std::collections::HashMap;
use std::sync::LazyLock;

use crate::kept::Since;
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::{Postgres, QueryBuilder};
use uuid::Uuid;

use crate::access::{OnCommittee, Wallet};
use crate::chain_state::{self, ReadView};
use crate::http::ApiError;
use crate::items::ItemsComponent;
use crate::third_parties::provider;

use super::{CollectionListingRowOut, CollectionRow, PUBLIC_LISTING, body_columns, project};

/// The most rows the committee's listing or the public search answers.
pub const COMMITTEE_CAP: i64 = 1000;
/// The most rows an address's own listing answers.
pub const ADDRESS_CAP: i64 = 5000;
/// The longest `q`.
pub const Q_MAX: usize = 256;
pub(crate) const MAX_ASSIGNEES: usize = 20;
pub(crate) const MAX_TAGS: usize = 5;
pub(crate) const TAG_MAX: usize = 64;
const UNPAGINATED_LIMIT: i64 = 100_000;
const UNASSIGNED: &str = "unassigned";

const CURATION_STATUSES: [&str; 7] = [
    "pending",
    "approved",
    "rejected",
    "to_review",
    "under_review",
    "disabled",
    "auto_reviewing",
];
const COLLECTION_STATUSES: [&str; 5] =
    ["draft", "under_review", "published", "rejected", "disabled"];
const REVIEW_TYPES: [&str; 2] = ["new", "updates"];

const ON_CHAIN_UNAPPROVED: &str = "(chain.contract_address IS NOT NULL AND NOT chain.is_approved)";
const APPROVED_ON_CHAIN: &str = "COALESCE(chain.is_approved, false)";
const REVIEWED_ON_CHAIN: &str = "(chain.contract_address IS NOT NULL AND NOT chain.is_approved \
    AND chain.reviewed_at <> chain.created_at)";
const ON_CHAIN: &str = "(chain.contract_address IS NOT NULL)";
const STANDARD: &str = "(c.third_party_id IS NULL AND c.urn_suffix IS NULL)";
const THIRD_PARTY: &str = "(c.third_party_id IS NOT NULL AND c.urn_suffix IS NOT NULL)";
macro_rules! newest_curation {
    ($column:literal) => {
        concat!(
            "(SELECT cc.",
            $column,
            " FROM builder.collection_curations cc WHERE cc.collection_id = c.id ORDER BY cc.created_at DESC LIMIT 1)"
        )
    };
}
const NEWEST_STATUS: &str = newest_curation!("status");
const NEWEST_ASSIGNEE: &str = newest_curation!("assignee");
const NO_CURATION: &str =
    "NOT EXISTS (SELECT 1 FROM builder.collection_curations cn WHERE cn.collection_id = c.id)";
const PENDING_ITEM_CURATION: &str = "EXISTS (SELECT 1 FROM builder.item_curations pk \
    JOIN builder.items pi ON pi.id = pk.item_id WHERE pi.collection_id = c.id AND pk.status = 'pending')";
const AUTO_REVIEWING: &str = "(COALESCE((SELECT cc.status FROM builder.collection_curations cc \
    WHERE cc.collection_id = c.id ORDER BY cc.created_at DESC LIMIT 1) = 'pending', false) \
    AND c.third_party_id IS NULL AND EXISTS (SELECT 1 FROM builder.validation_requests r \
    WHERE r.collection_id = c.id AND r.settled_at IS NULL AND NOT EXISTS (SELECT 1 FROM builder.collection_events d \
    WHERE d.collection_id = c.id AND d.type IN ('review.approved', 'review.rejected') AND d.created_at > \
    COALESCE((SELECT max(s.created_at) FROM builder.collection_events s WHERE s.collection_id = c.id \
    AND s.type = 'review.automated_started' AND s.payload->>'validationId' = r.id::text), r.created_at))))";
static STATUS: LazyLock<String> = LazyLock::new(|| {
    format!(
        "(CASE WHEN c.third_party_id IS NOT NULL THEN NULL \
         WHEN chain.contract_address IS NULL THEN 'draft' \
         WHEN {NEWEST_STATUS} = 'rejected' THEN 'rejected' \
         WHEN chain.is_approved THEN CASE WHEN {NEWEST_STATUS} = 'pending' THEN 'under_review' ELSE 'published' END \
         WHEN {NEWEST_STATUS} = 'approved' OR ({NO_CURATION} AND {REVIEWED_ON_CHAIN}) THEN 'disabled' \
         ELSE 'under_review' END)"
    )
});
const REQUESTED_AT: &str = concat!(
    "COALESCE((SELECT max(rk.created_at) FROM builder.items ri JOIN builder.item_curations rk \
     ON rk.item_id = ri.id WHERE ri.collection_id = c.id AND rk.status = 'pending'), \
     (SELECT max(rk.created_at) FROM builder.items ri JOIN builder.item_curations rk \
     ON rk.item_id = ri.id WHERE ri.collection_id = c.id), ",
    newest_curation!("created_at"),
    ", c.created_at)"
);
const PUBLISHED_IDS: &str = "(SELECT m.collection_id AS id FROM builder.items m WHERE m.blockchain_item_id IS NOT NULL \
    UNION SELECT t.id FROM builder.collections t WHERE t.third_party_id IS NOT NULL AND EXISTS (SELECT 1 \
    FROM builder.items ci JOIN builder.item_curations k ON k.item_id = ci.id WHERE ci.collection_id = t.id)) pub \
    CROSS JOIN LATERAL (SELECT * FROM builder.collections c0 WHERE c0.id = pub.id";
const ADDRESS_IDS: &str = "SELECT o.id FROM builder.collections o WHERE o.eth_address = $1 AND o.third_party_id IS NULL \
    UNION SELECT t.id FROM builder.collections t WHERE t.third_party_id = ANY($2) \
    UNION SELECT b.id FROM shop.collections s JOIN builder.collections b ON b.contract_address = s.contract_address \
    WHERE s.network = 'matic' AND (s.creator = $1 OR EXISTS (SELECT 1 FROM unnest(s.managers || s.minters) r \
    WHERE lower(r) = $1))";
const PUBLISHED: &str = "(EXISTS (SELECT 1 FROM builder.items m WHERE m.collection_id = c.id \
    AND m.blockchain_item_id IS NOT NULL) OR EXISTS (SELECT 1 FROM builder.items m \
    JOIN builder.item_curations k ON k.item_id = m.id WHERE m.collection_id = c.id))";
const UNPUBLISHED: &str = "NOT EXISTS (SELECT 1 FROM builder.items m WHERE m.collection_id = c.id \
    AND m.blockchain_item_id IS NOT NULL) AND NOT EXISTS (SELECT 1 FROM builder.items m \
    JOIN builder.item_curations k ON k.item_id = m.id WHERE m.collection_id = c.id)";
macro_rules! last_activity {
    () => {
        "GREATEST(c.updated_at, (SELECT max(i.updated_at) FROM builder.items i WHERE i.collection_id = c.id), \
         (SELECT cc.updated_at FROM builder.collection_curations cc WHERE cc.collection_id = c.id \
           ORDER BY cc.created_at DESC LIMIT 1))"
    };
}
const LAST_ACTIVITY: &str = last_activity!();
const CURATION_STATUS: &str = "(SELECT cc.status FROM builder.collection_curations cc \
    WHERE cc.collection_id = c.id ORDER BY cc.created_at DESC LIMIT 1)";

/// The request's query string as its key and value pairs.
pub struct ListQuery(Vec<(String, String)>);

impl ListQuery {
    pub fn parse(raw: Option<&str>) -> Self {
        Self(
            url::form_urlencoded::parse(raw.unwrap_or_default().as_bytes())
                .into_owned()
                .collect(),
        )
    }

    pub(crate) fn all(&self, key: &str) -> Vec<&str> {
        self.0
            .iter()
            .filter(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
            .collect()
    }

    /// A parameter given once; a repeated one is refused.
    pub(crate) fn one(&self, key: &str) -> Result<Option<&str>, ApiError> {
        match self.all(key).as_slice() {
            [] => Ok(None),
            [value] => Ok(Some(value)),
            values => {
                Err(ApiError::bad_request(format!("Invalid {key}"))
                    .with_data(json!({ key: values })))
            }
        }
    }

    /// The query without any `key` parameter.
    pub(crate) fn without(&self, key: &str) -> Self {
        Self(self.0.iter().filter(|(k, _)| k != key).cloned().collect())
    }

    /// builder-server's public search: a non-empty `q` and `is_published`
    /// exactly `true`.
    pub fn is_public_search(&self) -> bool {
        self.all("q").iter().any(|q| !q.is_empty()) && self.all("is_published") == ["true"]
    }

    pub(crate) fn non_empty(&self, key: &str) -> Result<Option<&str>, ApiError> {
        Ok(self.one(key)?.filter(|v| !v.is_empty()))
    }
}

/// JavaScript's `parseInt(s, 10)`: leading whitespace, a sign, the leading
/// digits; `None` when there are none. Saturates.
pub(crate) fn js_int(s: &str) -> Option<i64> {
    let s = s.trim_start();
    let (negative, rest) = match s.as_bytes().first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return None;
    }
    let value = digits.parse::<i64>().unwrap_or(i64::MAX);
    Some(if negative { -value } else { value })
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Page {
    pub(crate) limit: i64,
    pub(crate) offset: i64,
    pub(crate) page: Option<i64>,
}

/// `getPaginationParams`: paginated when `page` and `limit` are both
/// non-zero; `limit` at or above 100,000 counts as absent. The row count is
/// capped at `cap`.
pub(crate) fn page(q: &ListQuery, cap: i64) -> Result<Page, ApiError> {
    let limit = q
        .non_empty("limit")?
        .and_then(js_int)
        .filter(|l| *l < UNPAGINATED_LIMIT);
    let page = q.non_empty("page")?.and_then(js_int);
    if limit.is_some_and(|l| l < 0) || page.is_some_and(|p| p < 0) {
        return Err(ApiError::bad_request("Invalid pagination")
            .with_data(json!({ "page": q.all("page"), "limit": q.all("limit") })));
    }
    let paged = page.filter(|p| *p > 0 && limit.is_some_and(|l| l > 0));
    let limit = limit.map_or(cap, |l| l.min(cap));
    Ok(match paged {
        Some(page) => Page {
            limit,
            offset: limit.saturating_mul(page - 1),
            page: Some(page),
        },
        None => Page {
            limit,
            offset: 0,
            page: None,
        },
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sort {
    MostRelevant,
    CreatedAtDesc,
    CreatedAtAsc,
    NameDesc,
    NameAsc,
    UpdatedAtDesc,
    UpdatedAtAsc,
    LastActivityDesc,
    LastActivityAsc,
    RequestedAtDesc,
    RequestedAtAsc,
    Unordered,
}

impl Sort {
    fn named(raw: &str) -> Self {
        match raw {
            "MOST_RELEVANT" => Self::MostRelevant,
            "CREATED_AT_DESC" => Self::CreatedAtDesc,
            "CREATED_AT_ASC" => Self::CreatedAtAsc,
            "NAME_DESC" => Self::NameDesc,
            "NAME_ASC" => Self::NameAsc,
            "UPDATED_AT_DESC" => Self::UpdatedAtDesc,
            "UPDATED_AT_ASC" => Self::UpdatedAtAsc,
            "LAST_ACTIVITY_DESC" => Self::LastActivityDesc,
            "LAST_ACTIVITY_ASC" => Self::LastActivityAsc,
            "REQUESTED_AT_DESC" => Self::RequestedAtDesc,
            "REQUESTED_AT_ASC" => Self::RequestedAtAsc,
            _ => Self::Unordered,
        }
    }

    fn order_by(self, review_flow: bool) -> String {
        match self {
            Self::MostRelevant => format!(
                " ORDER BY CASE WHEN {} THEN 0 WHEN {} THEN 1 WHEN {NEWEST_STATUS} = 'approved' THEN 2 \
                 WHEN {NEWEST_STATUS} = 'rejected' THEN 3 ELSE 4 END, c.created_at DESC, c.id",
                curation_condition("to_review", review_flow),
                curation_condition("under_review", review_flow),
            ),
            Self::CreatedAtDesc => " ORDER BY c.created_at DESC, c.id".into(),
            Self::CreatedAtAsc => " ORDER BY c.created_at ASC, c.id".into(),
            Self::NameDesc => " ORDER BY c.name DESC, c.id".into(),
            Self::NameAsc => " ORDER BY c.name ASC, c.id".into(),
            Self::UpdatedAtDesc => " ORDER BY c.updated_at DESC, c.id".into(),
            Self::UpdatedAtAsc => " ORDER BY c.updated_at ASC, c.id".into(),
            Self::LastActivityDesc => concat!(" ORDER BY ", last_activity!(), " DESC, c.id").into(),
            Self::LastActivityAsc => concat!(" ORDER BY ", last_activity!(), " ASC, c.id").into(),
            Self::RequestedAtDesc => format!(" ORDER BY {REQUESTED_AT} DESC, c.id"),
            Self::RequestedAtAsc => format!(" ORDER BY {REQUESTED_AT} ASC, c.id"),
            Self::Unordered => " ORDER BY c.id".into(),
        }
    }
}

/// One filter, as a constant fragment and the values it binds.
enum Filter {
    Fragment(&'static str),
    Sql(String),
    Assignees(Vec<String>, bool),
    Name(String),
    Tags(Vec<String>),
    Contract(String),
}

fn curation_condition(status: &str, review_flow: bool) -> String {
    let auto = if review_flow { AUTO_REVIEWING } else { "false" };
    let st = NEWEST_STATUS;
    match status {
        "pending" => format!(
            "((({st} = 'pending' AND ({ON_CHAIN_UNAPPROVED} OR {THIRD_PARTY})) OR ({NO_CURATION} \
             AND {ON_CHAIN_UNAPPROVED})) AND NOT {auto})"
        ),
        "approved" => format!(
            "(({st} = 'approved' AND ({APPROVED_ON_CHAIN} OR {THIRD_PARTY})) OR ({NO_CURATION} \
             AND {APPROVED_ON_CHAIN}))"
        ),
        "rejected" => format!("({st} = 'rejected' AND ({ON_CHAIN} OR {THIRD_PARTY}))"),
        "to_review" => format!(
            "({NEWEST_ASSIGNEE} IS NULL AND (({THIRD_PARTY} AND ({st} = 'pending' OR {NO_CURATION} \
             OR {PENDING_ITEM_CURATION})) OR ({STANDARD} AND {ON_CHAIN} AND ({st} = 'pending' OR \
             ({NO_CURATION} AND NOT {APPROVED_ON_CHAIN} AND NOT {REVIEWED_ON_CHAIN})))) AND NOT {auto})"
        ),
        "under_review" => format!(
            "({NEWEST_ASSIGNEE} IS NOT NULL AND ({st} = 'pending' OR ({THIRD_PARTY} AND \
             {PENDING_ITEM_CURATION})) AND NOT {auto})"
        ),
        "auto_reviewing" => format!("({STANDARD} AND {auto})"),
        _ => format!(
            "({ON_CHAIN_UNAPPROVED} AND ({st} = 'approved' OR ({NO_CURATION} AND {REVIEWED_ON_CHAIN})))"
        ),
    }
}

/// A listing request, validated.
pub struct Listing {
    filters: Vec<Filter>,
    published_only: bool,
    address: Option<String>,
    providers: Vec<String>,
    status: Option<&'static str>,
    sort: Sort,
    review_flow: bool,
    page: Page,
}

fn invalid(error: &str, data: Value) -> ApiError {
    ApiError::bad_request(error).with_data(data)
}

fn common(q: &ListQuery, filters: &mut Vec<Filter>) -> Result<bool, ApiError> {
    if let Some(text) = q.non_empty("q")? {
        if text.chars().count() > Q_MAX {
            return Err(invalid("Invalid q", json!({ "max": Q_MAX })));
        }
        let pattern = text
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        filters.push(Filter::Name(pattern));
    }
    match q.non_empty("type")? {
        None => {}
        Some("standard") => filters.push(Filter::Fragment("c.third_party_id IS NULL")),
        Some("third_party") => filters.push(Filter::Fragment("c.third_party_id IS NOT NULL")),
        Some(other) => return Err(invalid("Invalid type", json!({ "type": other }))),
    }
    match q.non_empty("is_published")? {
        Some("true") => Ok(true),
        Some(_) => {
            filters.push(Filter::Fragment(UNPUBLISHED));
            Ok(false)
        }
        None => Ok(false),
    }
}

fn distinct(values: Vec<&str>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for v in values.into_iter().filter(|v| !v.is_empty()) {
        if !out.iter().any(|seen| seen == v) {
            out.push(v.to_string());
        }
    }
    out
}

impl Listing {
    /// `GET /v1/collections`: `getReviewFilters`, then the shared filters,
    /// the item tags and the sort.
    fn everyone(q: &ListQuery, review_flow: bool) -> Result<Self, ApiError> {
        let mut filters = Vec::new();
        let statuses = distinct(q.all("status"));
        if let Some(bad) = statuses
            .iter()
            .find(|s| !CURATION_STATUSES.contains(&s.as_str()))
        {
            return Err(invalid(
                "Invalid status",
                json!({ "status": bad, "validStatuses": CURATION_STATUSES }),
            ));
        }
        let review_types = distinct(q.all("review_type"));
        if review_types.len() > 1 {
            return Err(invalid(
                "Only one review type can be given",
                json!({ "review_type": review_types }),
            ));
        }
        if let Some(bad) = review_types
            .first()
            .filter(|t| !REVIEW_TYPES.contains(&t.as_str()))
        {
            return Err(invalid(
                "Invalid review type",
                json!({ "review_type": bad, "validTypes": REVIEW_TYPES }),
            ));
        }
        let assignees: Vec<String> = distinct(q.all("assignee"))
            .into_iter()
            .map(|a| {
                if a == UNASSIGNED {
                    a
                } else {
                    a.to_ascii_lowercase()
                }
            })
            .collect();
        if assignees.len() > MAX_ASSIGNEES {
            return Err(invalid(
                "Too many assignees",
                json!({ "max": MAX_ASSIGNEES }),
            ));
        }
        if let Some(bad) = assignees
            .iter()
            .find(|a| *a != UNASSIGNED && !dcl_auth::is_eth_address(a))
        {
            return Err(invalid("Invalid assignee", json!({ "assignee": bad })));
        }
        let published_only = common(q, &mut filters)?;
        if !assignees.is_empty() {
            let unassigned = assignees.iter().any(|a| a == UNASSIGNED);
            let curators = assignees.into_iter().filter(|a| a != UNASSIGNED).collect();
            filters.push(Filter::Assignees(curators, unassigned));
        }
        if !statuses.is_empty() {
            let conditions = statuses
                .iter()
                .map(|status| curation_condition(status, review_flow))
                .collect::<Vec<_>>()
                .join(" OR ");
            filters.push(Filter::Sql(format!("({conditions})")));
        }
        match review_types.first().map(String::as_str) {
            Some("updates") => filters.push(Filter::Fragment(
                "(COALESCE(chain.is_approved, false) OR (chain.contract_address IS NOT NULL \
                 AND NOT chain.is_approved AND chain.reviewed_at <> chain.created_at))",
            )),
            Some(_) => filters.push(Filter::Fragment(
                "NOT (COALESCE(chain.is_approved, false) OR (chain.contract_address IS NOT NULL \
                 AND NOT chain.is_approved AND chain.reviewed_at <> chain.created_at))",
            )),
            None => {}
        }
        let tags: Vec<String> = match q.all("tag").as_slice() {
            [""] => Vec::new(),
            tags => tags.iter().map(|t| t.to_lowercase()).collect(),
        };
        if tags.len() > MAX_TAGS {
            return Err(invalid("Too many tags", json!({ "max": MAX_TAGS })));
        }
        if tags.iter().any(|t| t.chars().count() > TAG_MAX) {
            return Err(invalid("Invalid tag", json!({ "max": TAG_MAX })));
        }
        if !tags.is_empty() {
            filters.push(Filter::Tags(tags));
        }
        Ok(Self {
            filters,
            published_only,
            address: None,
            providers: Vec::new(),
            status: None,
            sort: q.non_empty("sort")?.map_or(Sort::Unordered, Sort::named),
            review_flow,
            page: page(q, COMMITTEE_CAP)?,
        })
    }

    /// `GET /v1/{address}/collections`: the address's rows and the shared
    /// filters, its `contract_address` and `status`; newest first by
    /// default.
    fn of(wallet: &Wallet, q: &ListQuery) -> Result<Self, ApiError> {
        let mut filters = Vec::new();
        if let Some(raw) = q.one("contract_address")? {
            if !dcl_auth::is_eth_address(raw) {
                return Err(invalid(
                    "Invalid contract address",
                    json!({ "contract_address": raw }),
                ));
            }
            filters.push(Filter::Contract(raw.to_ascii_lowercase()));
        }
        let status = match q.all("status").as_slice() {
            [] => None,
            [one] => Some(
                COLLECTION_STATUSES
                    .into_iter()
                    .find(|s| s == one)
                    .ok_or_else(|| {
                        invalid(
                            "Invalid status",
                            json!({ "status": one, "validStatuses": COLLECTION_STATUSES }),
                        )
                    })?,
            ),
            many => {
                return Err(invalid(
                    "Invalid status",
                    json!({ "status": many, "validStatuses": COLLECTION_STATUSES }),
                ));
            }
        };
        if common(q, &mut filters)? {
            filters.push(Filter::Fragment(PUBLISHED));
        }
        Ok(Self {
            filters,
            published_only: false,
            address: Some(wallet.address().to_string()),
            providers: wallet.provider_ids(),
            status,
            sort: q
                .non_empty("sort")?
                .map_or(Sort::CreatedAtDesc, Sort::named),
            review_flow: false,
            page: page(q, ADDRESS_CAP)?,
        })
    }

    /// The published collections whose name matches, probed one by one
    /// from the published ids, so a search never reads a draft.
    fn published_ids(&self) -> QueryBuilder<Postgres> {
        let mut sql = QueryBuilder::new("SELECT c.id FROM ");
        sql.push(PUBLISHED_IDS);
        if let Some(pattern) = self.name() {
            sql.push(" AND c0.name ILIKE '%' || ")
                .push_bind(pattern.to_string())
                .push(" || '%' ESCAPE '\\'");
        }
        sql.push(" OFFSET 0) c");
        sql
    }

    fn name(&self) -> Option<&str> {
        self.filters.iter().find_map(|f| match f {
            Filter::Name(pattern) => Some(pattern.as_str()),
            _ => None,
        })
    }

    fn push_where(
        &self,
        sql: &mut QueryBuilder<Postgres>,
        ids: Option<&[Uuid]>,
        with_status: bool,
    ) {
        sql.push(" WHERE true");
        if let Some(ids) = ids {
            sql.push(" AND c.id = ANY(")
                .push_bind(ids.to_vec())
                .push(")");
        }
        if let Some(address) = &self.address {
            sql.push(" AND (chain.contract_address IS NULL OR chain.creator = ")
                .push_bind(address.clone())
                .push(
                    " OR EXISTS (SELECT 1 FROM unnest(chain.managers || chain.minters) r WHERE lower(r) = ",
                )
                .push_bind(address.clone())
                .push("))");
        }
        for filter in &self.filters {
            if self.published_only && matches!(filter, Filter::Name(_)) {
                continue;
            }
            sql.push(" AND ");
            match filter {
                Filter::Fragment(fragment) => {
                    sql.push(*fragment);
                }
                Filter::Sql(condition) => {
                    sql.push(condition.as_str());
                }
                Filter::Assignees(curators, unassigned) => {
                    sql.push("(")
                        .push(NEWEST_ASSIGNEE)
                        .push(" = ANY(")
                        .push_bind(curators.clone())
                        .push(")");
                    if *unassigned {
                        sql.push(" OR ").push(NEWEST_ASSIGNEE).push(" IS NULL");
                    }
                    sql.push(")");
                }
                Filter::Name(pattern) => {
                    sql.push("c.name ILIKE '%' || ")
                        .push_bind(pattern.clone())
                        .push(" || '%' ESCAPE '\\'");
                }
                Filter::Tags(tags) => {
                    sql.push(
                        "EXISTS (SELECT 1 FROM builder.items t WHERE t.collection_id = c.id \
                         AND json_typeof(t.data::json->'tags') = 'array' AND EXISTS (SELECT 1 FROM \
                         json_array_elements_text(t.data::json->'tags') tag WHERE lower(tag) = ANY(",
                    )
                    .push_bind(tags.clone())
                    .push(")) OFFSET 0)");
                }
                Filter::Contract(contract) => {
                    sql.push("c.contract_address = ")
                        .push_bind(contract.clone());
                }
            }
        }
        if with_status && let Some(status) = self.status {
            sql.push(" AND ")
                .push(STATUS.as_str())
                .push(" = ")
                .push_bind(status);
        }
    }
}

#[derive(sqlx::FromRow)]
struct ListedRow {
    #[sqlx(flatten)]
    collection: CollectionRow,
    curation_status: Option<String>,
    last_activity_at: chrono::NaiveDateTime,
}

#[derive(Serialize)]
struct Paginated<T> {
    total: i64,
    limit: i64,
    pages: i64,
    page: i64,
    results: Vec<T>,
}

/// The committee's listing: every collection, any filter, full rows.
pub struct CommitteeListing {
    listing: Listing,
}

/// The public search: published collections only, public rows.
pub struct PublicSearch {
    listing: Listing,
}

/// `GET /v1/addresses`: the contract addresses of published standard
/// collections, by the public search's filters.
pub struct PublishedAddresses {
    listing: Listing,
}

/// An address's own listing: the signer's collections and the collections
/// of the providers it manages, full rows.
pub struct OwnListing {
    listing: Listing,
    programmatic: HashMap<String, bool>,
}

impl CommitteeListing {
    /// The listing `q` asks for, for a committee member; `review_flow` says
    /// whether the review flow is on ([`crate::review::enabled`]).
    pub fn new(
        _committee: &OnCommittee,
        q: &ListQuery,
        review_flow: bool,
    ) -> Result<Self, ApiError> {
        Ok(Self {
            listing: Listing::everyone(q, review_flow)?,
        })
    }
}

impl PublicSearch {
    /// The search `q` asks for, or `None` when `q` is no public search.
    pub fn new(q: &ListQuery, review_flow: bool) -> Result<Option<Self>, ApiError> {
        if !q.is_public_search() {
            return Ok(None);
        }
        let listing = Listing::everyone(q, review_flow)?;
        Ok(listing.published_only.then_some(Self { listing }))
    }
}

impl PublishedAddresses {
    /// The listing `q` asks for: its `type` ignored, standard collections
    /// only, published ones only.
    pub fn new(q: &ListQuery, review_flow: bool) -> Result<Self, ApiError> {
        let mut listing = Listing::everyone(&q.without("type"), review_flow)?;
        listing
            .filters
            .push(Filter::Fragment("c.third_party_id IS NULL"));
        listing.published_only = true;
        Ok(Self { listing })
    }
}

impl OwnListing {
    /// The signer's own listing `q` asks for.
    pub fn new(wallet: &Wallet, q: &ListQuery) -> Result<Self, ApiError> {
        Ok(Self {
            listing: Listing::of(wallet, q)?,
            programmatic: wallet.providers().iter().cloned().collect(),
        })
    }
}

fn listing_row(
    network: &crate::network::Network,
    r: ListedRow,
    programmatic: Option<bool>,
) -> CollectionListingRowOut {
    CollectionListingRowOut {
        collection: r.collection.out(network, programmatic),
        curation_status: r.curation_status,
        last_activity_at: crate::http::iso(&r.last_activity_at.and_utc()),
    }
}

/// The listing's rows; a third-party row of a provider the registry has
/// carries the provider's `is_programmatic`, so a page that holds one reads
/// the registry after its rows, with no connection held.
async fn programmatic_rows(
    network: &crate::network::Network,
    rows: Vec<ListedRow>,
    state: &crate::State,
) -> Result<Vec<CollectionListingRowOut>, ApiError> {
    let registry = match rows.iter().any(|r| r.collection.third_party()) {
        true => Some(state.third_parties.registry(state, Since::read()).await?),
        false => None,
    };
    Ok(rows
        .into_iter()
        .map(|r| {
            let programmatic = r
                .collection
                .provider()
                .zip(registry.as_deref())
                .and_then(|(id, registry)| provider(registry, id))
                .map(|p| p.is_programmatic);
            listing_row(network, r, programmatic)
        })
        .collect())
}

impl ItemsComponent {
    async fn list(
        &self,
        listing: &Listing,
        view_only: bool,
    ) -> Result<(Vec<ListedRow>, i64, Option<Value>), ApiError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
            .execute(&mut *tx)
            .await?;
        sqlx::query("SET LOCAL jit = off").execute(&mut *tx).await?;
        let view: ReadView = chain_state::for_read(&mut tx).await?;
        let ids: Option<Vec<Uuid>> = match &listing.address {
            Some(address) => Some(
                sqlx::query_scalar(ADDRESS_IDS)
                    .bind(address)
                    .bind(&listing.providers)
                    .fetch_all(&mut *tx)
                    .await?,
            ),
            None if listing.published_only => Some(
                listing
                    .published_ids()
                    .build_query_scalar()
                    .fetch_all(&mut *tx)
                    .await?,
            ),
            None => None,
        };
        let ids = ids.as_deref();
        let mut sql = QueryBuilder::new("SELECT ");
        sql.push(body_columns(&view))
            .push(", ")
            .push(CURATION_STATUS)
            .push(" AS curation_status, ")
            .push(LAST_ACTIVITY)
            .push(" AS last_activity_at FROM ")
            .push(view.collections())
            .push(" WHERE c.id = ANY(ARRAY(SELECT c.id FROM ")
            .push(view.collections());
        listing.push_where(&mut sql, ids, true);
        let order = listing.sort.order_by(listing.review_flow);
        sql.push(order.as_str())
            .push(" LIMIT ")
            .push_bind(listing.page.limit)
            .push(" OFFSET ")
            .push_bind(listing.page.offset)
            .push("))")
            .push(order.as_str());
        let rows: Vec<ListedRow> = sql.build_query_as().fetch_all(&mut *tx).await?;
        let mut total = i64::try_from(rows.len())
            .map_err(|_| ApiError::internal("a listing page past i64 rows"))?;
        let mut counts = None;
        if listing.page.page.is_some() {
            let mut count = QueryBuilder::new("SELECT count(*) FROM ");
            count.push(view.collections());
            listing.push_where(&mut count, ids, true);
            total = count.build_query_scalar().fetch_one(&mut *tx).await?;
            if !view_only {
                let mut by_status = QueryBuilder::new("SELECT ");
                by_status
                    .push(STATUS.as_str())
                    .push(" AS status, count(*) FROM ");
                by_status.push(view.collections());
                listing.push_where(&mut by_status, ids, false);
                by_status.push(" GROUP BY 1");
                let rows: Vec<(Option<String>, i64)> =
                    by_status.build_query_as().fetch_all(&mut *tx).await?;
                let mut map = serde_json::Map::new();
                for status in COLLECTION_STATUSES {
                    let n = rows
                        .iter()
                        .find(|(s, _)| s.as_deref() == Some(status))
                        .map_or(0, |(_, n)| *n);
                    map.insert(status.to_string(), json!(n));
                }
                counts = Some(Value::Object(map));
            }
        }
        tx.commit().await?;
        Ok((rows, total, counts))
    }

    fn envelope<T: Serialize>(
        listing: &Listing,
        results: Vec<T>,
        total: i64,
        counts: Option<Value>,
    ) -> Result<Value, ApiError> {
        let Some(page) = listing.page.page else {
            return project(results, None);
        };
        let limit = listing.page.limit;
        let mut body = project(
            Paginated {
                total,
                limit,
                pages: (total + limit - 1) / limit,
                page,
                results,
            },
            None,
        )?;
        if let Some(counts) = counts {
            body["counts"] = counts;
        }
        Ok(body)
    }

    /// `GET /v1/collections` for a committee member, each row with its
    /// provider's `is_programmatic` ([`programmatic_rows`]).
    pub async fn committee_collections(
        &self,
        request: &CommitteeListing,
        state: &crate::State,
    ) -> Result<Value, ApiError> {
        let (rows, total, _) = self.list(&request.listing, true).await?;
        let results = programmatic_rows(&self.network, rows, state).await?;
        Self::envelope(&request.listing, results, total, None)
    }

    /// `GET /v1/collections?q=..&is_published=true` for anyone: the public
    /// rows of published collections, each with its provider's
    /// `is_programmatic` ([`programmatic_rows`]).
    pub async fn public_collections(
        &self,
        search: &PublicSearch,
        state: &crate::State,
    ) -> Result<Value, ApiError> {
        let (rows, total, _) = self.list(&search.listing, true).await?;
        let results: Vec<Value> = programmatic_rows(&self.network, rows, state)
            .await?
            .into_iter()
            .map(|r| project(r, Some(PUBLIC_LISTING)))
            .collect::<Result<_, _>>()?;
        Self::envelope(&search.listing, results, total, None)
    }

    /// `GET /v1/addresses`: the listed collections' contract addresses.
    pub async fn published_addresses(
        &self,
        request: &PublishedAddresses,
    ) -> Result<Value, ApiError> {
        let (rows, total, _) = self.list(&request.listing, true).await?;
        let results: Vec<Option<String>> = rows
            .into_iter()
            .map(|r| r.collection.contract_address().map(str::to_string))
            .collect();
        Self::envelope(&request.listing, results, total, None)
    }

    /// `GET /v1/{address}/collections`: the full rows, and the status counts
    /// when paginated.
    pub async fn address_collections(&self, request: &OwnListing) -> Result<Value, ApiError> {
        let (rows, total, counts) = self.list(&request.listing, false).await?;
        let results: Vec<CollectionListingRowOut> = rows
            .into_iter()
            .map(|r| {
                let programmatic = r
                    .collection
                    .provider()
                    .map(|id| request.programmatic.get(id).copied().unwrap_or(false));
                listing_row(&self.network, r, programmatic)
            })
            .collect();
        Self::envelope(&request.listing, results, total, counts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(raw: &str) -> ListQuery {
        ListQuery::parse(Some(raw))
    }

    #[test]
    fn pagination_reads_its_numbers_as_builder_server_does_and_caps_them() {
        let p = |raw: &str| page(&query(raw), COMMITTEE_CAP);
        assert_eq!(
            p("").unwrap(),
            Page {
                limit: COMMITTEE_CAP,
                offset: 0,
                page: None
            }
        );
        assert_eq!(
            p("page=2&limit=12").unwrap(),
            Page {
                limit: 12,
                offset: 12,
                page: Some(2)
            }
        );
        assert_eq!(
            p("page=2abc&limit= 12x").unwrap(),
            Page {
                limit: 12,
                offset: 12,
                page: Some(2)
            }
        );
        assert_eq!(
            p("limit=5").unwrap(),
            Page {
                limit: 5,
                offset: 0,
                page: None
            }
        );
        assert_eq!(
            p("page=0&limit=5").unwrap(),
            Page {
                limit: 5,
                offset: 0,
                page: None
            }
        );
        assert_eq!(
            p("limit=0").unwrap(),
            Page {
                limit: 0,
                offset: 0,
                page: None
            }
        );
        assert_eq!(p("page=1&limit=99999").unwrap().limit, COMMITTEE_CAP);
        for unpaged in ["page=1&limit=100000", "page=1&limit=abc", "page=2"] {
            assert_eq!(
                p(unpaged).unwrap(),
                Page {
                    limit: COMMITTEE_CAP,
                    offset: 0,
                    page: None
                },
                "{unpaged}"
            );
        }
        assert_eq!(
            p("page=99999999999999999999&limit=10").unwrap().offset,
            i64::MAX
        );
        for refused in ["limit=-1", "page=-2&limit=3", "page=1&page=2"] {
            assert_eq!(p(refused).unwrap_err().status(), 400, "{refused}");
        }
    }

    #[test]
    fn a_hostile_sort_or_filter_value_never_reaches_the_statement() {
        let hostile = "NAME_ASC; DROP TABLE builder.collections; --";
        let raw = format!(
            "sort={}&q={}&type=standard&is_published=false&tag={}&status=to_review",
            url::form_urlencoded::byte_serialize(hostile.as_bytes()).collect::<String>(),
            url::form_urlencoded::byte_serialize(hostile.as_bytes()).collect::<String>(),
            url::form_urlencoded::byte_serialize(hostile.as_bytes()).collect::<String>(),
        );
        let listing = Listing::everyone(&query(&raw), false).unwrap();
        assert_eq!(listing.sort, Sort::Unordered);
        let mut sql = QueryBuilder::<Postgres>::new("SELECT 1 FROM x");
        listing.push_where(&mut sql, None, true);
        sql.push(listing.sort.order_by(false).as_str());
        let text = sql.sql().as_str().to_string();
        assert!(!text.contains("DROP"), "{text}");
        assert!(text.contains("ILIKE '%' || $1 || '%'"), "{text}");
        assert_eq!(Sort::named("name_asc"), Sort::Unordered);
        assert!(Listing::everyone(&query("type=x"), false).is_err());
        assert!(Listing::everyone(&query(&format!("q={}", "a".repeat(Q_MAX + 1))), false).is_err());
        assert!(Listing::everyone(&query(&format!("q={}", "ä".repeat(Q_MAX))), false).is_ok());
    }

    #[test]
    fn the_review_filters_refuse_as_builder_server_does() {
        let status = |raw: &str| {
            Listing::everyone(&query(raw), false)
                .err()
                .map(|e| (e.status(), e.to_string()))
        };
        assert_eq!(status("status=nope"), Some((400, "Invalid status".into())));
        assert_eq!(
            status("review_type=new&review_type=updates"),
            Some((400, "Only one review type can be given".into()))
        );
        assert_eq!(
            status("review_type=old"),
            Some((400, "Invalid review type".into()))
        );
        assert_eq!(
            status("assignee=nope"),
            Some((400, "Invalid assignee".into()))
        );
        let many = (0..21)
            .map(|i| format!("assignee=0x{i:040x}"))
            .collect::<Vec<_>>()
            .join("&");
        assert_eq!(status(&many), Some((400, "Too many assignees".into())));
        assert_eq!(
            status("status=pending&status=pending&review_type=new"),
            None
        );
        let tags = (0..6)
            .map(|i| format!("tag=t{i}"))
            .collect::<Vec<_>>()
            .join("&");
        assert_eq!(status(&tags), Some((400, "Too many tags".into())));
        assert_eq!(
            status(&format!("tag={}", "t".repeat(65))),
            Some((400, "Invalid tag".into()))
        );
        assert_eq!(status(&format!("tag={}", "ä".repeat(64))), None);
    }
}
