//! Authoring metadata from local profile and discovery projections.
//! Missing lookups retain stored facts at the same location. A move clears
//! stale location facts; organizer posters and explicit clears survive refreshes.

use serde_json::{Map, Value, json};
use sqlx::PgConnection;

use crate::shared::errors::ApiError;

fn text<'a>(raw: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    raw.get(key)
        .and_then(Value::as_str)
        .filter(|v| !v.is_empty())
}

pub(super) fn jump_in_url(world: bool, server: Option<&str>, x: i32, y: i32) -> String {
    match server.filter(|server| world && !server.is_empty()) {
        Some(server) => format!(
            "https://decentraland.org/play/?realm={}",
            urlencoding::encode(server)
        ),
        None => format!("https://decentraland.org/play/?position={x}%2C{y}"),
    }
}

fn target(raw: &Map<String, Value>) -> String {
    jump_in_url(
        raw.get("world").and_then(Value::as_bool) == Some(true),
        text(raw, "server"),
        raw.get("x").and_then(Value::as_i64).unwrap_or(0) as i32,
        raw.get("y").and_then(Value::as_i64).unwrap_or(0) as i32,
    )
}

fn refresh_target(raw: &mut Map<String, Value>, previous: &Map<String, Value>) {
    // Older local creates interpolated the realm without encoding it. Only
    // that exact old target is recognized, leaving custom play links intact.
    let legacy = text(previous, "server")
        .filter(|_| previous.get("world").and_then(Value::as_bool) == Some(true))
        .map(|server| format!("https://decentraland.org/play/?realm={server}"));
    if text(raw, "url")
        .is_none_or(|value| value == target(previous) || legacy.as_deref() == Some(value))
    {
        raw.insert("url".into(), json!(target(raw)));
    }
}

fn location_changed(raw: &Map<String, Value>, previous: &Map<String, Value>) -> bool {
    ["place_id", "world", "server", "x", "y"]
        .iter()
        .any(|key| raw.get(*key) != previous.get(*key))
}

fn refresh_poster(raw: &mut Map<String, Value>, place: Option<&Value>, creating: bool) {
    // Provenance is local authoring metadata in the existing document, never
    // inferred from a poster URL that an organizer may have chosen explicitly.
    let generated = text(raw, "_image_source") == Some("generated")
        || (creating && text(raw, "image").is_none());
    if !generated {
        return;
    }
    let fallback = place
        .and_then(|place| place.get("image"))
        .and_then(Value::as_str)
        .filter(|image| crate::sanitize::sanitize_asset_url(image).is_ok())
        .map(str::to_owned)
        .unwrap_or_else(|| super::posters::DEFAULT_EVENT_IMAGE.to_owned());
    raw.insert("image".into(), json!(fallback));
    raw.insert("_image_source".into(), json!("generated"));
}

pub(super) async fn enrich(
    conn: &mut PgConnection,
    raw: &mut Map<String, Value>,
    previous: Option<&Map<String, Value>>,
    refresh_url: bool,
) -> Result<(), ApiError> {
    if refresh_url && let Some(previous) = previous {
        refresh_target(raw, previous);
    }
    if let Some(user) = text(raw, "user").map(str::to_lowercase) {
        let name: Option<String> = sqlx::query_scalar(
            "SELECT d.entity_metadata->'avatars'->0->>'name' \
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
    // A transient catalogue miss must not remove an already generated scene
    // poster on an unrelated edit. A move must discard that stale fallback.
    if previous.is_none() || moved || place.is_some() {
        refresh_poster(raw, place.as_ref(), previous.is_none());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn missing_sources_preserve_stored_facts_until_the_location_changes() {
        let Some(pool) = crate::db::scratch::pool(1).await else {
            return;
        };
        let mut conn = pool.acquire().await.unwrap();
        let mut raw=json!({"user":"missing-profile","place_id":"missing-place","user_name":"Stored creator","scene_name":"Stored scene","estate_id":"stored-estate","estate_name":"Stored estate","x":1,"y":2,"image":"/poster/kept.webp"}).as_object().unwrap().clone();
        let previous = raw.clone();
        enrich(&mut conn, &mut raw, Some(&previous), true)
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
        enrich(&mut conn, &mut raw, Some(&previous), true)
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
        enrich(&mut conn, &mut raw, Some(&previous), true)
            .await
            .unwrap();
        assert_eq!(raw["user_name"], "Stored creator");
        raw.insert(
            "image".into(),
            json!("https://cdn.example.org/last-known-scene.png"),
        );
        raw.insert("_image_source".into(), json!("generated"));
        let previous = raw.clone();
        enrich(&mut conn, &mut raw, Some(&previous), true)
            .await
            .unwrap();
        assert_eq!(
            raw["image"], "https://cdn.example.org/last-known-scene.png",
            "same-location source miss retains a generated scene poster"
        );
        let previous = raw.clone();
        raw.insert("x".into(), json!(8));
        enrich(&mut conn, &mut raw, Some(&previous), true)
            .await
            .unwrap();
        assert_eq!(
            raw["image"],
            super::super::posters::DEFAULT_EVENT_IMAGE,
            "a move cannot retain the previous scene poster"
        );
    }

    #[test]
    fn encoded_world_targets_and_legacy_unencoded_targets_refresh_identically() {
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
            "https://decentraland.org/play/?realm=hello.dcl.eth%26position%3D1%2C2"
        );
        for url in [
            doc["url"].as_str().unwrap(),
            "https://decentraland.org/play/?realm=hello.dcl.eth&position=1,2",
        ] {
            let mut previous = doc.as_object().unwrap().clone();
            previous.insert("url".into(), json!(url));
            let mut edited = previous.clone();
            edited.insert("server".into(), json!("new.dcl.eth&position=3,4"));
            refresh_target(&mut edited, &previous);
            assert_eq!(
                edited["url"],
                "https://decentraland.org/play/?realm=new.dcl.eth%26position%3D3%2C4"
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
        assert_eq!(raw["url"], "https://decentraland.org/play/?position=4%2C-5");
        let previous = raw.clone();
        raw.insert("world".into(), json!(true));
        raw.insert("server".into(), json!("hello.dcl.eth&position=1,2"));
        refresh_target(&mut raw, &previous);
        assert_eq!(
            raw["url"],
            "https://decentraland.org/play/?realm=hello.dcl.eth%26position%3D1%2C2"
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
            raw["url"],
            "https://decentraland.org/play/?realm=custom.dcl.eth&position=1,2"
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
        )
        .await
        .unwrap();
        assert_eq!(raw["image"], super::super::posters::DEFAULT_EVENT_IMAGE);
    }
}
