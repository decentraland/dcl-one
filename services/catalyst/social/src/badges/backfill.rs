//! `POST /badges/{id}/backfill`: upstream's bulk import of progress computed
//! elsewhere (`badges-backfill-handler.ts`, `backfill-merger.ts`,
//! `backfills/*`), the only grant path of `event_enthusiast` and of the
//! one-time event badges. Bearer-gated like grant and revoke.
//!
//! The request is `{"registries": [{"userAddress", "data": {"progress": ...}}]}`
//! and the answer `{"data": {"badge", "userProgressesMerged", "failures"}}`.
//! Upstream's checks, in its order: an id outside upstream's `BadgeId` enum
//! is 404, an unknown or unpublished badge 404, an archived one 400; a body
//! that is not JSON, or any registry a merger refuses, fails the whole
//! request with 400 before anything is written. Each registry is then merged
//! with the wallet's stored progress by the badge's merger and checked
//! (`validateUserProgress`); a check that fails, or a badge archived since,
//! is a `failures` entry, the rest are written and counted. A database error
//! on one wallet is a `failures` entry too (logged, generic message), so the
//! wallets written before it are still reported.
//!
//! Each wallet is one transaction: the badge row `FOR SHARE`, the progress
//! advisory lock of [`apply_progress`], the stored progress read under it,
//! the merge, then [`apply_progress`] (steps, document, every reached tier,
//! completion; never a notification) and the merger's own instants on the
//! tiers and the completion. Unlike upstream, the merge reads the progress
//! under the lock, earned tiers and completion are never taken back, a
//! reached tier or target the merger left unstamped is stamped now, and a
//! wallet listed twice is merged twice in order. The gateway caps request
//! bodies at 1 MB, so callers send their registries in batches.

use std::collections::{HashMap, HashSet};

use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, NaiveDateTime};
use serde::Serialize;
use serde_json::{Map, Value, json};
use sqlx::PgConnection;

use super::progress::{Applied, ApplyOptions, apply_progress};
use super::types::{CatalogBadge, Data};
use crate::identity::Address;
use crate::notifications::admin::RequireAdmin;
use crate::notifications::errors::ApiError;

type AppState = std::sync::Arc<super::State>;

/// Which of upstream's mergers a badge id takes (`backfill-merger.ts`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    WearablesEquipment,
    Emotionista,
    EmoteCreator,
    Fashionista,
    WearableDesigner,
    EventEnthusiast,
    MovesMaster,
    SocialButterfly,
    LandArchitect,
    ProfilePro,
    OpenForBusiness,
    OneTime,
    /// In upstream's enum without a merger: every registry is refused.
    Unsupported,
}

/// `parseBadgeId` then the merger switch: `None` is outside the enum (404).
pub fn kind(badge_id: &str) -> Option<Kind> {
    Some(match badge_id {
        "regally_rare" | "exotic_elegance" | "epic_ensemble" | "unique_unicorn"
        | "legendary_look" | "mythic_model" => Kind::WearablesEquipment,
        "emotionista" => Kind::Emotionista,
        "emote_creator" => Kind::EmoteCreator,
        "fashionista" => Kind::Fashionista,
        "wearable_designer" => Kind::WearableDesigner,
        "event_enthusiast" => Kind::EventEnthusiast,
        "moves_master" => Kind::MovesMaster,
        "social_butterfly" => Kind::SocialButterfly,
        "land_architect" => Kind::LandArchitect,
        "profile_pro" => Kind::ProfilePro,
        "open_for_business" => Kind::OpenForBusiness,
        "music_festival_2024"
        | "fashion_week_2025"
        | "builder_talent_trail_2025"
        | "music_talent_trail_2025"
        | "game_night_2025"
        | "career_quest_2025"
        | "art_week_2025"
        | "fright_night_2025"
        | "music_festival_2025"
        | "festive_trail_2025"
        | "welcome_guide"
        | "buildathon_friendzone"
        | "halloweeks_2026" => Kind::OneTime,
        "dclcitizen" | "traveler" | "vertical_voyager" | "walkabout_wanderer" | "referral" => {
            Kind::Unsupported
        }
        _ => return None,
    })
}

/// The badge's goals as the mergers read them.
#[derive(Debug, Clone, PartialEq)]
pub struct Def {
    pub is_tier: bool,
    /// A single badge's `criteria.steps`.
    pub target: i64,
    /// A tiered badge's tiers in order: (tier id, steps).
    pub tiers: Vec<(String, i64)>,
}

impl Def {
    fn from_catalog(badge: &CatalogBadge) -> Self {
        Self {
            is_tier: badge.tiers.is_some(),
            target: badge.criteria.as_ref().map_or(1, |c| c.steps as i64),
            tiers: badge
                .tiers
                .iter()
                .flatten()
                .map(|t| (t.tier_id.clone(), t.criteria.steps as i64))
                .collect(),
        }
    }
}

/// Upstream's `UserBadge` as a merger sees it: instants in epoch ms.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct UserState {
    pub steps: i64,
    /// The progress document without `steps`.
    pub doc: Map<String, Value>,
    /// `achieved_tiers`, in their order.
    pub tiers: Vec<(String, i64)>,
    pub completed_at: Option<i64>,
}

/// One registry's `data.progress`, checked as its merger's validator does.
#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    Equipment {
        completed_with: Vec<Value>,
        completed_at: i64,
    },
    /// (id, instant) items of the purchase and publication badges.
    Items(Vec<(String, i64)>),
    /// (event id, first visit) of `eventsAttended`.
    Events(Vec<(String, i64)>),
    Moves {
        used: i64,
        last_triggered_at: i64,
        /// (tier steps, completed at)
        tiers: Vec<(i64, i64)>,
    },
    /// (profile address, visited at)
    Visits(Vec<(String, i64)>),
    LandArchitect(i64),
    ProfilePro {
        completed_at: i64,
        description: String,
    },
    OpenForBusiness {
        completed_at: Option<i64>,
        collection_created_at: Option<i64>,
        store_created_at: Option<i64>,
    },
    OneTime(i64),
}

/// `Number.isInteger`: an integral JSON number (`1.0` included).
fn int(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n.as_i64().or_else(|| {
            n.as_f64()
                .filter(|f| f.fract() == 0.0 && f.abs() < 9.0e15)
                .map(|f| f as i64)
        }),
        _ => None,
    }
}

fn str_of(v: &Value) -> Option<String> {
    v.as_str().map(str::to_string)
}

/// `[{id_key: string, ts_key: integer}]`, or `None`.
fn pairs(v: &Value, id_key: &str, ts_key: &str) -> Option<Vec<(String, i64)>> {
    v.as_array()?
        .iter()
        .map(|item| Some((str_of(item.get(id_key)?)?, int(item.get(ts_key)?)?)))
        .collect()
}

/// The upstream item keys of the purchase and publication badges:
/// (request array, stored document key, id key, instant key).
fn item_keys(kind: Kind) -> Option<(&'static str, &'static str, &'static str, &'static str)> {
    Some(match kind {
        Kind::Emotionista => (
            "emotesBought",
            "transactions_emotes_purchase",
            "transactionHash",
            "saleAt",
        ),
        Kind::Fashionista => (
            "wearablesBought",
            "transactions_wearable_purchase",
            "transactionHash",
            "saleAt",
        ),
        Kind::EmoteCreator => ("emotesPublished", "published_emotes", "itemId", "createdAt"),
        Kind::WearableDesigner => (
            "wearablesPublished",
            "published_wearables",
            "itemId",
            "createdAt",
        ),
        _ => return None,
    })
}

/// A registry's `data` as its merger accepts it; `None` is upstream's
/// "Could not backfill this badge". `def` is the badge as the request read it.
pub fn parse(kind: Kind, def: &Def, data: &Value) -> Option<Input> {
    let progress = data.get("progress")?;
    let field = |k: &str| progress.get(k);
    let tiered = || def.is_tier && !def.tiers.is_empty();
    match kind {
        Kind::WearablesEquipment => {
            let completed_with = field("completedWith")?.as_array()?;
            let completed_at = int(field("completedAt")?).filter(|t| *t != 0)?;
            completed_with
                .iter()
                .all(Value::is_string)
                .then(|| Input::Equipment {
                    completed_with: completed_with.clone(),
                    completed_at,
                })
        }
        Kind::Emotionista | Kind::Fashionista | Kind::EmoteCreator | Kind::WearableDesigner => {
            let (array, _, id_key, ts_key) = item_keys(kind)?;
            if !tiered() {
                return None;
            }
            pairs(field(array)?, id_key, ts_key).map(Input::Items)
        }
        Kind::EventEnthusiast => {
            if !tiered() {
                return None;
            }
            let events = field("eventsAttended")?.as_array()?;
            events
                .iter()
                .map(|e| {
                    str_of(e.get("eventName")?)?;
                    int(e.get("timeSpentInSeconds")?)?;
                    Some((str_of(e.get("eventId")?)?, int(e.get("firstVisitAt")?)?))
                })
                .collect::<Option<Vec<_>>>()
                .map(Input::Events)
        }
        Kind::MovesMaster => {
            let used = int(field("usedEmotesCount")?)?;
            let last_triggered_at = int(field("lastEmoteTriggeredAt")?).filter(|t| *t > 0)?;
            let tiers = field("achievedTiers")?
                .as_array()?
                .iter()
                .map(|t| Some((int(t.get("steps")?)?, int(t.get("completedAt")?)?)))
                .collect::<Option<Vec<_>>>()?;
            let known = tiers
                .iter()
                .all(|(steps, _)| def.tiers.iter().any(|(_, s)| s == steps));
            (tiered() && known && (0..=i32::MAX as i64).contains(&used)).then_some(Input::Moves {
                used,
                last_triggered_at,
                tiers,
            })
        }
        Kind::SocialButterfly => {
            if !tiered() {
                return None;
            }
            pairs(field("profiles_visited")?, "profile_address", "visited_at").map(Input::Visits)
        }
        Kind::LandArchitect => int(field("firstSceneDeployedAt")?).map(Input::LandArchitect),
        Kind::ProfilePro => Some(Input::ProfilePro {
            completed_at: int(field("completedAt")?)?,
            description: str_of(field("description")?)?,
        }),
        Kind::OpenForBusiness => {
            // absent is fine; present must be an integer (null is not)
            let optional = |k: &str| match field(k) {
                None => Some(None),
                Some(v) => int(v).map(Some),
            };
            Some(Input::OpenForBusiness {
                completed_at: optional("completedAt")?,
                collection_created_at: optional("collectionCreatedAt")?,
                store_created_at: optional("storeCreatedAt")?,
            })
        }
        Kind::OneTime => {
            let completed_at = int(field("completedAt")?).filter(|t| *t != 0)?;
            (!def.is_tier).then_some(Input::OneTime(completed_at))
        }
        Kind::Unsupported => None,
    }
}

/// JavaScript truthiness of an optional instant.
fn truthy(v: Option<i64>) -> Option<i64> {
    v.filter(|t| *t != 0)
}

/// `getUniqueSortedItems`: the last instant given per id, in first-seen
/// order, then stably sorted by instant.
fn unique_sorted(items: impl IntoIterator<Item = (String, i64)>) -> Vec<(String, i64)> {
    let mut out: Vec<(String, i64)> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    for (id, ts) in items {
        match index.get(&id) {
            Some(&i) => out[i].1 = ts,
            None => {
                index.insert(id.clone(), out.len());
                out.push((id, ts));
            }
        }
    }
    out.sort_by_key(|(_, ts)| *ts);
    out
}

fn stamp_of(tiers: &[(String, i64)], tier_id: &str) -> Option<i64> {
    tiers
        .iter()
        .find(|(id, _)| id == tier_id)
        .map(|(_, at)| *at)
}

/// The stored items of a purchase or publication badge (malformed entries
/// left out; a missing list is empty).
fn stored_pairs(
    doc: &Map<String, Value>,
    key: &str,
    id_key: &str,
    ts_key: &str,
) -> Vec<(String, i64)> {
    doc.get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| Some((str_of(item.get(id_key)?)?, int(item.get(ts_key)?)?)))
        .collect()
}

/// Merges `input` into the stored progress (`None`: the wallet has none) as
/// upstream's merger for `kind` does; `now` is `Date.now()`.
pub fn merge(
    kind: Kind,
    def: &Def,
    current: Option<UserState>,
    input: &Input,
    now: i64,
) -> Result<UserState, String> {
    let fresh = current.is_none();
    let mut user = current.unwrap_or_default();
    match (kind, input) {
        (
            Kind::WearablesEquipment,
            Input::Equipment {
                completed_with,
                completed_at,
            },
        ) => {
            if fresh {
                user.steps = 1;
            }
            // upstream compares against `progress.completedAt`, a key no
            // writer sets; the stored completion counts too, so the earliest wins
            let stored =
                truthy(user.completed_at).or(truthy(user.doc.get("completedAt").and_then(int)));
            if stored.is_none_or(|s| *completed_at < s) {
                user.completed_at = Some(*completed_at);
                user.doc.insert(
                    "completed_with".into(),
                    Value::Array(completed_with.clone()),
                );
            }
        }
        (
            Kind::Emotionista | Kind::Fashionista | Kind::EmoteCreator | Kind::WearableDesigner,
            Input::Items(items),
        ) => {
            let (_, key, id_key, ts_key) = item_keys(kind).ok_or("no item keys")?;
            let stored = stored_pairs(&user.doc, key, id_key, ts_key);
            let sorted = unique_sorted(stored.into_iter().chain(items.iter().cloned()));
            user.steps = sorted.len() as i64;
            let list = sorted
                .iter()
                .map(|(id, ts)| json!({ id_key: id, ts_key: ts }))
                .collect();
            user.doc = Map::from_iter([(key.to_string(), Value::Array(list))]);
            let reached: Vec<(String, i64)> = def
                .tiers
                .iter()
                .filter(|(_, steps)| *steps <= user.steps)
                .map(|(tier_id, steps)| {
                    let item = sorted
                        .get((*steps - 1) as usize)
                        .ok_or_else(|| format!("Could not find publication for tier {tier_id}"))?;
                    let had = truthy(stamp_of(&user.tiers, tier_id)).unwrap_or(now);
                    Ok((tier_id.clone(), had.min(item.1)))
                })
                .collect::<Result<_, String>>()?;
            if !reached.is_empty() {
                user.tiers = reached;
            }
            if !user.tiers.is_empty() && user.tiers.len() == def.tiers.len() {
                let (last_id, last_steps) = def.tiers.last().ok_or("no tiers")?;
                let item = sorted.get((*last_steps - 1) as usize).ok_or_else(|| {
                    format!("Could not find the item related to the last tier {last_id}")
                })?;
                let completed = [user.completed_at, Some(item.1), Some(now)]
                    .into_iter()
                    .flatten()
                    .min();
                if truthy(completed).is_some() {
                    user.completed_at = completed;
                }
            }
        }
        (Kind::EventEnthusiast, Input::Events(events)) => {
            let mut visited: Vec<String> = user
                .doc
                .get("events_ids_visited")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(str_of)
                .collect();
            for (id, _) in events {
                if !visited.contains(id) {
                    visited.push(id.clone());
                }
            }
            // a JavaScript Set keeps the first of equal ids, in insertion order
            let mut unique: Vec<String> = Vec::with_capacity(visited.len());
            for id in visited {
                if !unique.contains(&id) {
                    unique.push(id);
                }
            }
            user.steps = unique.len() as i64;
            user.doc = Map::from_iter([(
                "events_ids_visited".to_string(),
                Value::Array(unique.into_iter().map(Value::String).collect()),
            )]);
            let mut sorted = events.clone();
            sorted.sort_by_key(|(_, at)| *at);
            let reached: Vec<(String, i64)> = def
                .tiers
                .iter()
                .filter(|(_, steps)| *steps <= user.steps)
                .map(|(tier_id, steps)| {
                    let had = stamp_of(&user.tiers, tier_id);
                    let at = match sorted.get((*steps - 1) as usize) {
                        Some((_, visit)) => match had {
                            Some(h) if h < *visit => h,
                            _ => *visit,
                        },
                        None => had.unwrap_or(now),
                    };
                    (tier_id.clone(), at)
                })
                .collect();
            if !reached.is_empty() {
                user.tiers = reached;
            }
            if !user.tiers.is_empty() && user.tiers.len() == def.tiers.len() {
                let (_, last_steps) = def.tiers.last().ok_or("no tiers")?;
                match sorted.get((*last_steps - 1) as usize) {
                    Some((_, visit)) if truthy(user.completed_at).is_none_or(|c| *visit < c) => {
                        user.completed_at = Some(*visit)
                    }
                    None if truthy(user.completed_at).is_none() => user.completed_at = Some(now),
                    _ => {}
                }
            }
        }
        (
            Kind::MovesMaster,
            Input::Moves {
                used,
                last_triggered_at,
                tiers,
            },
        ) => {
            // upstream rebuilds the row from the backfill alone
            user = UserState {
                steps: *used,
                doc: Map::from_iter([
                    (
                        "last_used_emote_timestamp".to_string(),
                        json!(last_triggered_at),
                    ),
                    ("last_day_used_emotes_timestamps".to_string(), json!([])),
                ]),
                tiers: Vec::new(),
                completed_at: None,
            };
            for (steps, at) in tiers {
                let (tier_id, _) = def
                    .tiers
                    .iter()
                    .find(|(_, s)| s == steps)
                    .ok_or("tierId received is invalid, breaking backfill")?;
                user.tiers.push((tier_id.clone(), *at));
            }
            if user.tiers.len() == def.tiers.len() {
                user.completed_at = Some(now);
            }
        }
        (Kind::SocialButterfly, Input::Visits(visits)) => {
            // upstream's merge keeps the backfill's visits only: the earliest
            // per profile, never later than now
            let mut first: Vec<(String, i64)> = Vec::new();
            for (profile, at) in visits {
                match first.iter_mut().find(|(p, _)| p == profile) {
                    Some(entry) => entry.1 = entry.1.min(*at),
                    None => first.push((profile.clone(), now.min(*at))),
                }
            }
            first.sort_by_key(|(_, at)| *at);
            user.steps = first.len() as i64;
            user.doc = Map::from_iter([(
                "profiles_visited".to_string(),
                Value::Array(
                    first
                        .iter()
                        .map(|(p, at)| json!({ "profile_address": p, "visited_at": at }))
                        .collect(),
                ),
            )]);
            let reached: Vec<(String, i64)> = def
                .tiers
                .iter()
                .filter(|(_, steps)| *steps <= user.steps)
                .map(|(tier_id, steps)| {
                    let at = first
                        .get((*steps - 1) as usize)
                        .and_then(|(_, at)| truthy(Some(*at)))
                        .unwrap_or(now);
                    let at = stamp_of(&user.tiers, tier_id).map_or(at, |had| had.min(at));
                    (tier_id.clone(), at)
                })
                .collect();
            if !reached.is_empty() {
                user.tiers = reached;
            }
            if !user.tiers.is_empty() && user.tiers.len() == def.tiers.len() {
                let (_, last_steps) = def.tiers.last().ok_or("no tiers")?;
                user.completed_at = Some(match first.get((*last_steps - 1) as usize) {
                    Some((_, at)) => truthy(user.completed_at).map_or(*at, |c| c.min(*at)),
                    None => now,
                });
            }
        }
        (Kind::LandArchitect, Input::LandArchitect(at)) => {
            user.steps = 1;
            user.completed_at = Some(truthy(user.completed_at).unwrap_or(now).min(*at));
        }
        (
            Kind::ProfilePro,
            Input::ProfilePro {
                completed_at,
                description,
            },
        ) => {
            user.steps = 1;
            user.doc.insert(
                "description_added".into(),
                Value::String(description.clone()),
            );
            user.completed_at = Some(truthy(user.completed_at).unwrap_or(now).min(*completed_at));
        }
        (
            Kind::OpenForBusiness,
            Input::OpenForBusiness {
                completed_at,
                collection_created_at,
                store_created_at,
            },
        ) => {
            if truthy(user.completed_at).is_none() {
                let collection = truthy(*collection_created_at).is_some();
                let store = truthy(*store_created_at).is_some();
                user.steps = collection as i64 + store as i64;
                user.doc
                    .insert("store_completed".into(), Value::Bool(store));
                user.doc
                    .insert("collection_submitted".into(), Value::Bool(collection));
            }
            if let Some(at) = truthy(*completed_at) {
                user.completed_at = Some(truthy(user.completed_at).unwrap_or(now).min(at));
            }
        }
        (Kind::OneTime, Input::OneTime(at)) => {
            user.steps = def.target;
            // the first time it was earned is kept; older rows carry it in the
            // document too, as a number or as text
            let as_ms = |v: &Value| {
                match v {
                    Value::String(s) if !s.trim().is_empty() => s.trim().parse::<f64>().ok(),
                    Value::Number(n) => n.as_f64(),
                    _ => None,
                }
                .filter(|f| f.is_finite())
                .map(|f| f as i64)
            };
            let earlier: Vec<i64> = user
                .completed_at
                .into_iter()
                .chain(user.doc.get("completed_at").and_then(as_ms))
                .collect();
            let first = if earlier.is_empty() {
                (*at).min(now)
            } else {
                earlier.into_iter().fold(*at, i64::min)
            };
            user.doc.insert("completed_at".into(), json!(first));
            user.completed_at = Some(first);
        }
        _ => return Err("Invalid Badge ID".into()),
    }
    Ok(user)
}

/// Upstream's `validateUserProgress`: the errors of a merged row.
pub fn validate(user: &UserState, def: &Def) -> Vec<String> {
    let mut errors = Vec::new();
    // distinct, so a repeated tier cannot stand in for a missing one
    let achieved = user
        .tiers
        .iter()
        .map(|(id, _)| id)
        .collect::<HashSet<_>>()
        .len();
    if def.is_tier && user.completed_at.is_some() && achieved != def.tiers.len() {
        errors.push("mismatch between badge tiers and achieved tiers".to_string());
    }
    if !def.is_tier && user.completed_at.is_some() && user.steps != def.target {
        errors.push("mismatch between badge criteria and progress steps".to_string());
    }
    if def.is_tier {
        for (i, (tier_id, at)) in user.tiers.iter().enumerate() {
            let Some((_, steps)) = def.tiers.iter().find(|(id, _)| id == tier_id) else {
                errors.push(format!("tier achieved {tier_id} not found in badge tiers"));
                continue;
            };
            if *steps > user.steps {
                errors.push(format!(
                    "tier achieved {tier_id} criteria steps are greater than progress steps"
                ));
            }
            if let Some((previous, previous_at)) = i.checked_sub(1).map(|p| &user.tiers[p])
                && previous_at > at
            {
                errors.push(format!(
                    "tier achieved {tier_id} completed before previous tier {previous}"
                ));
            }
        }
    }
    errors
}

/// The database is out of reach, not refusing this statement.
fn connection_lost(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::Io(_)
        | sqlx::Error::Tls(_)
        | sqlx::Error::Protocol(_)
        | sqlx::Error::PoolTimedOut
        | sqlx::Error::PoolClosed
        | sqlx::Error::WorkerCrashed => true,
        // 08 connection exception; 57P01-03 shutdown or not accepting
        sqlx::Error::Database(e) => e.code().is_some_and(|c| {
            c.starts_with("08") || matches!(c.as_ref(), "57P01" | "57P02" | "57P03")
        }),
        _ => false,
    }
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Failure {
    pub address: String,
    pub errors: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct BackfillResult {
    pub badge: Value,
    #[serde(rename = "userProgressesMerged")]
    pub user_progresses_merged: u64,
    pub failures: Vec<Failure>,
}

fn message(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "message": message }))).into_response()
}

fn naive(ms: i64) -> Option<NaiveDateTime> {
    DateTime::from_timestamp_millis(ms).map(|t| t.naive_utc())
}

enum Written {
    Merged,
    Failed(Vec<String>),
}

/// Merges and writes one wallet in its own transaction (see the module docs).
async fn backfill_one(
    conn: &mut PgConnection,
    address: &Address,
    badge_id: &str,
    kind: Kind,
    input: &Input,
    now: NaiveDateTime,
) -> Result<Written, sqlx::Error> {
    let not_awardable = || Written::Failed(vec!["The badge is not awardable".into()]);
    let badge: Option<(bool, Option<i32>)> = sqlx::query_as(
        "SELECT is_tier, criteria_steps FROM social.badge_definitions \
         WHERE id = $1 AND published_at IS NOT NULL AND archived_at IS NULL FOR SHARE",
    )
    .bind(badge_id)
    .fetch_optional(&mut *conn)
    .await?;
    let Some((is_tier, criteria_steps)) = badge else {
        return Ok(not_awardable());
    };
    let tiers: Vec<(String, i32)> = sqlx::query_as(
        "SELECT tier_id, criteria_steps FROM social.badge_tiers WHERE badge_id = $1 ORDER BY ordinal",
    )
    .bind(badge_id)
    .fetch_all(&mut *conn)
    .await?;
    let def = Def {
        is_tier,
        target: criteria_steps.unwrap_or(1) as i64,
        tiers: tiers.into_iter().map(|(id, s)| (id, s as i64)).collect(),
    };
    // apply_progress's lock, taken here so the merge reads what it will write over
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!("badge-progress:{}:{badge_id}", address.as_str()))
        .execute(&mut *conn)
        .await?;
    let stored: Option<(i32, Value, Option<NaiveDateTime>)> = sqlx::query_as(
        "SELECT steps_done, progress, completed_at FROM social.user_badge_progress \
         WHERE address = $1 AND badge_id = $2",
    )
    .bind(address)
    .bind(badge_id)
    .fetch_optional(&mut *conn)
    .await?;
    let current = match stored {
        None => None,
        Some((steps, document, completed_at)) => {
            let earned: Vec<(String, NaiveDateTime)> = sqlx::query_as(
                "SELECT a.tier_id, a.completed_at FROM social.user_achieved_tiers a \
                 LEFT JOIN social.badge_tiers t ON t.badge_id = a.badge_id AND t.tier_id = a.tier_id \
                 WHERE a.address = $1 AND a.badge_id = $2 ORDER BY a.completed_at, t.ordinal",
            )
            .bind(address)
            .bind(badge_id)
            .fetch_all(&mut *conn)
            .await?;
            let mut doc = match document {
                Value::Object(map) => map,
                _ => Map::new(),
            };
            doc.remove("steps");
            Some(UserState {
                steps: steps as i64,
                doc,
                tiers: earned
                    .into_iter()
                    .map(|(id, at)| (id, at.and_utc().timestamp_millis()))
                    .collect(),
                completed_at: completed_at.map(|t| t.and_utc().timestamp_millis()),
            })
        }
    };
    let merged = match merge(kind, &def, current, input, now.and_utc().timestamp_millis()) {
        Ok(merged) => merged,
        Err(error) => return Ok(Written::Failed(vec![error])),
    };
    let mut errors = validate(&merged, &def);
    let steps = i32::try_from(merged.steps).ok().filter(|s| *s >= 0);
    if steps.is_none() {
        errors.push("progress steps out of range".into());
    }
    let stamps: Option<Vec<(String, NaiveDateTime)>> = merged
        .tiers
        .iter()
        .map(|(id, ms)| Some((id.clone(), naive(*ms)?)))
        .collect();
    let completed_at = merged.completed_at.map(naive);
    if stamps.is_none() || completed_at.is_some_and(|c| c.is_none()) {
        errors.push("instant out of range".into());
    }
    if !errors.is_empty() {
        return Ok(Written::Failed(errors));
    }
    let options = ApplyOptions {
        at: now,
        notify: None,
        granted_by: None,
    };
    let doc = merged.doc;
    let applied = apply_progress(conn, address, badge_id, &options, |p| {
        p.steps = steps.unwrap_or_default();
        p.state = doc;
    })
    .await?;
    if applied == Applied::NotAwardable {
        return Ok(not_awardable());
    }
    for (tier_id, at) in stamps.unwrap_or_default() {
        sqlx::query(
            "INSERT INTO social.user_achieved_tiers (address, badge_id, tier_id, completed_at) \
             VALUES ($1, $2, $3, $4) \
             ON CONFLICT (address, badge_id, tier_id) DO UPDATE SET completed_at = EXCLUDED.completed_at",
        )
        .bind(address)
        .bind(badge_id)
        .bind(&tier_id)
        .bind(at)
        .execute(&mut *conn)
        .await?;
    }
    if let Some(Some(at)) = completed_at {
        sqlx::query(
            "UPDATE social.user_badge_progress SET completed_at = $3 \
             WHERE address = $1 AND badge_id = $2",
        )
        .bind(address)
        .bind(badge_id)
        .bind(at)
        .execute(&mut *conn)
        .await?;
    }
    Ok(Written::Merged)
}

/// `POST /badges/{id}/backfill`.
pub async fn backfill_badge(
    _admin: RequireAdmin,
    State(state): State<AppState>,
    Path(badge_id): Path<String>,
    body: Bytes,
) -> Result<Response, ApiError> {
    let Some(kind) = kind(&badge_id) else {
        return Ok(message(StatusCode::NOT_FOUND, "Badge does not exists"));
    };
    let Some(badge) = state.badges.badge(&badge_id).await? else {
        return Ok(message(StatusCode::NOT_FOUND, "Badge does not exists"));
    };
    if badge.archived {
        return Ok(message(
            StatusCode::BAD_REQUEST,
            "Badge is archived and cannot be granted",
        ));
    }
    let def = Def::from_catalog(&badge);
    let registries = serde_json::from_slice::<Value>(&body)
        .ok()
        .and_then(|b| b.get("registries")?.as_array().cloned())
        .and_then(|list| {
            list.into_iter()
                .map(|r| Some((str_of(r.get("userAddress")?)?, r.get("data").cloned())))
                .collect::<Option<Vec<_>>>()
        });
    let Some(registries) = registries else {
        return Ok(message(StatusCode::BAD_REQUEST, "Invalid body"));
    };
    // every registry is checked before anything is written, as upstream merges
    // them all before its first write
    let mut inputs = Vec::with_capacity(registries.len());
    for (address, data) in registries {
        let Some(input) = data.as_ref().and_then(|d| parse(kind, &def, d)) else {
            tracing::warn!(badge = %badge_id, address = %address, "badges: backfill registry refused");
            return Ok(message(
                StatusCode::BAD_REQUEST,
                "Could not backfill this badge",
            ));
        };
        inputs.push((address, input));
    }

    let mut merged = 0;
    let mut failures = Vec::new();
    let mut wallets = inputs.iter();
    while let Some((raw, input)) = wallets.next() {
        let Ok(address) = Address::parse(raw) else {
            failures.push(Failure {
                address: raw.clone(),
                errors: vec!["invalid address".into()],
            });
            continue;
        };
        let now = chrono::Utc::now().naive_utc();
        // a statement error fails this wallet only: earlier wallets are
        // committed and the response must still say which. A lost database
        // fails the rest at once instead of waiting out the pool per wallet.
        let written = async {
            let mut tx = state.badges.pool().begin().await?;
            let written = backfill_one(&mut tx, &address, &badge_id, kind, input, now).await?;
            match written {
                Written::Merged => tx.commit().await?,
                Written::Failed(_) => tx.rollback().await?,
            }
            Ok::<_, sqlx::Error>(written)
        }
        .await;
        let written = match written {
            Ok(written) => written,
            Err(error) => {
                tracing::error!(badge = %badge_id, address = %address, %error, "badges: backfill write failed");
                failures.push(Failure {
                    address: address.into_string(),
                    errors: vec!["could not write the progress".into()],
                });
                if connection_lost(&error) {
                    failures.extend(wallets.by_ref().map(|(raw, _)| Failure {
                        address:
                            Address::parse(raw).map_or_else(|_| raw.clone(), Address::into_string),
                        errors: vec!["not attempted: the database is unavailable".into()],
                    }));
                }
                continue;
            }
        };
        match written {
            Written::Merged => merged += 1,
            Written::Failed(errors) => failures.push(Failure {
                address: address.into_string(),
                errors,
            }),
        }
    }
    tracing::info!(badge = %badge_id, merged, failed = failures.len(), "badges: backfill");

    let mut badge = serde_json::to_value(&badge).unwrap_or_default();
    if let Value::Object(map) = &mut badge {
        map.remove("archived");
    }
    Ok(Json(Data::new(BackfillResult {
        badge,
        user_progresses_merged: merged,
        failures,
    }))
    .into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_800_000_000_000;

    fn tiered(steps: &[i64]) -> Def {
        Def {
            is_tier: true,
            target: 1,
            tiers: steps
                .iter()
                .enumerate()
                .map(|(i, s)| (format!("t{i}"), *s))
                .collect(),
        }
    }

    fn single(target: i64) -> Def {
        Def {
            is_tier: false,
            target,
            tiers: Vec::new(),
        }
    }

    fn input(kind: Kind, def: &Def, progress: Value) -> Input {
        parse(kind, def, &json!({ "progress": progress })).expect("accepted")
    }

    #[test]
    fn kinds_follow_upstreams_enum_and_switch() {
        assert_eq!(kind("halloweeks_2026"), Some(Kind::OneTime));
        assert_eq!(kind("mythic_model"), Some(Kind::WearablesEquipment));
        assert_eq!(kind("traveler"), Some(Kind::Unsupported));
        assert_eq!(kind("own_badge"), None);
    }

    #[test]
    fn the_validators_refuse_what_upstreams_refuse() {
        let t = tiered(&[1, 3]);
        let s = single(1);
        for (kind, def, data) in [
            (
                Kind::OneTime,
                &s,
                json!({ "progress": { "completedAt": 0 } }),
            ),
            (
                Kind::OneTime,
                &s,
                json!({ "progress": { "completedAt": 1.5 } }),
            ),
            (
                Kind::OneTime,
                &t,
                json!({ "progress": { "completedAt": 1 } }),
            ),
            (Kind::OneTime, &s, json!({})),
            (
                Kind::Emotionista,
                &t,
                json!({ "progress": { "emotesBought": [{ "transactionHash": 1, "saleAt": 1 }] } }),
            ),
            (
                Kind::Emotionista,
                &s,
                json!({ "progress": { "emotesBought": [] } }),
            ),
            (
                Kind::MovesMaster,
                &t,
                json!({ "progress": { "usedEmotesCount": 3, "lastEmoteTriggeredAt": 1, "achievedTiers": [{ "steps": 2, "completedAt": 1 }] } }),
            ),
            (
                Kind::MovesMaster,
                &t,
                json!({ "progress": { "usedEmotesCount": 3, "lastEmoteTriggeredAt": 0, "achievedTiers": [] } }),
            ),
            (
                Kind::OpenForBusiness,
                &s,
                json!({ "progress": { "storeCreatedAt": null } }),
            ),
            (
                Kind::EventEnthusiast,
                &t,
                json!({ "progress": { "eventsAttended": [{ "eventId": "e", "firstVisitAt": 1, "timeSpentInSeconds": 1 }] } }),
            ),
            (Kind::Unsupported, &s, json!({ "progress": {} })),
        ] {
            assert_eq!(parse(kind, def, &data), None, "{kind:?} {data}");
        }
        assert_eq!(
            parse(
                Kind::OneTime,
                &s,
                &json!({ "progress": { "completedAt": 5.0 } })
            ),
            Some(Input::OneTime(5))
        );
        assert!(parse(Kind::OpenForBusiness, &s, &json!({ "progress": {} })).is_some());
    }

    #[test]
    fn items_union_dedupe_and_stamp_tiers_from_the_reaching_item() {
        let def = tiered(&[1, 3]);
        let current = UserState {
            steps: 1,
            doc: Map::from_iter([(
                "transactions_emotes_purchase".into(),
                json!([{ "transactionHash": "a", "saleAt": 50 }]),
            )]),
            tiers: vec![("t0".into(), 40)],
            completed_at: None,
        };
        let data = input(
            Kind::Emotionista,
            &def,
            json!({ "emotesBought": [
                { "nftId": "x", "transactionHash": "b", "saleAt": 20 },
                { "nftId": "y", "transactionHash": "a", "saleAt": 60 },
                { "nftId": "z", "transactionHash": "c", "saleAt": 30 }
            ] }),
        );
        let merged = merge(Kind::Emotionista, &def, Some(current), &data, NOW).unwrap();
        assert_eq!(merged.steps, 3);
        assert_eq!(
            Value::Object(merged.doc.clone()),
            json!({ "transactions_emotes_purchase": [
                { "transactionHash": "b", "saleAt": 20 },
                { "transactionHash": "c", "saleAt": 30 },
                { "transactionHash": "a", "saleAt": 60 }
            ] })
        );
        assert_eq!(merged.tiers, vec![("t0".into(), 20), ("t1".into(), 60)]);
        assert_eq!(merged.completed_at, Some(60));
        assert!(validate(&merged, &def).is_empty());
        let again = merge(Kind::Emotionista, &def, Some(merged.clone()), &data, NOW).unwrap();
        assert_eq!(again, merged, "a re-run changes nothing");
    }

    #[test]
    fn event_enthusiast_stamps_from_the_backfills_visits() {
        let def = tiered(&[1, 2, 5]);
        let data = input(
            Kind::EventEnthusiast,
            &def,
            json!({ "eventsAttended": [
                { "eventId": "e2", "eventName": "B", "firstVisitAt": 200, "timeSpentInSeconds": 400 },
                { "eventId": "e1", "eventName": "A", "firstVisitAt": 100, "timeSpentInSeconds": 400 }
            ] }),
        );
        let merged = merge(Kind::EventEnthusiast, &def, None, &data, NOW).unwrap();
        assert_eq!(merged.steps, 2);
        assert_eq!(merged.tiers, vec![("t0".into(), 100), ("t1".into(), 200)]);
        assert_eq!(merged.completed_at, None);
        assert_eq!(
            Value::Object(merged.doc),
            json!({ "events_ids_visited": ["e2", "e1"] })
        );
    }

    #[test]
    fn moves_master_rebuilds_the_row_from_the_backfill() {
        let def = tiered(&[100, 1000]);
        let data = input(
            Kind::MovesMaster,
            &def,
            json!({ "usedEmotesCount": 1200, "lastEmoteTriggeredAt": 99,
                    "achievedTiers": [{ "steps": 100, "completedAt": 10 }, { "steps": 1000, "completedAt": 20 }] }),
        );
        let stored = UserState {
            steps: 5,
            doc: Map::from_iter([("other".into(), json!(1))]),
            ..UserState::default()
        };
        let merged = merge(Kind::MovesMaster, &def, Some(stored), &data, NOW).unwrap();
        assert_eq!(merged.steps, 1200);
        assert_eq!(merged.tiers, vec![("t0".into(), 10), ("t1".into(), 20)]);
        assert_eq!(merged.completed_at, Some(NOW));
        assert_eq!(
            Value::Object(merged.doc),
            json!({ "last_used_emote_timestamp": 99, "last_day_used_emotes_timestamps": [] })
        );
    }

    #[test]
    fn social_butterfly_keeps_the_earliest_visit_per_profile() {
        let def = tiered(&[1, 2]);
        let data = input(
            Kind::SocialButterfly,
            &def,
            json!({ "profiles_visited": [
                { "profile_address": "0xb", "visited_at": 30 },
                { "profile_address": "0xa", "visited_at": 20 },
                { "profile_address": "0xb", "visited_at": 10 }
            ] }),
        );
        let merged = merge(Kind::SocialButterfly, &def, None, &data, NOW).unwrap();
        assert_eq!(merged.steps, 2);
        assert_eq!(merged.tiers, vec![("t0".into(), 10), ("t1".into(), 20)]);
        assert_eq!(merged.completed_at, Some(20));
    }

    #[test]
    fn single_badges_keep_their_first_completion() {
        let def = single(1);
        let one_time = input(Kind::OneTime, &def, json!({ "completedAt": 500 }));
        let merged = merge(Kind::OneTime, &def, None, &one_time, NOW).unwrap();
        assert_eq!((merged.steps, merged.completed_at), (1, Some(500)));
        assert_eq!(merged.doc.get("completed_at"), Some(&json!(500)));
        let earlier = UserState {
            steps: 1,
            doc: Map::from_iter([("completed_at".into(), json!("300"))]),
            completed_at: Some(400),
            ..UserState::default()
        };
        let kept = merge(Kind::OneTime, &def, Some(earlier), &one_time, NOW).unwrap();
        assert_eq!(kept.completed_at, Some(300));

        let land = input(
            Kind::LandArchitect,
            &def,
            json!({ "firstSceneDeployedAt": 70 }),
        );
        let merged = merge(Kind::LandArchitect, &def, None, &land, NOW).unwrap();
        assert_eq!((merged.steps, merged.completed_at), (1, Some(70)));

        let pro = input(
            Kind::ProfilePro,
            &def,
            json!({ "completedAt": 80, "description": "hi" }),
        );
        let merged = merge(Kind::ProfilePro, &def, None, &pro, NOW).unwrap();
        assert_eq!(merged.doc.get("description_added"), Some(&json!("hi")));
        assert_eq!(merged.completed_at, Some(80));

        let equipment = input(
            Kind::WearablesEquipment,
            &def,
            json!({ "completedAt": 90, "completedWith": ["urn:a"] }),
        );
        let merged = merge(Kind::WearablesEquipment, &def, None, &equipment, NOW).unwrap();
        assert_eq!((merged.steps, merged.completed_at), (1, Some(90)));
        assert_eq!(merged.doc.get("completed_with"), Some(&json!(["urn:a"])));
    }

    #[test]
    fn wearables_equipment_keeps_the_earliest_completion() {
        let def = single(1);
        let earlier = UserState {
            steps: 1,
            doc: Map::from_iter([("completed_with".into(), json!(["urn:old"]))]),
            completed_at: Some(50),
            ..UserState::default()
        };
        let later = input(
            Kind::WearablesEquipment,
            &def,
            json!({ "completedAt": 90, "completedWith": ["urn:new"] }),
        );
        let kept = merge(
            Kind::WearablesEquipment,
            &def,
            Some(earlier.clone()),
            &later,
            NOW,
        )
        .unwrap();
        assert_eq!(kept.completed_at, Some(50));
        assert_eq!(kept.doc.get("completed_with"), Some(&json!(["urn:old"])));

        let sooner = input(
            Kind::WearablesEquipment,
            &def,
            json!({ "completedAt": 20, "completedWith": ["urn:new"] }),
        );
        let moved = merge(Kind::WearablesEquipment, &def, Some(earlier), &sooner, NOW).unwrap();
        assert_eq!(moved.completed_at, Some(20));
        assert_eq!(moved.doc.get("completed_with"), Some(&json!(["urn:new"])));
    }

    #[test]
    fn open_for_business_counts_its_two_steps() {
        let def = single(2);
        let partial = input(
            Kind::OpenForBusiness,
            &def,
            json!({ "collectionCreatedAt": 10 }),
        );
        let merged = merge(Kind::OpenForBusiness, &def, None, &partial, NOW).unwrap();
        assert_eq!((merged.steps, merged.completed_at), (1, None));
        assert_eq!(
            Value::Object(merged.doc.clone()),
            json!({ "store_completed": false, "collection_submitted": true })
        );
        assert!(validate(&merged, &def).is_empty());

        let claimed = input(
            Kind::OpenForBusiness,
            &def,
            json!({ "collectionCreatedAt": 10, "completedAt": 20 }),
        );
        let merged = merge(Kind::OpenForBusiness, &def, None, &claimed, NOW).unwrap();
        assert_eq!(
            validate(&merged, &def),
            vec!["mismatch between badge criteria and progress steps"]
        );
    }

    #[test]
    fn moves_master_repeated_tiers_do_not_complete_the_badge() {
        let def = tiered(&[100, 500, 1000]);
        let data = input(
            Kind::MovesMaster,
            &def,
            json!({ "usedEmotesCount": 600, "lastEmoteTriggeredAt": 99,
                    "achievedTiers": [{ "steps": 100, "completedAt": 10 },
                                      { "steps": 100, "completedAt": 10 },
                                      { "steps": 500, "completedAt": 20 }] }),
        );
        let merged = merge(Kind::MovesMaster, &def, None, &data, NOW).unwrap();
        assert_eq!(
            validate(&merged, &def),
            vec!["mismatch between badge tiers and achieved tiers"]
        );
    }

    #[test]
    fn validation_reports_upstreams_errors() {
        let def = tiered(&[1, 3]);
        let user = UserState {
            steps: 2,
            tiers: vec![("t0".into(), 50), ("t1".into(), 10), ("nope".into(), 60)],
            completed_at: Some(1),
            ..UserState::default()
        };
        assert_eq!(
            validate(&user, &def),
            vec![
                "mismatch between badge tiers and achieved tiers",
                "tier achieved t1 criteria steps are greater than progress steps",
                "tier achieved t1 completed before previous tier t0",
                "tier achieved nope not found in badge tiers",
            ]
        );
    }
}
