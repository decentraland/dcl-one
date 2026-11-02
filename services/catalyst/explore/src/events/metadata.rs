//! Authoring metadata from the local profile and discovery projections.
//! Missing source facts remain unknown; an edit never substitutes the signer
//! for the creator or turns a scene title into an estate identity.

use serde_json::{Map, Value, json};
use sqlx::PgConnection;

use crate::shared::errors::ApiError;

fn text<'a>(raw: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    raw.get(key)
        .and_then(Value::as_str)
        .filter(|v| !v.is_empty())
}

fn refresh_target(raw: &mut Map<String, Value>, previous: &Map<String, Value>) {
    let generated = text(raw, "url").is_none_or(|value| {
        let mut expected = previous.clone();
        expected.remove("url");
        write_target(&mut expected);
        text(&expected, "url") == Some(value)
    });
    if generated {
        write_target(raw);
    }
}

fn write_target(raw: &mut Map<String, Value>) {
    let x = raw.get("x").and_then(Value::as_i64).unwrap_or(0);
    let y = raw.get("y").and_then(Value::as_i64).unwrap_or(0);
    let target = if raw.get("world").and_then(Value::as_bool) == Some(true)
        && let Some(server) = text(raw, "server")
    {
        format!(
            "https://decentraland.org/play/?realm={}",
            urlencoding::encode(server)
        )
    } else {
        format!("https://decentraland.org/play/?position={x}%2C{y}")
    };
    raw.insert("url".into(), json!(target));
}

pub(super) async fn enrich(
    conn: &mut PgConnection,
    raw: &mut Map<String, Value>,
    previous: Option<&Map<String, Value>>,
) -> Result<(), ApiError> {
    if let Some(previous) = previous {
        refresh_target(raw, previous);
    }
    let user = text(raw, "user").unwrap_or_default().to_lowercase();
    let name: Option<String> = sqlx::query_scalar(
        "SELECT d.entity_metadata->'avatars'->0->>'name' \
         FROM content.active_pointers p JOIN content.deployments d ON d.entity_id=p.entity_id \
         WHERE p.pointer=$1 AND d.entity_type='profile'",
    )
    .bind(user)
    .fetch_optional(&mut *conn)
    .await?
    .flatten();
    raw.insert("user_name".into(), json!(name.filter(|n| !n.is_empty())));
    let place_id = text(raw, "place_id").unwrap_or_default().to_owned();
    let place: Option<Value> = sqlx::query_scalar(
        "SELECT raw FROM explore.place_indexed WHERE id=$1 OR (world IS TRUE AND lower(world_name)=lower($1)) \
         ORDER BY (id=$1) DESC LIMIT 1",
    ).bind(place_id).fetch_optional(&mut *conn).await?;
    let place = place.unwrap_or(Value::Null);
    for key in ["estate_id", "estate_name"] {
        raw.insert(key.into(), place.get(key).cloned().unwrap_or(Value::Null));
    }
    raw.insert(
        "scene_name".into(),
        place.get("title").cloned().unwrap_or(Value::Null),
    );
    if text(raw, "image").is_none() {
        let fallback = place
            .get("image")
            .and_then(Value::as_str)
            .filter(|image| crate::sanitize::sanitize_asset_url(image).is_ok())
            .map(str::to_owned);
        if let Some(image) = fallback {
            raw.insert("image".into(), json!(image));
        } else if raw.get("world").and_then(Value::as_bool) != Some(true) {
            let x = raw.get("x").and_then(Value::as_i64).unwrap_or(0);
            let y = raw.get("y").and_then(Value::as_i64).unwrap_or(0);
            raw.insert(
                "image".into(),
                json!(format!(
                    "https://api.decentraland.org/v1/parcels/{x}/{y}/map.png"
                )),
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
