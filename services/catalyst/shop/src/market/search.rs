// from: marketplace-server src/logic/catalog/creator-profiles.ts (the creator search), src/logic/catalog/collection-search.ts, src/ports/search-suggest/component.ts, src/controllers/handlers/creator-search-handler.ts, src/controllers/handlers/search-suggest-handler.ts
//! The shop's search dropdown: `/v3/catalog/creators/search` (creators whose
//! profile name or one of their NAMEs matches every term of the query) and
//! `/v3/catalog/suggest` (the items the grid would show for the query, the
//! matching collections and creators, with creator names resolved, in one
//! answer).
//!
//! Upstream matches over precomputed word tables with pg_trgm's
//! `word_similarity`; v2 defers pg_trgm until measured, so a name matches
//! when every normalized query
//! term is inside it, and ranks in the same tiers: a name that IS the query
//! (its words in any order, or run together as a NAME is), then one that
//! starts with it, then any other match. The creator directory (approved
//! creators with their item and collection counts, the active profile's name
//! and face, their NAMEs) is one statement cached for 15 minutes: creators
//! number in the thousands and a dropdown asks on every keystroke. Each read
//! applies the content denylist to the cached profiles, so a new listing does
//! not wait for the next refresh.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::extract::{Query, State};
use axum::http::header::CACHE_CONTROL;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::Row;

use crate::content::Denylist;
use crate::content::queries::non_denylisted;

use super::catalog::escape_like;
use super::catalogue::{self, Access, CatalogueRead, Purpose};
use super::items::{self, CreditCatalogItem};
use super::mirrors::Mirror;
use super::readers::{RefreshClaim, Ttl, memoized_holding};
use super::wire::{ApiError, Params};
use super::{MarketplaceState, SharedState};

/// Words a query drops, but only when it has anything else to search for:
/// "atari x rtfkt" is a search for atari and rtfkt.
const STOPWORDS: &[&str] = &[
    "the", "a", "an", "of", "and", "x", "de", "la", "el", "los", "las", "y",
];
const MAX_TERMS: usize = 6;
pub const CREATOR_SEARCH_DEFAULT_LIMIT: i64 = 4;
pub const CREATOR_SEARCH_MAX_LIMIT: i64 = 10;
/// NAMEs kept per creator: enough to be found by, bounded for the whales.
const CREATOR_MAX_NAMES: usize = 10;
pub const COLLECTION_SUGGEST_DEFAULT_LIMIT: i64 = 4;
pub const COLLECTION_SUGGEST_MAX_LIMIT: i64 = 10;
/// The window the collections' sales tiebreak counts over (the sales reader's).
const COLLECTION_SALES_WINDOW_DAYS: i64 = 90;
pub const SUGGEST_ITEMS_DEFAULT_LIMIT: i64 = 5;
pub const SUGGEST_SECTION_MAX_LIMIT: i64 = 10;
const SUGGEST_CACHE: &str = "public,max-age=60,s-maxage=60";

fn words(value: &str) -> Vec<String> {
    value
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

/// The query as matched: one term per whitespace-separated word collapsed to
/// its letters and digits ("t-shirt" is tshirt), de-duplicated, stopwords
/// dropped when anything else remains, at most six.
pub fn query_terms(search: &str) -> Vec<String> {
    let mut terms: Vec<String> = Vec::new();
    for word in search.split_whitespace() {
        let term: String = words(word).concat();
        if !term.is_empty() && !terms.contains(&term) {
            terms.push(term);
        }
    }
    let kept: Vec<String> = terms
        .iter()
        .filter(|t| !STOPWORDS.contains(&t.as_str()))
        .cloned()
        .collect();
    let mut out = if kept.is_empty() { terms } else { kept };
    out.truncate(MAX_TERMS);
    out
}

/// A string in the shapes a query is compared against.
#[derive(Debug, Clone, PartialEq)]
struct Forms {
    phrase: String,
    sorted: String,
    compact: String,
}

fn forms(value: &str) -> Forms {
    let w = words(value);
    let mut sorted = w.clone();
    sorted.sort();
    Forms {
        phrase: w.join(" "),
        sorted: sorted.join(" "),
        compact: w.concat(),
    }
}

/// How a name answers the query, higher first: 2 it IS the query, 1 it
/// starts with it, 0 every term is inside it; `None` when a term is missing.
fn name_tier(name: &Forms, query: &Forms, terms: &[String]) -> Option<u8> {
    if terms.is_empty() || name.compact.is_empty() {
        return None;
    }
    if !terms
        .iter()
        .all(|t| name.phrase.contains(t.as_str()) || name.compact.contains(t.as_str()))
    {
        return None;
    }
    if name.sorted == query.sorted || name.compact == query.compact {
        return Some(2);
    }
    if name.phrase.starts_with(&query.phrase) || name.compact.starts_with(&query.compact) {
        return Some(1);
    }
    Some(0)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CreatorEntry {
    pub address: String,
    /// The active profile's entity, which the content denylist can withhold.
    #[serde(default)]
    pub entity_id: Option<String>,
    pub name: Option<String>,
    pub face: Option<String>,
    /// Whether the creator holds a NAME spelled as the profile name, as
    /// `/lambdas/profiles` sets `hasClaimedName`, from the NFT index.
    #[serde(default)]
    pub claimed: bool,
    pub names: Vec<String>,
    pub items: i64,
    pub collections: i64,
}

impl CreatorEntry {
    /// What to call the creator: the profile name, else the first NAME.
    pub fn display_name(&self) -> Option<String> {
        self.name.clone().or_else(|| self.names.first().cloned())
    }

    /// The entry without its profile's name, face and claim when the content
    /// denylist lists the profile's entity, as `/lambdas/profiles` then has no
    /// profile to answer. The NAMEs stay: they come from the NFT index.
    fn without_denylisted_profile(mut self, denylist: &Denylist) -> Self {
        if self
            .entity_id
            .as_deref()
            .is_some_and(|id| !non_denylisted(id, denylist))
        {
            self.name = None;
            self.face = None;
            self.claimed = false;
        }
        self
    }
}

/// A creator's profile as a feed carries it inline (`creatorProfile`), so a
/// card names its creator without asking `/lambdas/profiles`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "market/"))]
pub struct CreatorProfile {
    /// The active profile's name; empty without one.
    pub name: String,
    pub face: Option<String>,
    pub claimed: bool,
}

/// How long a feed waits for a creator directory this node has not read yet.
const DIRECTORY_WAIT: Duration = Duration::from_millis(500);
/// How old a kept directory a feed still serves while it is refreshed (a failed refresh keeps
/// the copy).
const DIRECTORY_STALE_FOR: Duration = Duration::from_secs(3600);
/// The longest a feed-started directory read may take before it ends as failed.
#[cfg(not(test))]
const DIRECTORY_READ_LIMIT: Duration = Duration::from_secs(60);
/// Every test's cold read runs under this limit, so it leaves a loaded runner room for a read
/// that starts behind a held lock, and still ends inside the hung-read test's 20 s poll.
#[cfg(test)]
const DIRECTORY_READ_LIMIT: Duration = Duration::from_secs(10);
/// How long after a failed directory read the next one waits, so a failing database sees one
/// directory read per this period rather than one per feed.
const DIRECTORY_RETRY_AFTER: Duration = Duration::from_secs(30);

/// The creator directory for a feed, which it never holds up: the fresh copy; else the last good
/// one (kept up to [`DIRECTORY_STALE_FOR`]) while one background read refreshes it; else, with
/// none yet, that read for at most [`DIRECTORY_WAIT`], and only by the feed that started it.
/// `None` answers without inline profiles, and the cards ask for them. Only one read runs at a
/// time, and a failed one is retried after [`DIRECTORY_RETRY_AFTER`]. `sources` is what the
/// feed's own catalogue read saw ([`CatalogueRead::sources`]): a directory read from another
/// source (before an ownership change) is never served.
pub async fn directory_snapshot(state: &SharedState, sources: &str) -> Option<Arc<Value>> {
    let readers = &state.readers;
    let key = directory_key(sources);
    if let Some(fresh) = readers.fresh(Ttl::M15, &key) {
        return Some(fresh);
    }
    let refresh = readers
        .begin_refresh(&key, DIRECTORY_RETRY_AFTER)
        .map(|id| {
            // Released however the read ends, a panic included, and even when the task never
            // runs (a runtime shutting down drops it).
            let mut claim = RefreshClaim::new(Arc::clone(state), key.clone(), id);
            let state = Arc::clone(state);
            let sources = sources.to_string();
            tokio::spawn(async move {
                // A read that hangs (a stuck connection, a lock) ends as failed at the limit, so
                // the claim falls to the backoff instead of being held until a restart.
                let read = tokio::time::timeout(DIRECTORY_READ_LIMIT, async {
                    let read = catalogue::begin(
                        &state,
                        Purpose::Describe,
                        &[Mirror::Items],
                        None,
                        Access::ReadOnly,
                    )
                    .await
                    .map_err(|_| ApiError::Internal("the catalogue cannot be read".into()))?;
                    let read_sources = read.sources();
                    let directory = directory_value(&state, &mut Some(read))
                        .await?
                        .ok_or_else(|| ApiError::Internal("the directory read failed".into()))?;
                    // Read after an ownership change: filed under its own sources, and not
                    // this claim's.
                    if read_sources != sources {
                        return Err(ApiError::Internal(
                            "the catalogue's sources changed during the directory read".into(),
                        ));
                    }
                    Ok(directory)
                })
                .await
                .unwrap_or_else(|_| {
                    Err(ApiError::Internal(format!(
                        "the creator directory read took over {DIRECTORY_READ_LIMIT:?}"
                    )))
                });
                match &read {
                    Ok(_) => claim.succeeded(),
                    Err(e) => tracing::warn!(error = ?e, "creator directory unavailable; feeds answer without creator profiles"),
                }
                read.ok()
            })
        });
    if let Some(kept) = readers.kept_within(&key, DIRECTORY_STALE_FOR) {
        return Some(kept);
    }
    // With no copy yet, a feed that did not start the read answers without profiles rather than
    // leading a read of its own, which its wait would cancel.
    let read = refresh?;
    tokio::time::timeout(DIRECTORY_WAIT, read)
        .await
        .ok()
        .and_then(Result::ok)
        .flatten()
}

/// The profile of each of `addresses` (lowercase) the directory holds with an active profile,
/// after the content denylist. A creator with none is left out, so the card asks
/// `/lambdas/profiles` and shows a profile deployed since the directory was read; a denylisted
/// profile still answers empty.
pub fn creator_profiles(
    directory: &Value,
    addresses: &HashSet<String>,
    denylist: &Denylist,
) -> HashMap<String, CreatorProfile> {
    directory
        .as_array()
        .into_iter()
        .flatten()
        .filter(|e| {
            e.get("address")
                .and_then(Value::as_str)
                .is_some_and(|a| addresses.contains(a))
        })
        .filter_map(|e| CreatorEntry::deserialize(e).ok())
        .filter(|c| c.entity_id.is_some())
        .map(|c| {
            let c = c.without_denylisted_profile(denylist);
            let profile = CreatorProfile {
                name: c.name.unwrap_or_default(),
                face: c.face,
                claimed: c.claimed,
            };
            (c.address, profile)
        })
        .collect()
}

/// A creator the dropdown can offer.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "market/"))]
pub struct CreatorSearchHit {
    pub address: String,
    pub name: String,
    pub face: Option<String>,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub items: i64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub collections: i64,
}

const CREATOR_DIRECTORY_SQL: &str = "WITH creators AS ( \
       SELECT lower(creator) AS address, count(*)::int8 AS items, count(DISTINCT contract_address)::int8 AS collections \
       FROM shop.items WHERE search_is_collection_approved = true AND creator IS NOT NULL GROUP BY 1 \
     ) \
     SELECT c.address, c.items, c.collections, d.entity_id, \
            NULLIF(trim(d.entity_metadata->'v'->'avatars'->0->>'name'), '') AS name, \
            d.entity_metadata->'v'->'avatars'->0->'avatar'->'snapshots'->>'face256' AS face, \
            EXISTS (SELECT 1 FROM shop.nft n WHERE n.owner = c.address \
                    AND n.ens_subdomain = d.entity_metadata->'v'->'avatars'->0->>'name') AS claimed, \
            COALESCE((SELECT array_agg(n.ens_subdomain ORDER BY n.minted_block, n.ens_subdomain) FROM shop.nft n \
                      WHERE n.owner = c.address AND n.ens_subdomain IS NOT NULL), '{}') AS names \
     FROM creators c \
     LEFT JOIN content.active_pointers ap ON ap.pointer = c.address \
     LEFT JOIN content.deployments d ON d.entity_id = ap.entity_id";

/// The creator's face: the CDN's or this node's `/profile-images` for the
/// active profile; one it never uploaded only where something derives it.
fn face_url(state: &MarketplaceState, entity_id: &str, face: Option<&str>) -> Option<String> {
    let face = match face.filter(|f| !f.is_empty()) {
        Some(face) => face,
        None if state.derives_faces() => "",
        None => return None,
    };
    let mut metadata = json!({ "avatars": [{ "avatar": { "snapshots": { "face256": face } } }] });
    super::profiles::rewrite_snapshot_urls(
        entity_id,
        &mut metadata,
        state.settings.profile_cdn_base_url.as_deref(),
        &state.settings.profile_images_public_url,
    );
    metadata["avatars"][0]["avatar"]["snapshots"]["face256"]
        .as_str()
        .map(str::to_string)
}

/// The cached directory, read in `read`'s snapshot, after the content denylist.
pub async fn creator_directory(
    state: &MarketplaceState,
    held: &mut Option<CatalogueRead>,
) -> Result<Option<Vec<CreatorEntry>>, ApiError> {
    let Some(value) = directory_value(state, held).await? else {
        return Ok(None);
    };
    let entries: Vec<CreatorEntry> = serde_json::from_value(Arc::unwrap_or_clone(value))
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Some(
        entries
            .into_iter()
            .map(|c| c.without_denylisted_profile(&state.denylist))
            .collect(),
    ))
}

/// The creator directory's memo key prefix, under [`Ttl::M15`].
pub(crate) const CREATOR_DIRECTORY_KEY: &str = "search:creator-directory";

/// The directory's memo key for the catalogue `sources` it was read from
/// ([`CatalogueRead::sources`]). A display aggregate: within one source it
/// keeps its stated staleness ([`Ttl::M15`], kept up to
/// [`DIRECTORY_STALE_FOR`]); an ownership change, a hand-back included,
/// files it anew.
pub(crate) fn directory_key(sources: &str) -> String {
    format!("{CREATOR_DIRECTORY_KEY}:{sources}")
}

/// The directory memo holds the read only in the request that computes it:
/// one that waits on another's computation drops it first (`held` becomes
/// `None`). `Ok(None)`: that computation failed, and the caller opens a new
/// read and asks again.
async fn directory_value(
    state: &MarketplaceState,
    held: &mut Option<CatalogueRead>,
) -> Result<Option<Arc<Value>>, ApiError> {
    let key = directory_key(&held.as_ref().expect("a request holds its read").sources());
    memoized_holding(
        state,
        Ttl::M15,
        key.clone(),
        held,
        async |read: &mut CatalogueRead| {
            let rows = sqlx::query(CREATOR_DIRECTORY_SQL)
                .fetch_all(&mut *read.tx)
                .await
                .map_err(|e| e.to_string())?;
            let entries: Vec<CreatorEntry> = rows
                .iter()
                .map(|r| {
                    let entity_id: Option<String> = r.get("entity_id");
                    let face: Option<String> = r.get("face");
                    let mut names: Vec<String> = r.get("names");
                    names.truncate(CREATOR_MAX_NAMES);
                    CreatorEntry {
                        address: r.get("address"),
                        name: r.get("name"),
                        face: entity_id
                            .as_deref()
                            .and_then(|id| face_url(state, id, face.as_deref())),
                        entity_id,
                        claimed: r.get("claimed"),
                        names,
                        items: r.get("items"),
                        collections: r.get("collections"),
                    }
                })
                .collect();
            state.warm_faces(
                rows.iter()
                    .filter_map(|r| r.get::<Option<String>, _>("entity_id")),
            );
            serde_json::to_value(entries).map_err(|e| e.to_string())
        },
    )
    .await
    .map_err(ApiError::Internal)
    .inspect(|directory| {
        if let Some(directory) = directory {
            state.readers.keep(&key, Arc::clone(directory));
        }
    })
}

/// Creators whose profile name or a NAME matches every term, best first:
/// the tier, then how much they have published (a tie goes to the one with
/// a shop to browse), then name and address so pages are stable.
pub fn rank_creators(
    directory: &[CreatorEntry],
    search: &str,
    first: usize,
) -> Vec<CreatorSearchHit> {
    let terms = query_terms(search);
    let query = forms(&terms.join(" "));
    let mut hits: Vec<(u8, &CreatorEntry)> = directory
        .iter()
        .filter_map(|c| {
            let tier = c
                .name
                .iter()
                .chain(c.names.iter())
                .filter_map(|n| name_tier(&forms(n), &query, &terms))
                .max()?;
            Some((tier, c))
        })
        .collect();
    hits.sort_by(|(ta, a), (tb, b)| {
        tb.cmp(ta)
            .then(b.items.cmp(&a.items))
            .then(b.collections.cmp(&a.collections))
            .then(a.display_name().cmp(&b.display_name()))
            .then(a.address.cmp(&b.address))
    });
    hits.into_iter()
        .take(first)
        .map(|(_, c)| CreatorSearchHit {
            address: c.address.clone(),
            name: c.display_name().unwrap_or_else(|| c.address.clone()),
            face: c.face.clone(),
            items: c.items,
            collections: c.collections,
        })
        .collect()
}

fn clamp(value: Option<f64>, default: i64, max: i64) -> usize {
    super::casts::page(
        value
            .filter(|v| v.is_finite())
            .map(super::casts::saturating_i64)
            .unwrap_or(default)
            .clamp(1, max),
    )
}

/// `GET /v3/catalog/creators/search?search=&first=`: nothing typed is nobody
/// suggested, not an error.
pub async fn get_creator_search(
    State(state): State<SharedState>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let p = Params::new(&pairs);
    let search = p.get_string("search", None).unwrap_or_default();
    let first = clamp(
        p.get_number("first", None),
        CREATOR_SEARCH_DEFAULT_LIMIT,
        CREATOR_SEARCH_MAX_LIMIT,
    );
    // The directory memo holds the read only in the request that computes
    // it; one that waits drops it first, and starts again if that failed.
    let directory = loop {
        let read =
            match catalogue::preflight(&state, Purpose::Describe, &[Mirror::Items], None).await {
                Ok(read) => read,
                Err(envelope) => return envelope,
            };
        let stamp = read.stamp();
        if query_terms(&search).is_empty() {
            stamp.note(&state);
            return (
                [(CACHE_CONTROL, SUGGEST_CACHE)],
                Json(json!({ "data": [] })),
            )
                .into_response();
        }
        match creator_directory(&state, &mut Some(read)).await {
            Ok(Some(directory)) => {
                stamp.note(&state);
                break directory;
            }
            Ok(None) => continue,
            Err(e) => return e.into_response(),
        }
    };
    (
        [(CACHE_CONTROL, SUGGEST_CACHE)],
        Json(json!({ "data": rank_creators(&directory, &search, first) })),
    )
        .into_response()
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "market/"))]
pub struct SuggestedCollection {
    #[cfg_attr(feature = "ts", ts(rename = "contractAddress"))]
    pub contract_address: String,
    pub name: String,
    pub creator: String,
    #[cfg_attr(feature = "ts", ts(rename = "creatorName"))]
    pub creator_name: Option<String>,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub items: i64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub sales: i64,
}

/// Approved collections with at least one approved item whose name holds
/// every term (the bounded ILIKE pre-filter), ranked in the name tiers,
/// then by approved items, then by sales in the window, then name and
/// address.
pub async fn search_collections(
    conn: &mut sqlx::PgConnection,
    search: &str,
    first: usize,
    directory: &[CreatorEntry],
    sales: &HashMap<String, i64>,
) -> Result<Vec<SuggestedCollection>, ApiError> {
    let terms = query_terms(search);
    if terms.is_empty() {
        return Ok(Vec::new());
    }
    let patterns: Vec<String> = terms
        .iter()
        .map(|t| format!("%{}%", escape_like(t)))
        .collect();
    let rows = sqlx::query(
        "SELECT c.contract_address, c.name, lower(c.creator) AS creator, count(*)::int8 AS items \
         FROM shop.collections c \
         JOIN shop.items i ON i.network = c.network AND i.contract_address = c.contract_address AND i.search_is_collection_approved = true \
         WHERE c.is_approved = true \
           AND (SELECT bool_and(regexp_replace(lower(c.name), '[^[:alnum:]]+', '', 'g') LIKE p OR lower(c.name) LIKE p) FROM unnest($1::text[]) AS p) \
         GROUP BY c.contract_address, c.name, c.creator",
    )
    .bind(&patterns)
    .fetch_all(&mut *conn)
    .await?;
    let query = forms(&terms.join(" "));
    let mut hits: Vec<(u8, SuggestedCollection)> = rows
        .iter()
        .filter_map(|r| {
            let name: String = r.get("name");
            let tier = name_tier(&forms(&name), &query, &terms)?;
            let contract: String = r.get("contract_address");
            let creator: String = r.get("creator");
            Some((
                tier,
                SuggestedCollection {
                    sales: sales.get(&contract).copied().unwrap_or(0),
                    creator_name: directory
                        .iter()
                        .find(|c| c.address == creator)
                        .and_then(CreatorEntry::display_name),
                    contract_address: contract,
                    name,
                    creator,
                    items: r.get("items"),
                },
            ))
        })
        .collect();
    hits.sort_by(|(ta, a), (tb, b)| {
        tb.cmp(ta)
            .then(b.items.cmp(&a.items))
            .then(b.sales.cmp(&a.sales))
            .then(a.name.cmp(&b.name))
            .then(a.contract_address.cmp(&b.contract_address))
    });
    Ok(hits.into_iter().take(first).map(|(_, c)| c).collect())
}

/// Sales per collection in the window, from the sales signal on `read`;
/// an unavailable signal costs the tiebreak, not the answer. It reads under
/// a savepoint, so a failed read leaves the snapshot usable.
async fn collection_sales(
    state: &MarketplaceState,
    read: &mut CatalogueRead,
) -> HashMap<String, i64> {
    let mut out = HashMap::new();
    let Ok(mut savepoint) = sqlx::Connection::begin(&mut *read.tx).await else {
        return out;
    };
    match super::trending_sales::recent(state, &mut savepoint, COLLECTION_SALES_WINDOW_DAYS).await {
        Ok(sales) => {
            for sale in sales {
                *out.entry(sale.contract_address).or_default() += 1;
            }
            let _ = savepoint.commit().await;
        }
        Err(_) => {
            let _ = savepoint.rollback().await;
        }
    }
    out
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestedCatalogItem {
    #[serde(flatten)]
    pub item: CreditCatalogItem,
    pub creator_name: Option<String>,
}

/// `GET /v3/catalog/suggest?search=&items=&collections=&creators=`: the
/// items the grid shows for the query (its first rows and its total, so "See
/// all (N)" is that number), the matching collections and creators, run in
/// parallel over one creator directory.
pub async fn get_search_suggest(
    State(state): State<SharedState>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let p = Params::new(&pairs);
    let search = p.get_string("search", None).unwrap_or_default();
    let items_first = clamp(
        p.get_number("items", None),
        SUGGEST_ITEMS_DEFAULT_LIMIT,
        SUGGEST_SECTION_MAX_LIMIT,
    );
    let collections_first = clamp(
        p.get_number("collections", None),
        COLLECTION_SUGGEST_DEFAULT_LIMIT,
        SUGGEST_SECTION_MAX_LIMIT,
    );
    let creators_first = clamp(
        p.get_number("creators", None),
        CREATOR_SEARCH_DEFAULT_LIMIT,
        SUGGEST_SECTION_MAX_LIMIT,
    );
    let empty = json!({ "items": { "data": [], "total": 0 }, "collections": { "data": [] }, "creators": { "data": [] } });
    if query_terms(&search).is_empty() {
        return ([(CACHE_CONTROL, SUGGEST_CACHE)], Json(empty)).into_response();
    }
    // Pool work first: the read below holds its connection.
    let rate = match items::mana_usd_rate(&state).await {
        Ok(rate) => rate,
        Err(envelope) => return envelope,
    };
    // One snapshot answers the items and the collections (the items carry
    // prices); it is opened again, so its readiness is checked again. The
    // creator directory (a display aggregate of stated staleness, filed under
    // the sources that serve each network) holds the read only in the
    // request that computes it: a request that waited on another's opens a
    // new read from the same sources.
    let (directory, mut read) = loop {
        let read = match items::priced_preflight(&state, None).await {
            Ok(read) => read,
            Err(envelope) => return envelope,
        };
        let sources = read.sources();
        let mut held = Some(read);
        let directory = match creator_directory(&state, &mut held).await {
            Ok(Some(directory)) => directory,
            Ok(None) => continue,
            Err(e) => return e.into_response(),
        };
        let read = match held {
            Some(read) => read,
            None => match items::priced_preflight(&state, None).await {
                Ok(read) if read.sources() == sources => read,
                Ok(_) => continue,
                Err(envelope) => return envelope,
            },
        };
        break (directory, read);
    };
    read.note(&state);
    let sales = collection_sales(&state, &mut read).await;
    let item_pairs = vec![
        ("search".to_string(), search.clone()),
        ("first".to_string(), items_first.to_string()),
    ];
    let items_page = items::catalog_items_page(&state, &mut read, &item_pairs, rate).await;
    let collections =
        search_collections(&mut read.tx, &search, collections_first, &directory, &sales).await;
    let (item_rows, total) = match items_page {
        Ok(page) => page,
        Err(r) => return r,
    };
    let collections = match collections {
        Ok(c) => c,
        Err(e) => return e.into_response(),
    };
    let names: HashMap<&str, Option<String>> = directory
        .iter()
        .map(|c| (c.address.as_str(), c.display_name()))
        .collect();
    let items: Vec<SuggestedCatalogItem> = item_rows
        .into_iter()
        .map(|item| {
            let creator_name = names
                .get(item.item.creator.to_lowercase().as_str())
                .cloned()
                .flatten();
            SuggestedCatalogItem { item, creator_name }
        })
        .collect();
    let body = json!({
        "items": { "data": items, "total": total },
        "collections": { "data": collections },
        "creators": { "data": rank_creators(&directory, &search, creators_first) },
    });
    ([(CACHE_CONTROL, SUGGEST_CACHE)], Json(body)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn creator(address: &str, name: Option<&str>, names: &[&str], items: i64) -> CreatorEntry {
        CreatorEntry {
            address: address.into(),
            entity_id: None,
            name: name.map(str::to_string),
            face: None,
            claimed: false,
            names: names.iter().map(|n| n.to_string()).collect(),
            items,
            collections: 1,
        }
    }

    /// A feed never waits long on the creator directory. With none read yet and that read held up
    /// (the items table locked), it answers without profiles within `DIRECTORY_WAIT` while the
    /// read carries on and fills the directory; once expired, the old copy answers at once while
    /// one background read makes it fresh again.
    #[tokio::test]
    async fn a_feed_never_waits_long_on_the_creator_directory() {
        let Some(pool) = crate::db::scratch::isolated_pool(4).await else {
            return;
        };
        let state =
            crate::market::tests::support::state_with_empty_readers(Some(pool.clone())).await;
        // The background read passes the catalogue gate: the mirrors' passes.
        sqlx::query(
            "INSERT INTO shop.mirror_state (mirror, network, source_url, last_complete_pass_at) \
             SELECT m, n, 'test', timezone('UTC', now()) \
             FROM unnest(ARRAY['collections', 'items']) m CROSS JOIN unnest(ARRAY['ethereum', 'matic']) n",
        )
        .execute(&pool)
        .await
        .unwrap();
        let until_fresh = |state: SharedState| async move {
            tokio::time::timeout(Duration::from_secs(20), async {
                while !state.readers.holds(Ttl::M15, &directory_key("mirror")) {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
            })
            .await
            .expect("the background read fills the directory");
        };

        let mut lock = pool.begin().await.unwrap();
        sqlx::query("LOCK TABLE shop.items IN ACCESS EXCLUSIVE MODE")
            .execute(&mut *lock)
            .await
            .unwrap();
        let asked = std::time::Instant::now();
        let held = tokio::time::timeout(
            Duration::from_secs(10),
            directory_snapshot(&state, "mirror"),
        )
        .await
        .expect("the feed is not held up by the directory's read");
        assert!(held.is_none());
        assert!(
            asked.elapsed() < DIRECTORY_WAIT + Duration::from_secs(5),
            "{:?}",
            asked.elapsed()
        );
        // Another feed while that read is still held up does not lead a read of its own: it
        // answers without profiles at once.
        let follower = std::time::Instant::now();
        assert!(directory_snapshot(&state, "mirror").await.is_none());
        assert!(
            follower.elapsed() < DIRECTORY_WAIT,
            "{:?}",
            follower.elapsed()
        );
        assert!(!state.readers.holds(Ttl::M15, &directory_key("mirror")));
        lock.rollback().await.unwrap();
        until_fresh(state.clone()).await;
        let read = directory_snapshot(&state, "mirror")
            .await
            .expect("the fresh directory");

        state.readers.forget(Ttl::M15, &directory_key("mirror"));
        assert!(!state.readers.holds(Ttl::M15, &directory_key("mirror")));
        let mut lock = pool.begin().await.unwrap();
        sqlx::query("LOCK TABLE shop.items IN ACCESS EXCLUSIVE MODE")
            .execute(&mut *lock)
            .await
            .unwrap();
        assert_eq!(
            directory_snapshot(&state, "mirror").await,
            Some(read),
            "the expired copy answers while its refresh is held up"
        );
        lock.rollback().await.unwrap();
        until_fresh(state.clone()).await;

        // The kept copy serves for an hour from its read, and no longer: past it, with the cache
        // entry expired and the refresh held up, a feed answers without profiles.
        state.readers.forget(Ttl::M15, &directory_key("mirror"));
        state.readers.backdate_kept(
            &directory_key("mirror"),
            DIRECTORY_STALE_FOR + Duration::from_secs(1),
        );
        let mut lock = pool.begin().await.unwrap();
        sqlx::query("LOCK TABLE shop.items IN ACCESS EXCLUSIVE MODE")
            .execute(&mut *lock)
            .await
            .unwrap();
        let held = tokio::time::timeout(
            Duration::from_secs(10),
            directory_snapshot(&state, "mirror"),
        )
        .await
        .expect("the feed is not held up by the directory's read");
        assert!(held.is_none(), "a copy older than the hour is not served");
        lock.rollback().await.unwrap();
        until_fresh(state.clone()).await;
    }

    /// A feed-started directory read that hangs ends as failed at its limit and releases its claim
    /// to the backoff: a stuck read never holds the refresh until a restart.
    #[tokio::test]
    async fn a_hung_directory_read_releases_its_claim() {
        let Some(pool) = crate::db::scratch::isolated_pool(4).await else {
            return;
        };
        let state =
            crate::market::tests::support::state_with_empty_readers(Some(pool.clone())).await;
        // The background read passes the catalogue gate: the mirrors' passes.
        sqlx::query(
            "INSERT INTO shop.mirror_state (mirror, network, source_url, last_complete_pass_at) \
             SELECT m, n, 'test', timezone('UTC', now()) \
             FROM unnest(ARRAY['collections', 'items']) m CROSS JOIN unnest(ARRAY['ethereum', 'matic']) n",
        )
        .execute(&pool)
        .await
        .unwrap();
        let mut lock = pool.begin().await.unwrap();
        sqlx::query("LOCK TABLE shop.items IN ACCESS EXCLUSIVE MODE")
            .execute(&mut *lock)
            .await
            .unwrap();
        let started = std::time::Instant::now();
        assert!(directory_snapshot(&state, "mirror").await.is_none());
        // With the table still locked the read cannot succeed, so a claim this test can take
        // means the read ended as failed and released it.
        let claim = tokio::time::timeout(Duration::from_secs(20), async {
            loop {
                if let Some(claim) = state
                    .readers
                    .begin_refresh(&directory_key("mirror"), Duration::ZERO)
                {
                    break claim;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await
        .expect("the hung read releases its claim");
        assert!(
            started.elapsed() >= DIRECTORY_READ_LIMIT,
            "{:?}",
            started.elapsed()
        );
        state
            .readers
            .end_refresh(&directory_key("mirror"), claim, true);
        lock.rollback().await.unwrap();
    }

    #[test]
    fn a_feed_takes_the_profiles_of_its_creators_only() {
        let mut quack = creator("0xaa", Some("Quack"), &["quack"], 3);
        quack.entity_id = Some("bafyquack".into());
        quack.face = Some("https://cdn.example/entities/bafy/face.png".into());
        quack.claimed = true;
        let mut unnamed = creator("0xee", None, &[], 1);
        unnamed.entity_id = Some("bafyunnamed".into());
        let directory = serde_json::to_value(vec![
            quack,
            creator("0xbb", None, &[], 1),
            creator("0xdd", Some("Dee"), &[], 1),
            unnamed,
        ])
        .unwrap();
        let wanted: HashSet<String> = ["0xaa", "0xbb", "0xcc", "0xee"].map(String::from).into();
        let profiles = creator_profiles(&directory, &wanted, &Denylist::default());
        assert_eq!(profiles.len(), 2, "{profiles:?}");
        assert_eq!(
            profiles["0xaa"],
            CreatorProfile {
                name: "Quack".into(),
                face: Some("https://cdn.example/entities/bafy/face.png".into()),
                claimed: true
            }
        );
        assert!(
            !profiles.contains_key("0xbb"),
            "a creator with no profile carries none, so the card asks for it"
        );
        assert_eq!(
            profiles["0xee"],
            CreatorProfile {
                name: String::new(),
                face: None,
                claimed: false
            },
            "a profile with no name is still a profile"
        );
    }

    #[test]
    fn the_query_is_normalized_term_by_term() {
        assert_eq!(query_terms("Atari x RTFKT"), vec!["atari", "rtfkt"]);
        assert_eq!(
            query_terms("x"),
            vec!["x"],
            "a stopword alone is still a query"
        );
        assert_eq!(
            query_terms("T-Shirt t-shirt  o'brien"),
            vec!["tshirt", "obrien"]
        );
        assert_eq!(query_terms("a b c d e f g h").len(), MAX_TERMS);
        assert!(query_terms(" -- ").is_empty());
    }

    #[test]
    fn a_name_is_ranked_exact_then_prefix_then_partial() {
        let terms = query_terms("hat");
        let q = forms(&terms.join(" "));
        assert_eq!(name_tier(&forms("HAT"), &q, &terms), Some(2));
        assert_eq!(name_tier(&forms("Hat Pirate"), &q, &terms), Some(1));
        assert_eq!(
            name_tier(&forms("Hatch"), &q, &terms),
            Some(1),
            "a dropdown answers while the name is being typed"
        );
        assert_eq!(name_tier(&forms("Pirate Hat"), &q, &terms), Some(0));
        assert_eq!(name_tier(&forms("Treasure"), &q, &terms), None);
        let terms = query_terms("pirate hat");
        let q = forms(&terms.join(" "));
        assert_eq!(
            name_tier(&forms("Hat, Pirate"), &q, &terms),
            Some(2),
            "its words in any order"
        );
        assert_eq!(
            name_tier(&forms("Pirate Captain"), &q, &terms),
            None,
            "every term must match"
        );
        let terms = query_terms("star ducks");
        assert_eq!(
            name_tier(&forms("STARDUCKS"), &forms(&terms.join(" ")), &terms),
            Some(2),
            "a NAME is the words run together"
        );
    }

    #[test]
    fn creators_match_by_profile_name_or_name_and_rank_by_tier_then_catalogue() {
        let directory = vec![
            creator("0x01", Some("Duck Maker"), &[], 50),
            creator("0x02", None, &["starducks"], 5),
            creator("0x03", Some("Ducks"), &["quack"], 1),
            creator("0x04", Some("Other"), &["duckling"], 80),
            creator("0x05", Some("Nobody"), &[], 99),
        ];
        let hits = rank_creators(&directory, "ducks", 10);
        let order: Vec<&str> = hits.iter().map(|h| h.address.as_str()).collect();
        assert_eq!(
            order,
            vec!["0x03", "0x02"],
            "exact over partial; 'Duck Maker' and 'duckling' lack the term: {hits:?}"
        );
        assert_eq!(hits[1].name, "starducks", "no profile name: the first NAME");
        let hits = rank_creators(&directory, "duck", 10);
        assert_eq!(
            hits.iter().map(|h| h.address.as_str()).collect::<Vec<_>>(),
            vec!["0x04", "0x01", "0x03", "0x02"],
            "{hits:?}"
        );
        assert_eq!(rank_creators(&directory, "duck", 2).len(), 2);
    }

    #[tokio::test]
    async fn an_empty_query_suggests_nothing_without_a_lookup() {
        use tower::ServiceExt;
        let state = crate::market::tests::support::state_without_db_settings(
            crate::market::Settings::for_tests(),
        );
        let app = crate::market::market_router(state);
        let req = axum::http::Request::builder()
            .uri("/v3/catalog/suggest?search=%20--%20")
            .body(axum::body::Body::empty())
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), axum::http::StatusCode::OK);
        assert_eq!(res.headers()[CACHE_CONTROL], SUGGEST_CACHE);
        let body: Value = serde_json::from_slice(
            &axum::body::to_bytes(res.into_body(), 1 << 16)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            body,
            json!({ "items": { "data": [], "total": 0 }, "collections": { "data": [] }, "creators": { "data": [] } })
        );
    }

    #[tokio::test]
    async fn the_directory_reads_the_active_profile_and_the_catalogue_counts() {
        use crate::market::tests::support::state_with_db;
        let Some(state) = state_with_db(crate::market::Settings::for_tests()).await else {
            return;
        };
        let pool = state.pool.clone();
        let tag = uuid::Uuid::new_v4().simple().to_string();
        let creator = format!("0x{tag}00000000");
        let contract = format!("0x{tag}eeeeeeee");
        let entity = format!("bafy{tag}");
        sqlx::query(
            "INSERT INTO shop.items (network, contract_address, item_id, chain_id, item_type, urn, creator, rarity, total_supply, max_supply, available, price, \
                name, category, search_is_collection_approved, created_at, updated_at, reviewed_at, mirrored_at) \
             VALUES ('matic', $1, 0, 137, 'wearable_v2', $3, $2, 'common', 0, 10, 10, 0, 'Quackers Hat', 'hat', true, 1, 1, 1, timezone('UTC', now()))",
        )
        .bind(&contract)
        .bind(&creator)
        .bind(format!("urn:decentraland:matic:collections-v2:{contract}:0"))
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO content.deployments (deployer_address, version, entity_type, entity_id, entity_metadata, entity_timestamp, entity_pointers, local_timestamp, auth_chain) \
             VALUES ($1, 'v3', 'profile', $2, $3::json, now(), ARRAY[$1], now(), '[]'::json)",
        )
        .bind(&creator)
        .bind(&entity)
        .bind(json!({ "v": { "avatars": [{ "name": "Quackmaster", "avatar": { "snapshots": { "face256": "bafyface" } } }] } }).to_string())
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO content.active_pointers (pointer, entity_id, entity_type) VALUES ($1, $2, 'profile')").bind(&creator).bind(&entity).execute(&pool).await.unwrap();
        let names = format!("0x{tag}dddddddd");
        sqlx::query(
            "INSERT INTO shop.nft (network, contract_address, token_id, owner, ens_subdomain, minted_block, minted_at, transferred_block, transferred_log, transferred_at) \
             VALUES ('ethereum', $1, 1, $2, 'Quackmaster', 1, 1, 1, 0, 1)",
        )
        .bind(&names)
        .bind(&creator)
        .execute(&pool)
        .await
        .unwrap();
        let directory = creator_directory(
            &state,
            &mut Some(crate::market::catalogue::begin_unchecked(&state).await),
        )
        .await
        .unwrap()
        .expect("this request computed the directory");
        let entry = directory
            .iter()
            .find(|c| c.address == creator)
            .expect("the creator is in the directory");
        assert_eq!(
            (
                entry.name.as_deref(),
                entry.items,
                entry.collections,
                entry.claimed
            ),
            (Some("Quackmaster"), 1, 1, true)
        );
        let snapshot = directory_snapshot(&state, "mirror").await.unwrap();
        let wanted = HashSet::from([creator.clone()]);
        let profiles = creator_profiles(&snapshot, &wanted, &state.denylist);
        assert_eq!(profiles[&creator].name, "Quackmaster");
        assert!(profiles[&creator].claimed);
        assert!(
            entry
                .face
                .as_deref()
                .is_some_and(|f| f.ends_with(&format!("profile-images/entities/{entity}/face.png"))),
            "{entry:?}"
        );
        let hits = rank_creators(&directory, "quackmas", 4);
        assert_eq!(
            hits.first().map(|h| h.address.as_str()),
            Some(creator.as_str())
        );
        // A denylisted profile entity leaves the cached directory's reads at
        // once, as /lambdas/profiles leaves it out; the NAMEs stay.
        let listed = "https://registry.example/denylist";
        state.denylist.set_remote(listed, [entity.clone()]);
        let directory = creator_directory(
            &state,
            &mut Some(crate::market::catalogue::begin_unchecked(&state).await),
        )
        .await
        .unwrap()
        .expect("this request computed the directory");
        let entry = directory.iter().find(|c| c.address == creator).unwrap();
        assert_eq!(
            (entry.name.as_deref(), entry.face.as_deref(), entry.claimed),
            (None, None, false)
        );
        assert_eq!(entry.display_name().as_deref(), Some("Quackmaster"));
        let profiles = creator_profiles(&snapshot, &wanted, &state.denylist);
        assert_eq!(
            profiles[&creator],
            CreatorProfile {
                name: String::new(),
                face: None,
                claimed: false
            }
        );
        state.denylist.set_remote(listed, Vec::<String>::new());
        sqlx::query("DELETE FROM shop.nft WHERE contract_address = $1")
            .bind(&names)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM content.active_pointers WHERE pointer = $1")
            .bind(&creator)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM content.deployments WHERE entity_id = $1")
            .bind(&entity)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM shop.items WHERE contract_address = $1")
            .bind(&contract)
            .execute(&pool)
            .await
            .unwrap();
    }
}
