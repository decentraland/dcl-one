//! Authoring metadata from local profile and discovery projections, the
//! enrichment upstream did from the Land tiles and Places APIs
//! (`createEvent.ts:204-227`, `updateEvent.ts:339-390`): `user_name` from
//! the creator's profile this node holds, `estate_id`/`estate_name`/
//! `scene_name` and `place_id` from `explore.place_indexed` (the scene's
//! title stands for the tile name upstream read from Land), the generated
//! poster and the generated jump-in link. Missing lookups retain stored facts
//! at the same location; a move clears stale location facts; organizer
//! posters and explicit clears survive refreshes.

use serde_json::{Map, Value, json};
use sqlx::PgConnection;

use crate::shared::errors::ApiError;

/// `JUMP_IN_SITE_URL` (`Event/utils.ts:25-28`).
pub(super) const JUMP_IN_SITE_URL: &str = "https://decentraland.org/jump";
/// gatsby `Land` (`utils/api/Land`): `LAND_URL` default, the parcel and
/// estate map images upstream stored as the default poster
/// (`createEvent.ts:215-219`).
pub(super) const LAND_URL: &str = "https://api.decentraland.org/v1";

fn text<'a>(raw: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    raw.get(key)
        .and_then(Value::as_str)
        .filter(|v| !v.is_empty())
}

/// `eventTargetUrl` (`Event/utils.ts:65-76`): `JUMP_IN_SITE_URL/events`
/// with `position=x,y` and, when the event names a server, `realm=server`
/// (URLSearchParams encoding). Fixtures `events.create.minimal_success`,
/// `.world_success`.
pub(super) fn jump_in_url(server: Option<&str>, x: i32, y: i32) -> String {
    let mut url = format!(
        "{JUMP_IN_SITE_URL}/events?position={}",
        urlencoding::encode(&format!("{x},{y}"))
    );
    if let Some(server) = server.filter(|s| !s.is_empty()) {
        url.push_str("&realm=");
        url.push_str(&urlencoding::encode(server));
    }
    url
}

fn target(raw: &Map<String, Value>) -> String {
    jump_in_url(
        text(raw, "server"),
        raw.get("x").and_then(Value::as_i64).unwrap_or(0) as i32,
        raw.get("y").and_then(Value::as_i64).unwrap_or(0) as i32,
    )
}

/// The jump-in links this node generated before it took upstream's: only
/// the exact old target of the previous document counts as generated.
fn legacy_targets(previous: &Map<String, Value>) -> Vec<String> {
    let x = previous.get("x").and_then(Value::as_i64).unwrap_or(0);
    let y = previous.get("y").and_then(Value::as_i64).unwrap_or(0);
    let mut legacy = vec![format!("https://decentraland.org/play/?position={x}%2C{y}")];
    if previous.get("world").and_then(Value::as_bool) == Some(true)
        && let Some(server) = text(previous, "server")
    {
        legacy.push(format!(
            "https://decentraland.org/play/?realm={}",
            urlencoding::encode(server)
        ));
        legacy.push(format!("https://decentraland.org/play/?realm={server}"));
    }
    legacy
}

/// `updateEvent.ts:238-243`: an empty url or one under `JUMP_IN_SITE_URL`
/// is regenerated from the merged attributes; a custom link is kept.
pub(super) fn refresh_target(raw: &mut Map<String, Value>, previous: &Map<String, Value>) {
    let legacy = legacy_targets(previous);
    if text(raw, "url").is_none_or(|value| {
        value.starts_with(JUMP_IN_SITE_URL) || legacy.iter().any(|old| old == value)
    }) {
        raw.insert("url".into(), json!(target(raw)));
    }
}

fn location_changed(raw: &Map<String, Value>, previous: &Map<String, Value>) -> bool {
    ["place_id", "world", "server", "x", "y"]
        .iter()
        .any(|key| raw.get(*key) != previous.get(*key))
}

/// `createEvent.ts:209-224`: a world event's default poster is the events
/// site's `images/event-default.jpg`; a land event's the estate or parcel
/// map image.
pub(super) fn default_image(raw: &Map<String, Value>, events_base_url: &str) -> String {
    if raw.get("world").and_then(Value::as_bool) == Some(true) {
        return format!(
            "{}/images/event-default.jpg",
            events_base_url.trim_end_matches('/')
        );
    }
    match text(raw, "estate_id") {
        Some(estate_id) => format!("{LAND_URL}/estates/{estate_id}/map.png"),
        None => format!(
            "{LAND_URL}/parcels/{}/{}/map.png",
            raw.get("x").and_then(Value::as_i64).unwrap_or(0),
            raw.get("y").and_then(Value::as_i64).unwrap_or(0)
        ),
    }
}

fn refresh_poster(raw: &mut Map<String, Value>, creating: bool, events_base_url: &str) {
    // Provenance is local authoring metadata in the existing document, never
    // inferred from a poster URL that an organizer may have chosen explicitly.
    let generated = text(raw, "_image_source") == Some("generated")
        || (creating && text(raw, "image").is_none());
    if !generated {
        return;
    }
    raw.insert("image".into(), json!(default_image(raw, events_base_url)));
    raw.insert("_image_source".into(), json!("generated"));
}

pub(super) async fn enrich(
    conn: &mut PgConnection,
    raw: &mut Map<String, Value>,
    previous: Option<&Map<String, Value>>,
    refresh_url: bool,
    events_base_url: &str,
    places: Option<&crate::places::PlacesComponent>,
) -> Result<(), ApiError> {
    if refresh_url && let Some(previous) = previous {
        refresh_target(raw, previous);
    }
    if let Some(user) = text(raw, "user").map(str::to_lowercase) {
        // The deployers store a profile's metadata under `v`; the bare
        // shape is the older rows'.
        let name: Option<String> = sqlx::query_scalar(
            "SELECT coalesce(d.entity_metadata::jsonb->'v', d.entity_metadata::jsonb)->'avatars'->0->>'name' \
             FROM content.active_pointers p JOIN content.deployments d ON d.entity_id=p.entity_id \
             WHERE p.pointer=$1 AND d.entity_type='profile'",
        )
        .bind(user)
        .fetch_optional(&mut *conn)
        .await?
        .flatten();
        if let Some(name) = name.filter(|name| !name.is_empty()) {
            raw.insert("user_name".into(), json!(name));
        }
    }
    let moved = previous.is_some_and(|previous| location_changed(raw, previous));
    if moved {
        for key in ["scene_name", "estate_id", "estate_name"] {
            raw.insert(key.into(), Value::Null);
        }
    }
    let place: Option<Value> = if let Some(place_id) = text(raw, "place_id") {
        sqlx::query_scalar(
            "SELECT raw FROM explore.place_indexed WHERE id=$1 OR (world IS TRUE AND lower(world_name)=lower($1)) \
             ORDER BY (id=$1) DESC LIMIT 1",
        ).bind(place_id).fetch_optional(&mut *conn).await?
    } else {
        None
    };
    if let Some(place) = &place {
        for (source, key) in [
            ("title", "scene_name"),
            ("estate_id", "estate_id"),
            ("estate_name", "estate_name"),
        ] {
            if let Some(value) = place.get(source).filter(|value| !value.is_null()) {
                raw.insert(key.into(), value.clone());
            }
        }
    }
    if raw.get("world").and_then(Value::as_bool) != Some(true)
        && let Some(places) = places
        && let (Some(x), Some(y)) = (
            raw.get("x")
                .and_then(Value::as_i64)
                .and_then(|v| i32::try_from(v).ok()),
            raw.get("y")
                .and_then(Value::as_i64)
                .and_then(|v| i32::try_from(v).ok()),
        )
        && let Some(membership) = places.estate_at(conn, x, y).await?
    {
        // A fresh native answer supersedes a stale catalogue estate. A name
        // may only accompany the same known identity; no name is inferred.
        let old_id = raw.get("estate_id").and_then(Value::as_str);
        if old_id != membership.estate_id.as_deref() {
            raw.insert("estate_name".into(), Value::Null);
        }
        raw.insert("estate_id".into(), json!(membership.estate_id));
    }
    // A transient catalogue miss must not change an already generated poster
    // on an unrelated edit. A create or a move regenerates it for the new
    // location (the estate image once the estate is known).
    if previous.is_none() || moved || place.is_some() {
        refresh_poster(raw, previous.is_none(), events_base_url);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const EVENTS: &str = "https://events.decentraland.org";

    #[tokio::test]
    async fn native_estate_membership_replaces_stale_scene_facts_but_worlds_stay_unknown() {
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let mut settings = crate::land_authz::Settings::for_tests();
        settings.network = "ethereum-sepolia".into();
        settings.land_registry = format!("0x{:040x}", uuid::Uuid::new_v4().as_u128());
        settings.estate_registry = format!("0x{:040x}", uuid::Uuid::new_v4().as_u128());
        let places = crate::places::PlacesComponent::new(pool.clone(), "https://content.example");
        places.set_land_authz(std::sync::Arc::new(crate::land_authz::LandAuthzState::new(
            pool.clone(),
            crate::http::Outbound::for_tests(),
            settings.clone(),
            None,
        )));
        let mut tx = pool.begin().await.unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        let token = uuid::Uuid::new_v4().as_u128().to_string();
        sqlx::query("INSERT INTO explore.place(id,base_position,raw) VALUES($1,'7,8',$2)")
            .bind(&id)
            .bind(json!({"estate_id":"old","estate_name":"Old estate"}))
            .execute(&mut *tx)
            .await
            .unwrap();
        sqlx::query("INSERT INTO land_authz.index_cursor(network,land_registry,estate_registry,last_block,folded_block,folded_at,full_replay_started) VALUES($1,$2,$3,1,1,timezone('UTC',now()),true)")
            .bind(&settings.network).bind(&settings.land_registry).bind(&settings.estate_registry).execute(&mut *tx).await.unwrap();
        sqlx::query("INSERT INTO land_authz.token_right(token_address,token_id,x,y,updated_block,updated_log) VALUES($1,$2::numeric,7,8,1,0)")
            .bind(&settings.land_registry).bind(&token).execute(&mut *tx).await.unwrap();
        sqlx::query("INSERT INTO land_authz.estate_land(land_token_id,estate_token_id,updated_block,updated_log) VALUES($1::numeric,500,1,0)")
            .bind(&token).execute(&mut *tx).await.unwrap();
        let mut raw = json!({"place_id":id,"x":7,"y":8,"user":"0xabc"})
            .as_object()
            .unwrap()
            .clone();
        enrich(&mut tx, &mut raw, None, true, EVENTS, Some(&places))
            .await
            .unwrap();
        assert_eq!(raw["estate_id"], "500");
        assert_eq!(raw["estate_name"], Value::Null);
        sqlx::query(r#"UPDATE explore.place SET raw=raw || '{"estate_id":"500","estate_name":"Known estate"}' WHERE id=$1"#)
            .bind(&id).execute(&mut *tx).await.unwrap();
        enrich(&mut tx, &mut raw, None, true, EVENTS, Some(&places))
            .await
            .unwrap();
        assert_eq!(
            raw["estate_name"], "Known estate",
            "matching native identity preserves its known name"
        );
        sqlx::query(r#"UPDATE explore.place SET raw=raw || '{"estate_id":"old","estate_name":"Old estate"}' WHERE id=$1"#)
            .bind(&id).execute(&mut *tx).await.unwrap();
        sqlx::query("DELETE FROM land_authz.estate_land WHERE land_token_id=$1::numeric")
            .bind(&token)
            .execute(&mut *tx)
            .await
            .unwrap();
        enrich(&mut tx, &mut raw, None, true, EVENTS, Some(&places))
            .await
            .unwrap();
        assert_eq!(
            raw["estate_id"],
            Value::Null,
            "published non-membership removes stale metadata"
        );
        sqlx::query("UPDATE land_authz.index_cursor SET folded_at=timezone('UTC',now())-interval '31 minutes' WHERE network=$1").bind(&settings.network).execute(&mut *tx).await.unwrap();
        enrich(&mut tx, &mut raw, None, true, EVENTS, Some(&places))
            .await
            .unwrap();
        assert_eq!(
            raw["estate_id"], "old",
            "unready authority retains known catalogue metadata"
        );
        sqlx::query(
            "UPDATE land_authz.index_cursor SET folded_at=timezone('UTC',now()) WHERE network=$1",
        )
        .bind(&settings.network)
        .execute(&mut *tx)
        .await
        .unwrap();
        sqlx::query("INSERT INTO land_authz.estate_land(land_token_id,estate_token_id,updated_block,updated_log) VALUES($1::numeric,500,1,0)")
            .bind(&token).execute(&mut *tx).await.unwrap();
        raw.insert("world".into(), json!(true));
        raw.remove("place_id");
        raw.insert("estate_id".into(), json!("unchanged-world-fact"));
        let previous = raw.clone();
        enrich(
            &mut tx,
            &mut raw,
            Some(&previous),
            true,
            EVENTS,
            Some(&places),
        )
        .await
        .unwrap();
        assert_eq!(
            raw["estate_id"], "unchanged-world-fact",
            "world enrichment never consults a fresh parcel index at the same coordinates"
        );
        tx.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn missing_sources_preserve_stored_facts_until_the_location_changes() {
        let Some(pool) = crate::db::scratch::pool(1).await else {
            return;
        };
        let mut conn = pool.acquire().await.unwrap();
        let mut raw=json!({"user":"missing-profile","place_id":"missing-place","user_name":"Stored creator","scene_name":"Stored scene","estate_id":"stored-estate","estate_name":"Stored estate","x":1,"y":2,"image":"/poster/kept.webp"}).as_object().unwrap().clone();
        let previous = raw.clone();
        enrich(&mut conn, &mut raw, Some(&previous), true, EVENTS, None)
            .await
            .unwrap();
        for key in [
            "user_name",
            "scene_name",
            "estate_id",
            "estate_name",
            "image",
        ] {
            assert_eq!(raw[key], previous[key]);
        }
        let previous = raw.clone();
        raw.insert("x".into(), json!(5));
        raw.insert("place_id".into(), Value::Null);
        enrich(&mut conn, &mut raw, Some(&previous), true, EVENTS, None)
            .await
            .unwrap();
        assert_eq!(raw["user_name"], "Stored creator");
        for key in ["scene_name", "estate_id", "estate_name"] {
            assert!(raw[key].is_null(), "{key} belongs to the old location");
        }
        assert_eq!(raw["image"], "/poster/kept.webp");
        // Empty keys have no source; they must not replace stored facts.
        raw.insert("user".into(), json!(""));
        let previous = raw.clone();
        enrich(&mut conn, &mut raw, Some(&previous), true, EVENTS, None)
            .await
            .unwrap();
        assert_eq!(raw["user_name"], "Stored creator");
        raw.insert(
            "image".into(),
            json!("https://cdn.example.org/last-known-scene.png"),
        );
        raw.insert("_image_source".into(), json!("generated"));
        let previous = raw.clone();
        enrich(&mut conn, &mut raw, Some(&previous), true, EVENTS, None)
            .await
            .unwrap();
        assert_eq!(
            raw["image"], "https://cdn.example.org/last-known-scene.png",
            "same-location source miss retains a generated poster"
        );
        let previous = raw.clone();
        raw.insert("x".into(), json!(8));
        enrich(&mut conn, &mut raw, Some(&previous), true, EVENTS, None)
            .await
            .unwrap();
        assert_eq!(
            raw["image"], "https://api.decentraland.org/v1/parcels/8/2/map.png",
            "a move regenerates the parcel map poster (createEvent.ts:215-219)"
        );
    }

    /// `eventTargetUrl` (`Event/utils.ts:65-76`) and the old local targets.
    #[test]
    fn jump_in_targets_follow_upstream_and_legacy_local_targets_refresh() {
        assert_eq!(
            jump_in_url(None, 0, 0),
            "https://decentraland.org/jump/events?position=0%2C0"
        );
        assert_eq!(
            jump_in_url(Some("myworld.dcl.eth"), 0, 0),
            "https://decentraland.org/jump/events?position=0%2C0&realm=myworld.dcl.eth"
        );
        let body: super::super::moderation::CreateEvent=serde_json::from_value(json!({"name":"World","start_at":"2099-06-01T18:00:00Z","duration":3600000,"world":true,"server":"hello.dcl.eth&position=1,2","x":0,"y":0})).unwrap();
        let now = chrono::Utc::now();
        let doc = super::super::moderation::build_create_raw(
            &body,
            "creator",
            "2099-06-01T18:00:00Z".parse().unwrap(),
            None,
            true,
            now,
        )
        .unwrap();
        assert_eq!(
            doc["url"],
            "https://decentraland.org/jump/events?position=0%2C0&realm=hello.dcl.eth%26position%3D1%2C2"
        );
        for url in [
            doc["url"].as_str().unwrap(),
            "https://decentraland.org/play/?realm=hello.dcl.eth%26position%3D1%2C2",
            "https://decentraland.org/play/?realm=hello.dcl.eth&position=1,2",
        ] {
            let mut previous = doc.as_object().unwrap().clone();
            previous.insert("url".into(), json!(url));
            let mut edited = previous.clone();
            edited.insert("server".into(), json!("new.dcl.eth&position=3,4"));
            refresh_target(&mut edited, &previous);
            assert_eq!(
                edited["url"],
                "https://decentraland.org/jump/events?position=0%2C0&realm=new.dcl.eth%26position%3D3%2C4"
            );
        }
    }

    #[test]
    fn generated_targets_follow_location_but_external_targets_survive() {
        let mut raw = json!({"url":"https://decentraland.org/play/?position=1%2C2","x":4,"y":-5})
            .as_object()
            .unwrap()
            .clone();
        let previous = json!({"x":1,"y":2}).as_object().unwrap().clone();
        refresh_target(&mut raw, &previous);
        assert_eq!(
            raw["url"],
            "https://decentraland.org/jump/events?position=4%2C-5"
        );
        let previous = raw.clone();
        raw.insert("world".into(), json!(true));
        raw.insert("server".into(), json!("hello.dcl.eth&position=1,2"));
        refresh_target(&mut raw, &previous);
        assert_eq!(
            raw["url"],
            "https://decentraland.org/jump/events?position=4%2C-5&realm=hello.dcl.eth%26position%3D1%2C2"
        );
        raw.insert("url".into(), json!("https://example.org/event"));
        refresh_target(&mut raw, &previous);
        assert_eq!(raw["url"], "https://example.org/event");
        raw.insert(
            "url".into(),
            json!("https://decentraland.org/play/?realm=custom.dcl.eth&position=1,2"),
        );
        refresh_target(&mut raw, &previous);
        assert_eq!(
            raw["url"], "https://decentraland.org/play/?realm=custom.dcl.eth&position=1,2",
            "a play link that was never this node's generated target is custom"
        );
    }

    /// `createEvent.ts:209-224`: the land map or the events site's default.
    #[test]
    fn default_posters_follow_the_location() {
        let land = json!({"x":10,"y":-3}).as_object().unwrap().clone();
        assert_eq!(
            default_image(&land, EVENTS),
            "https://api.decentraland.org/v1/parcels/10/-3/map.png"
        );
        let estate = json!({"x":10,"y":-3,"estate_id":"42"})
            .as_object()
            .unwrap()
            .clone();
        assert_eq!(
            default_image(&estate, EVENTS),
            "https://api.decentraland.org/v1/estates/42/map.png"
        );
        let world = json!({"x":0,"y":0,"world":true,"server":"w.dcl.eth"})
            .as_object()
            .unwrap()
            .clone();
        assert_eq!(
            default_image(&world, "https://events.example.org/"),
            "https://events.example.org/images/event-default.jpg"
        );
    }

    #[tokio::test]
    async fn a_relative_default_remains_origin_independent_when_catalogue_metadata_is_missing() {
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let mut raw = json!({"image":super::super::posters::DEFAULT_EVENT_IMAGE,"_image_source":"generated","x":1,"y":2}).as_object().unwrap().clone();
        let previous = raw.clone();
        enrich(
            &mut pool.acquire().await.unwrap(),
            &mut raw,
            Some(&previous),
            true,
            EVENTS,
            None,
        )
        .await
        .unwrap();
        assert_eq!(raw["image"], super::super::posters::DEFAULT_EVENT_IMAGE);
    }
}
