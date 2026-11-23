//! The worlds change feed: `worlds.change_log` holds, for every world change
//! made on this node, the message a worlds content server publishes to SNS
//! for it (`@dcl/schemas` `Events`, type `world`), written in the transaction
//! of the change. `GET /worlds-content-server/changes` pages it for a relay
//! that publishes the events; this node publishes nothing itself.
//!
//! Bodies are the upstream's exactly: the schemas forbid additional
//! properties and consumers drop an event that fails validation. Each
//! event's `timestamp` is the database clock read once per transaction,
//! after the world's row lock, so the events of one world are timestamped in
//! commit order. The worlds mirror and the TS import never write here.
//! Access grants, permissions and blocks are not carried.
//!
//! The route takes its own bearer, `WORLDS_CHANGE_FEED_TOKEN` (unset refuses
//! it), which grants nothing else: a deployment event carries the deploy's
//! auth chain, which no public worlds route serves.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use axum::Json;
use axum::extract::{Query, State};
use axum::http::HeaderMap;
use serde_json::{Value, json};
use sqlx::{PgConnection, PgPool};

use crate::comms::world_identity::{Parcel, WorldName};

use super::SharedState;
use super::auth::authorize_admin;
use super::http::ApiError;
use super::settings_policy::js_truthy;
use super::store::{WorldSettingsRow, effective_base_parcel};

pub const DEPLOYMENT: &str = "deployment";
pub const WORLD_SCENES_UNDEPLOYMENT: &str = "world_scenes_undeployment";
pub const WORLD_UNDEPLOYMENT: &str = "world_undeployment";
pub const WORLD_SETTINGS_CHANGED: &str = "world_settings_changed";
pub const WORLD_SPAWN_COORDINATE_SET: &str = "world_spawn_coordinate_set";

/// How long a row stays readable.
pub const RETENTION: chrono::TimeDelta = chrono::TimeDelta::days(30);
const RETENTION_BATCH: i64 = 1000;
const RETENTION_MAX_BATCHES: usize = 100;
pub(super) const RETENTION_INTERVAL: Duration = Duration::from_secs(60 * 60);

/// Rows a page holds without `limit`, and at most.
pub const DEFAULT_LIMIT: i64 = 500;
pub const MAX_LIMIT: i64 = 1000;

/// The upstream's SNS body budget for a scenes undeployment: 255 KiB, the
/// rest of SNS's 256 KiB left to the message attributes.
pub const MESSAGE_BODY_BUDGET_BYTES: usize = 255 * 1024;

/// The emission clock: the database's `clock_timestamp()` in epoch ms. Read
/// once per transaction, after the world's row lock (`now()` is the
/// transaction's start, before any lock wait).
pub(crate) async fn emission_clock(conn: &mut PgConnection) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT (extract(epoch FROM clock_timestamp()) * 1000)::bigint")
        .fetch_one(conn)
        .await
}

/// One event row, in the caller's transaction.
pub(crate) async fn record(
    conn: &mut PgConnection,
    world: &WorldName,
    event: &Value,
    attributes: &Value,
) -> Result<(), sqlx::Error> {
    let kind = event.get("subType").and_then(Value::as_str).unwrap_or("");
    sqlx::query(
        "INSERT INTO worlds.change_log (kind, world_name, event, attributes) VALUES ($1, $2, $3, $4)",
    )
    .bind(kind)
    .bind(world.key())
    .bind(sqlx::types::Json(event))
    .bind(sqlx::types::Json(attributes))
    .execute(conn)
    .await?;
    Ok(())
}

/// `WorldDeploymentEvent`: the scene's entity id and deploy auth chain, and
/// this node's public worlds base as the content server to fetch it from.
pub(crate) fn deployment(entity_id: &str, auth_chain: &Value, base: &str, timestamp: i64) -> Value {
    json!({
        "entity": { "entityId": entity_id, "authChain": auth_chain },
        "contentServerUrls": [base],
        "type": "world",
        "subType": DEPLOYMENT,
        "key": entity_id,
        "timestamp": timestamp,
    })
}

/// The deployment's message attributes: `isMultiplayer` is whether
/// `metadata.multiplayerId` is truthy.
pub(crate) fn deployment_attributes(entity: &Value) -> Value {
    let multiplayer = entity
        .get("metadata")
        .and_then(|m| m.get("multiplayerId"))
        .is_some_and(js_truthy);
    json!({ "isMultiplayer": if multiplayer { "true" } else { "false" }, "priority": "1" })
}

/// `WorldSettingsChangedEvent` from the stored settings. An unset title,
/// description, rating, category list or thumbnail is left out; an unset
/// skybox time is `null`. Access type, spawn and version are never part of it.
pub(crate) fn settings_changed(
    world: &WorldName,
    base: &str,
    settings: &WorldSettingsRow,
    timestamp: i64,
) -> Value {
    let mut metadata = json!({
        "worldName": world.display(),
        "skyboxTime": settings.skybox_time,
        "singlePlayer": settings.single_player,
        "showInPlaces": settings.show_in_places,
    });
    let text = [
        ("title", &settings.title),
        ("description", &settings.description),
        ("contentRating", &settings.content_rating),
    ];
    for (key, value) in text {
        if let Some(value) = value.as_deref().filter(|v| !v.is_empty()) {
            metadata[key] = json!(value);
        }
    }
    if let Some(categories) = &settings.categories {
        metadata["categories"] = json!(categories);
    }
    if let Some(hash) = settings.thumbnail_hash.as_deref().filter(|h| !h.is_empty()) {
        metadata["thumbnailUrl"] = json!(format!("{base}/contents/{hash}"));
    }
    settings_event(world, metadata, timestamp)
}

/// The `WorldSettingsChangedEvent` of an access edit: the new access type only.
pub(crate) fn access_changed(world: &WorldName, access_type: &str, timestamp: i64) -> Value {
    settings_event(
        world,
        json!({ "worldName": world.display(), "accessType": access_type }),
        timestamp,
    )
}

fn settings_event(world: &WorldName, metadata: Value, timestamp: i64) -> Value {
    json!({
        "type": "world",
        "subType": WORLD_SETTINGS_CHANGED,
        "key": format!("{}-{timestamp}", world.display()),
        "timestamp": timestamp,
        "metadata": metadata,
    })
}

/// `WorldSpawnCoordinateSetEvent` when a settings edit left the world with a
/// spawn other than `old`; `None` when the spawn is unset or unchanged.
pub(crate) fn spawn_coordinate_set(
    world: &WorldName,
    old: Option<&str>,
    new: Option<&str>,
    timestamp: i64,
) -> Option<Value> {
    let coordinate = |raw: Option<&str>| raw.and_then(|r| Parcel::parse(r).ok());
    let new = coordinate(new)?;
    let old = coordinate(old);
    if old == Some(new) {
        return None;
    }
    let point = |p: Parcel| json!({ "x": p.x, "y": p.y });
    Some(json!({
        "type": "world",
        "subType": WORLD_SPAWN_COORDINATE_SET,
        "key": format!("{}-{timestamp}", world.display()),
        "timestamp": timestamp,
        "metadata": {
            "name": world.display(),
            "oldCoordinate": old.map(point),
            "newCoordinate": point(new),
        },
    }))
}

/// `WorldUndeploymentEvent`.
pub(crate) fn world_undeployment(world: &WorldName, timestamp: i64) -> Value {
    json!({
        "type": "world",
        "subType": WORLD_UNDEPLOYMENT,
        "key": world.display(),
        "timestamp": timestamp,
        "metadata": { "worldName": world.display() },
    })
}

/// A scene a per-parcel undeploy removed, as its row stood.
pub(crate) struct RemovedScene {
    pub entity_id: String,
    pub entity: Value,
    pub parcels: Vec<String>,
}

/// The outcome of [`scenes_undeployment`].
pub(crate) enum ScenesUndeployment {
    /// The event, and how many footprints did not fit the body budget.
    Event { event: Value, omitted: usize },
    /// No removed scene has a base parcel: there is nothing to announce.
    Empty,
    /// The scene identities alone exceed the budget (the serialized size).
    TooLarge(usize),
}

/// `WorldScenesUndeploymentEvent`: every removed scene with an effective base
/// parcel, each with its footprint while the body stays within `budget`
/// bytes (a consumer fetches an omitted footprint by entity id).
pub(crate) fn scenes_undeployment(
    world: &WorldName,
    timestamp: i64,
    removed: &[RemovedScene],
    budget: usize,
) -> ScenesUndeployment {
    let scenes: Vec<(&str, String, Vec<String>)> = removed
        .iter()
        .filter_map(|scene| {
            let base = effective_base_parcel(&scene.entity, &scene.parcels)?;
            let mut seen = HashSet::new();
            let parcels: Vec<String> = scene
                .parcels
                .iter()
                .filter_map(|p| Parcel::parse(p).ok().map(|p| p.canonical()))
                .filter(|p| seen.insert(p.clone()))
                .collect();
            Some((scene.entity_id.as_str(), base, parcels))
        })
        .collect();
    if scenes.is_empty() {
        return ScenesUndeployment::Empty;
    }
    let mut event = json!({
        "type": "world",
        "subType": WORLD_SCENES_UNDEPLOYMENT,
        "key": world.display(),
        "timestamp": timestamp,
        "metadata": {
            "worldName": world.display(),
            "scenes": scenes
                .iter()
                .map(|(id, base, _)| json!({ "entityId": id, "baseParcel": base }))
                .collect::<Vec<_>>(),
        },
    });
    let mut size = event.to_string().len();
    if size > budget {
        return ScenesUndeployment::TooLarge(size);
    }
    let mut omitted = 0;
    for (index, (_, _, parcels)) in scenes.into_iter().enumerate() {
        let parcels = json!(parcels);
        let added = r#","parcels":"#.len() + parcels.to_string().len();
        if size + added <= budget {
            event["metadata"]["scenes"][index]["parcels"] = parcels;
            size += added;
        } else {
            omitted += 1;
        }
    }
    ScenesUndeployment::Event { event, omitted }
}

/// Records the scenes undeployment of `removed`, if any has an identity.
pub(crate) async fn record_scenes_undeployment(
    conn: &mut PgConnection,
    world: &WorldName,
    timestamp: i64,
    removed: &[RemovedScene],
) -> Result<(), sqlx::Error> {
    match scenes_undeployment(world, timestamp, removed, MESSAGE_BODY_BUDGET_BYTES) {
        ScenesUndeployment::Event { event, omitted } => {
            if omitted > 0 {
                tracing::warn!(%world, scenes = removed.len(), omitted, "scene footprints left out of the undeployment event: over the body budget");
            }
            record(conn, world, &event, &json!({})).await
        }
        ScenesUndeployment::Empty => Ok(()),
        ScenesUndeployment::TooLarge(size) => {
            tracing::warn!(%world, scenes = removed.len(), size, "scenes undeployment not recorded: its identities exceed the body budget");
            Ok(())
        }
    }
}

/// A feed position: the writing transaction's id and the row id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    pub txid: u64,
    pub id: i64,
}

impl Cursor {
    pub const START: Self = Self { txid: 0, id: 0 };

    /// `<txid>:<id>`, both decimal; anything else is `None`.
    pub fn parse(raw: &str) -> Option<Self> {
        let (txid, id) = raw.split_once(':')?;
        let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
        if !digits(txid) || !digits(id) {
            return None;
        }
        Some(Self {
            txid: txid.parse().ok()?,
            id: id.parse().ok()?,
        })
    }
}

impl std::fmt::Display for Cursor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.txid, self.id)
    }
}

/// One feed entry.
#[derive(Debug, Clone, PartialEq)]
pub struct Change {
    pub cursor: Cursor,
    pub event: Value,
    pub attributes: Value,
}

/// Up to `limit` rows after `after` in (txid, id) order, only of
/// transactions older than every transaction still running, and whether more
/// such rows follow.
pub async fn read(
    pool: &PgPool,
    after: Cursor,
    limit: i64,
) -> Result<(Vec<Change>, bool), sqlx::Error> {
    let rows: Vec<(String, i64, Value, Value)> = sqlx::query_as(
        "SELECT txid::text, id, event, attributes FROM worlds.change_log \
         WHERE (txid, id) > ($1::text::xid8, $2) \
           AND txid < pg_snapshot_xmin(pg_current_snapshot()) \
         ORDER BY txid, id LIMIT $3",
    )
    .bind(after.txid.to_string())
    .bind(after.id)
    .bind(limit + 1)
    .fetch_all(pool)
    .await?;
    let more = rows.len() as i64 > limit;
    let changes = rows
        .into_iter()
        .take(limit as usize)
        .map(|(txid, id, event, attributes)| Change {
            cursor: Cursor {
                txid: txid.parse().unwrap_or(0),
                id,
            },
            event,
            attributes,
        })
        .collect();
    Ok((changes, more))
}

/// `GET /changes?after=<txid>:<id>&limit=<n>`: the feed after `after` (from
/// the oldest retained row without it). `next` is the last entry's cursor
/// while more rows are readable, `null` once caught up; a reader then polls
/// again with the last cursor it took.
pub async fn changes(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Value>, ApiError> {
    // The worlds bearer check (constant time, unset fails closed) over the feed's own token.
    authorize_admin(state.settings.change_feed_token.as_deref(), &headers)?;
    let after = match query.get("after").map(String::as_str) {
        None | Some("") => Cursor::START,
        Some(raw) => Cursor::parse(raw).ok_or_else(|| {
            ApiError::bad_request(format!(
                "Invalid cursor \"{raw}\": expected <txid>:<id> as returned by this route."
            ))
        })?,
    };
    let limit = match query.get("limit") {
        None => DEFAULT_LIMIT,
        Some(raw) => raw
            .parse::<i64>()
            .ok()
            .filter(|n| *n > 0)
            .ok_or_else(|| {
                ApiError::bad_request(format!(
                    "Invalid limit \"{raw}\": expected a positive integer."
                ))
            })?
            .min(MAX_LIMIT),
    };
    let (changes, more) = read(&state.pool, after, limit).await?;
    let next = changes
        .last()
        .filter(|_| more)
        .map(|c| c.cursor.to_string());
    let changes: Vec<Value> = changes
        .into_iter()
        .map(|c| {
            json!({
                "cursor": c.cursor.to_string(),
                "event": c.event,
                "attributes": c.attributes,
            })
        })
        .collect();
    Ok(Json(json!({ "changes": changes, "next": next })))
}

/// Deletes the rows created before `cutoff`, one bounded statement at a
/// time; returns how many went.
pub async fn delete_expired(
    pool: &PgPool,
    cutoff: chrono::NaiveDateTime,
) -> Result<u64, sqlx::Error> {
    let mut deleted = 0;
    for _ in 0..RETENTION_MAX_BATCHES {
        let n = sqlx::query(
            "DELETE FROM worlds.change_log WHERE id IN ( \
                 SELECT id FROM worlds.change_log WHERE created_at < $1 \
                 ORDER BY created_at LIMIT $2)",
        )
        .bind(cutoff)
        .bind(RETENTION_BATCH)
        .execute(pool)
        .await?
        .rows_affected();
        deleted += n;
        if n < RETENTION_BATCH as u64 {
            break;
        }
    }
    Ok(deleted)
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    fn world() -> WorldName {
        WorldName::parse("Example.dcl.eth").unwrap()
    }

    #[test]
    fn cursors_round_trip_and_refuse_anything_else() {
        let c = Cursor::parse("812:44").unwrap();
        assert_eq!(c, Cursor { txid: 812, id: 44 });
        assert_eq!(c.to_string(), "812:44");
        for bad in [
            "",
            "1",
            "1:",
            ":1",
            "a:1",
            "1:b",
            "-1:2",
            "1:-2",
            "1:2:3",
            " 1:2",
            "+1:2",
            "99999999999999999999:1",
        ] {
            assert_eq!(Cursor::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn the_deployment_body_and_attributes_are_the_upstream_shapes() {
        let chain = json!([{ "type": "SIGNER", "payload": "0xabc", "signature": "" }]);
        assert_eq!(
            deployment(
                "bafyscene",
                &chain,
                "https://worlds.example",
                1_700_000_000_123
            ),
            json!({
                "entity": { "entityId": "bafyscene", "authChain": chain },
                "contentServerUrls": ["https://worlds.example"],
                "type": "world",
                "subType": "deployment",
                "key": "bafyscene",
                "timestamp": 1_700_000_000_123_i64,
            })
        );
        let with = |id: Value| json!({ "metadata": { "multiplayerId": id } });
        assert_eq!(
            deployment_attributes(&with(json!("room"))),
            json!({ "isMultiplayer": "true", "priority": "1" })
        );
        for falsy in [json!(""), json!(0), json!(null), json!(false)] {
            assert_eq!(
                deployment_attributes(&with(falsy))["isMultiplayer"],
                "false"
            );
        }
        assert_eq!(deployment_attributes(&json!({}))["isMultiplayer"], "false");
    }

    fn settings() -> WorldSettingsRow {
        WorldSettingsRow {
            title: None,
            description: Some(String::new()),
            content_rating: None,
            spawn_coordinates: Some("1,1".into()),
            skybox_time: None,
            categories: None,
            single_player: false,
            show_in_places: true,
            thumbnail_hash: None,
            access_type: Some("allow-list".into()),
            realm_name_override: Some("realm".into()),
            preview_wearable_urns: None,
            settings_version: 7,
        }
    }

    #[test]
    fn settings_leave_out_what_is_unset_and_never_carry_access_spawn_or_version() {
        assert_eq!(
            settings_changed(&world(), "https://worlds.example", &settings(), 5),
            json!({
                "type": "world",
                "subType": "world_settings_changed",
                "key": "Example.dcl.eth-5",
                "timestamp": 5,
                "metadata": {
                    "worldName": "Example.dcl.eth",
                    "skyboxTime": null,
                    "singlePlayer": false,
                    "showInPlaces": true,
                },
            })
        );
        let full = WorldSettingsRow {
            title: Some("T".into()),
            description: Some("D".into()),
            content_rating: Some("E".into()),
            skybox_time: Some(3600),
            categories: Some(Vec::new()),
            thumbnail_hash: Some("bafythumb".into()),
            ..settings()
        };
        assert_eq!(
            settings_changed(&world(), "https://worlds.example", &full, 5)["metadata"],
            json!({
                "worldName": "Example.dcl.eth",
                "title": "T",
                "description": "D",
                "contentRating": "E",
                "skyboxTime": 3600,
                "categories": [],
                "singlePlayer": false,
                "showInPlaces": true,
                "thumbnailUrl": "https://worlds.example/contents/bafythumb",
            })
        );
        assert_eq!(
            access_changed(&world(), "shared-secret", 9)["metadata"],
            json!({ "worldName": "Example.dcl.eth", "accessType": "shared-secret" })
        );
    }

    #[test]
    fn a_spawn_event_needs_a_new_spawn_other_than_the_old_one() {
        assert_eq!(
            spawn_coordinate_set(&world(), Some("1,1"), Some("1,1"), 1),
            None
        );
        assert_eq!(spawn_coordinate_set(&world(), Some("1,1"), None, 1), None);
        assert_eq!(
            spawn_coordinate_set(&world(), None, Some("-2,3"), 4).unwrap(),
            json!({
                "type": "world",
                "subType": "world_spawn_coordinate_set",
                "key": "Example.dcl.eth-4",
                "timestamp": 4,
                "metadata": {
                    "name": "Example.dcl.eth",
                    "oldCoordinate": null,
                    "newCoordinate": { "x": -2, "y": 3 },
                },
            })
        );
        assert_eq!(
            spawn_coordinate_set(&world(), Some("0,0"), Some("1,0"), 4).unwrap()["metadata"]["oldCoordinate"],
            json!({ "x": 0, "y": 0 })
        );
    }

    fn removed(id: &str, base: Option<&str>, parcels: &[&str]) -> RemovedScene {
        RemovedScene {
            entity_id: id.into(),
            entity: json!({ "metadata": { "scene": { "base": base } } }),
            parcels: parcels.iter().map(|p| p.to_string()).collect(),
        }
    }

    #[test]
    fn scenes_undeployment_names_scenes_with_a_base_and_fits_footprints_in_the_budget() {
        let scenes = [
            removed("a", Some("0,0"), &["0,0", "0,1", "0,0"]),
            removed("b", None, &["5,5", "4,5"]),
            removed("c", Some("9,9"), &[]),
        ];
        let ScenesUndeployment::Event { event, omitted } =
            scenes_undeployment(&world(), 3, &scenes, MESSAGE_BODY_BUDGET_BYTES)
        else {
            panic!("expected an event");
        };
        assert_eq!(omitted, 0);
        assert_eq!(
            event,
            json!({
                "type": "world",
                "subType": "world_scenes_undeployment",
                "key": "Example.dcl.eth",
                "timestamp": 3,
                "metadata": {
                    "worldName": "Example.dcl.eth",
                    "scenes": [
                        { "entityId": "a", "baseParcel": "0,0", "parcels": ["0,0", "0,1"] },
                        { "entityId": "b", "baseParcel": "4,5", "parcels": ["5,5", "4,5"] },
                    ],
                },
            })
        );
        let identities = json!({
            "type": "world", "subType": "world_scenes_undeployment", "key": "Example.dcl.eth",
            "timestamp": 3,
            "metadata": { "worldName": "Example.dcl.eth", "scenes": [
                { "entityId": "a", "baseParcel": "0,0" }, { "entityId": "b", "baseParcel": "4,5" } ] },
        })
        .to_string()
        .len();
        let first = r#","parcels":["0,0","0,1"]"#.len();
        let ScenesUndeployment::Event { event, omitted } =
            scenes_undeployment(&world(), 3, &scenes, identities + first)
        else {
            panic!("expected an event");
        };
        assert_eq!(omitted, 1);
        assert_eq!(event.to_string().len(), identities + first);
        assert!(event["metadata"]["scenes"][1].get("parcels").is_none());
        assert!(matches!(
            scenes_undeployment(&world(), 3, &scenes, identities - 1),
            ScenesUndeployment::TooLarge(size) if size == identities
        ));
        assert!(matches!(
            scenes_undeployment(
                &world(),
                3,
                &[removed("c", Some("9,9"), &[])],
                MESSAGE_BODY_BUDGET_BYTES
            ),
            ScenesUndeployment::Empty
        ));
    }
}
