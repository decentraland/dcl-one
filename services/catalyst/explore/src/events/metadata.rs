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

fn refresh_target(raw: &mut Map<String, Value>) {
    let generated = text(raw, "url").is_none_or(|value| {
        url::Url::parse(value).is_ok_and(|url| {
            url.host_str() == Some("decentraland.org") && matches!(url.path(), "/play" | "/play/")
        })
    });
    if !generated {
        return;
    }
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
    places: Option<&crate::places::PlacesComponent>,
) -> Result<(), ApiError> {
    refresh_target(raw);
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
        } else {
            raw.insert("image".into(), json!(super::posters::DEFAULT_EVENT_IMAGE));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
        enrich(&mut tx, &mut raw, Some(&places)).await.unwrap();
        assert_eq!(raw["estate_id"], "500");
        assert_eq!(raw["estate_name"], Value::Null);
        sqlx::query("DELETE FROM land_authz.estate_land WHERE land_token_id=$1::numeric")
            .bind(&token)
            .execute(&mut *tx)
            .await
            .unwrap();
        enrich(&mut tx, &mut raw, Some(&places)).await.unwrap();
        assert_eq!(
            raw["estate_id"],
            Value::Null,
            "published non-membership removes stale metadata"
        );
        sqlx::query("UPDATE land_authz.index_cursor SET folded_at=timezone('UTC',now())-interval '31 minutes' WHERE network=$1").bind(&settings.network).execute(&mut *tx).await.unwrap();
        enrich(&mut tx, &mut raw, Some(&places)).await.unwrap();
        assert_eq!(
            raw["estate_id"], "old",
            "unready authority retains known catalogue metadata"
        );
        raw.insert("world".into(), json!(true));
        raw.remove("place_id");
        enrich(&mut tx, &mut raw, Some(&places)).await.unwrap();
        assert_eq!(raw["estate_id"], Value::Null);
        tx.rollback().await.unwrap();
    }

    #[test]
    fn generated_targets_follow_location_but_external_targets_survive() {
        let mut raw = json!({"url":"https://decentraland.org/play/?position=1%2C2","x":4,"y":-5})
            .as_object()
            .unwrap()
            .clone();
        refresh_target(&mut raw);
        assert_eq!(raw["url"], "https://decentraland.org/play/?position=4%2C-5");
        raw.insert("world".into(), json!(true));
        raw.insert("server".into(), json!("hello.dcl.eth&position=1,2"));
        refresh_target(&mut raw);
        assert_eq!(
            raw["url"],
            "https://decentraland.org/play/?realm=hello.dcl.eth%26position%3D1%2C2"
        );
        raw.insert("url".into(), json!("https://example.org/event"));
        refresh_target(&mut raw);
        assert_eq!(raw["url"], "https://example.org/event");
    }
}
