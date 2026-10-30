//! The place/world wire rows and the SQL that projects `explore.place_indexed`
//! onto them. Every raw jsonb key a third party wrote is read through a
//! `jsonb_typeof` guard (query.rs): one malformed value must cost one field,
//! never the whole listing.

use std::sync::LazyLock;

use chrono::{DateTime, NaiveDateTime, Utc};
use serde::Serialize;
use sqlx::Row;

use super::query::{
    EXCLUDE_FROM_RANKING_SQL, IS_PRIVATE_SQL, SHOW_IN_PLACES_SQL, SINGLE_PLAYER_SQL,
    USER_DISLIKE_SQL, USER_FAVORITE_SQL, USER_LIKE_SQL, raw_float8_sql, raw_int_sql,
    raw_timestamptz_sql,
};
use crate::sanitize::{ContentOrigin, sanitize_image_url, sanitize_place_description};

pub(super) fn place_columns() -> &'static str {
    static SQL: LazyLock<String> = LazyLock::new(|| {
        let disabled_at = raw_timestamptz_sql("disabled_at");
        let created_at = raw_timestamptz_sql("created_at");
        let updated_at = raw_timestamptz_sql("updated_at");
        let ranking = raw_float8_sql("ranking");
        let skybox_time = raw_float8_sql("skybox_time");
        let user_count = raw_int_sql("user_count");
        let user_visits = raw_int_sql("user_visits");
        let settings_version = raw_int_sql("settings_version");
        let like_rate = raw_float8_sql("like_rate");
        let like_score = raw_float8_sql("like_score");
        format!(
            r#"
    id, title, description, raw->>'image' AS image,
    creator_address AS owner,
    creator_address,
    COALESCE((SELECT array_agg(p::text) FROM jsonb_array_elements_text(raw->'positions') p), ARRAY[]::text[]) AS positions,
    base_position,
    raw->>'contact_name' AS contact_name,
    raw->>'contact_email' AS contact_email,
    content_rating,
    disabled,
    {disabled_at} AS disabled_at,
    raw->>'disabled_reason' AS disabled_reason,
    {created_at} AS created_at,
    {updated_at} AS updated_at,
    favorites + COALESCE((SELECT ic.favorites FROM explore.place_interaction_counts ic WHERE ic.entity_id = place_indexed.id), 0) AS favorites,
    likes + COALESCE((SELECT ic.likes FROM explore.place_interaction_counts ic WHERE ic.entity_id = place_indexed.id), 0) AS likes,
    dislikes + COALESCE((SELECT ic.dislikes FROM explore.place_interaction_counts ic WHERE ic.entity_id = place_indexed.id), 0) AS dislikes,
    categories,
    highlighted,
    raw->>'highlighted_image' AS highlighted_image,
    {ranking} AS ranking,
    {EXCLUDE_FROM_RANKING_SQL} AS exclude_from_ranking,
    raw->>'sdk' AS sdk,
    deployed_at,
    world,
    world_name,
    raw->>'world_id' AS world_id,
    raw->>'deployment_id' AS deployment_id,
    {IS_PRIVATE_SQL} AS is_private,
    {SHOW_IN_PLACES_SQL} AS show_in_places,
    {SINGLE_PLAYER_SQL} AS single_player,
    {skybox_time} AS skybox_time,
    {USER_FAVORITE_SQL} AS user_favorite,
    {USER_LIKE_SQL} AS user_like,
    {USER_DISLIKE_SQL} AS user_dislike,
    {user_count} AS user_count,
    COALESCE({user_visits}, 0) AS user_visits,
    {settings_version} AS settings_version,
    {like_rate} AS like_rate,
    {like_score} AS like_score
"#
        )
    });
    &SQL
}

#[derive(Debug, Clone, Default, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "places/"))]
pub struct PlaceRow {
    #[serde(skip)]
    #[cfg_attr(feature = "ts", ts(skip))]
    pub(crate) scene_place: bool,
    pub id: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub image: Option<String>,
    pub owner: Option<String>,
    pub positions: Vec<String>,
    pub base_position: String,
    pub contact_name: Option<String>,
    pub contact_email: Option<String>,
    pub content_rating: Option<String>,
    pub disabled: bool,
    #[cfg_attr(feature = "ts", ts(type = "string | null"))]
    pub disabled_at: Option<DateTime<Utc>>,
    pub disabled_reason: Option<String>,
    #[cfg_attr(feature = "ts", ts(type = "string | null"))]
    pub created_at: Option<DateTime<Utc>>,
    #[cfg_attr(feature = "ts", ts(type = "string | null"))]
    pub updated_at: Option<DateTime<Utc>>,
    pub favorites: i32,
    pub likes: i32,
    pub dislikes: i32,
    pub categories: Vec<String>,
    pub highlighted: bool,
    pub highlighted_image: Option<String>,
    pub ranking: Option<f64>,
    pub exclude_from_ranking: bool,
    pub sdk: Option<String>,
    pub creator_address: Option<String>,
    pub world_id: Option<String>,
    /// Immutable content-entity identifier for this indexed deployment.
    /// Null for legacy rows awaiting reconciliation (upstream places #856).
    pub deployment_id: Option<String>,
    #[cfg_attr(feature = "ts", ts(type = "string | null"))]
    pub deployed_at: Option<DateTime<Utc>>,
    pub world: bool,
    pub world_name: Option<String>,
    #[serde(skip)]
    pub is_private: bool,
    #[serde(skip)]
    pub show_in_places: bool,
    #[serde(skip)]
    pub single_player: bool,
    #[serde(skip)]
    pub skybox_time: Option<f64>,
    /// The world settings counter upstream mirrors from its worlds server
    /// (`null` until a settings write lands); serialized on worlds only.
    #[serde(skip)]
    pub settings_version: Option<i32>,
    pub user_favorite: bool,
    pub user_like: bool,
    pub user_dislike: bool,
    pub user_count: Option<i32>,
    pub user_visits: i32,
    pub like_rate: Option<f64>,
    pub like_score: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub live: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional, type = "string | null"))]
    pub live_event_name: Option<Option<String>>,
    /// Wallet addresses connected here, present only when with_connected_users=true:
    /// an empty array means nobody is connected, null means the comms service could
    /// not be reached and the connected users are unknown.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional, type = "Array<string> | null"))]
    pub connected_addresses: Option<Option<Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional, type = "Array<Record<string, unknown>>"))]
    pub realms_detail: Option<Vec<serde_json::Value>>,
}

impl PlaceRow {
    /// `realms_detail`, when asked for (`Some(realm)`): the realms with users
    /// in the place. Upstream reads them off the hot scenes of every realm;
    /// this node knows one realm, its own, so the list is that realm with
    /// the row's headcount, or empty when nobody is there. Worlds carry none.
    pub fn apply_realms_detail(&mut self, realm: Option<&str>) {
        let Some(realm) = realm else { return };
        if self.world {
            return;
        }
        let users = self.user_count.unwrap_or(0);
        self.realms_detail = Some(if users > 0 {
            vec![serde_json::json!({ "serverName": realm, "usersCount": users })]
        } else {
            Vec::new()
        });
    }
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "places/"))]
pub struct WorldRow {
    pub id: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub image: Option<String>,
    pub owner: Option<String>,
    pub world_name: Option<String>,
    pub content_rating: Option<String>,
    pub categories: Vec<String>,
    pub likes: i32,
    pub dislikes: i32,
    pub favorites: i32,
    pub like_rate: Option<f64>,
    pub like_score: Option<f64>,
    #[cfg_attr(feature = "ts", ts(type = "string | null"))]
    pub created_at: Option<DateTime<Utc>>,
    #[cfg_attr(feature = "ts", ts(type = "string | null"))]
    pub updated_at: Option<DateTime<Utc>>,
    pub disabled: bool,
    #[cfg_attr(feature = "ts", ts(type = "string | null"))]
    pub disabled_at: Option<DateTime<Utc>>,
    pub base_position: String,
    pub contact_name: Option<String>,
    #[cfg_attr(feature = "ts", ts(type = "string | null"))]
    pub deployed_at: Option<DateTime<Utc>>,
    pub highlighted: bool,
    pub highlighted_image: Option<String>,
    pub ranking: Option<f64>,
    pub exclude_from_ranking: bool,
    pub world: bool,
    pub is_private: bool,
    pub show_in_places: bool,
    pub single_player: bool,
    pub skybox_time: Option<f64>,
    pub settings_version: Option<i32>,
    pub user_like: bool,
    pub user_dislike: bool,
    pub user_favorite: bool,
    pub user_count: Option<i32>,
    pub user_visits: i32,
}

impl From<PlaceRow> for WorldRow {
    fn from(p: PlaceRow) -> Self {
        Self {
            id: p.id,
            title: p.title,
            description: p.description,
            image: p.image,
            owner: p.owner,
            world_name: p.world_name,
            content_rating: p.content_rating,
            categories: p.categories,
            likes: p.likes,
            dislikes: p.dislikes,
            favorites: p.favorites,
            like_rate: p.like_rate,
            like_score: p.like_score,
            created_at: p.created_at,
            updated_at: p.updated_at,
            disabled: p.disabled,
            disabled_at: p.disabled_at,
            base_position: p.base_position,
            contact_name: p.contact_name,
            deployed_at: p.deployed_at,
            highlighted: p.highlighted,
            highlighted_image: p.highlighted_image,
            ranking: p.ranking,
            exclude_from_ranking: p.exclude_from_ranking,
            world: p.world,
            is_private: p.is_private,
            show_in_places: p.show_in_places,
            single_player: p.single_player,
            skybox_time: p.skybox_time,
            settings_version: p.settings_version,
            user_like: p.user_like,
            user_dislike: p.user_dislike,
            user_favorite: p.user_favorite,
            user_count: p.user_count,
            user_visits: p.user_visits,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct PlaceListFilters {
    pub limit: i64,
    pub offset: i64,
    pub positions: Vec<String>,
    pub names: Vec<String>,
    pub categories: Vec<String>,
    pub only_highlighted: bool,
    pub only_excluded_from_ranking: bool,
    pub search: Option<String>,
    pub creator_address: Option<String>,
    pub sdk: Option<String>,
    pub order_by: PlaceOrderBy,
    pub order_desc: bool,
    pub ids: Vec<String>,
    pub only_worlds: bool,
    pub only_places: bool,
    /// `/api/map` lists categorized places only (upstream's
    /// `array_length(p.categories, 1) > 0`, applied to the count as well).
    pub only_categorized: bool,
    pub operated_positions: Vec<String>,
    pub owner_filtered: bool,
    pub destinations_mode: bool,
    pub require_content_places: bool,
    pub require_content_worlds: bool,
    pub place_user_counts: Vec<(String, i32)>,
    pub world_user_counts: Vec<(String, i32)>,
    /// Lowercased signer whose favorites/likes ride along in the page query.
    pub viewer: Option<String>,
    /// Restrict the page to `viewer`'s favorites inside the query (EXISTS).
    pub viewer_favorites_only: bool,
}

/// Viewer interaction columns; `idx` is the placeholder bound to the lowercased address.
pub(super) fn viewer_columns(idx: usize) -> String {
    format!(
        r#", EXISTS (SELECT 1 FROM explore.user_favorites uf WHERE uf.entity_id = place_indexed.id AND lower(uf."user") = ${idx}) AS viewer_favorite,
    (SELECT ul."like" FROM explore.user_likes ul WHERE ul.entity_id = place_indexed.id AND lower(ul."user") = ${idx} LIMIT 1) AS viewer_like"#
    )
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PlaceOrderBy {
    #[default]
    LikeScore,
    UpdatedAt,
    CreatedAt,
    UserVisits,
    MostActive,
}

impl PlaceOrderBy {
    pub fn parse(s: Option<&str>) -> Self {
        match s {
            Some("updated_at") => Self::UpdatedAt,
            Some("created_at") => Self::CreatedAt,
            Some("user_visits") => Self::UserVisits,
            Some("most_active") => Self::MostActive,
            _ => Self::LikeScore,
        }
    }
    pub(super) fn column(self) -> &'static str {
        static UPDATED_AT: LazyLock<String> = LazyLock::new(|| raw_timestamptz_sql("updated_at"));
        static CREATED_AT: LazyLock<String> = LazyLock::new(|| raw_timestamptz_sql("created_at"));
        static USER_VISITS: LazyLock<String> =
            LazyLock::new(|| format!("COALESCE({}, 0)", raw_int_sql("user_visits")));
        static MOST_ACTIVE: LazyLock<String> =
            LazyLock::new(|| format!("COALESCE({}, 0)", raw_int_sql("user_count")));
        static LIKE_SCORE: LazyLock<String> = LazyLock::new(|| raw_float8_sql("like_score"));
        match self {
            Self::LikeScore => &LIKE_SCORE,
            Self::UpdatedAt => &UPDATED_AT,
            Self::CreatedAt => &CREATED_AT,
            Self::UserVisits => &USER_VISITS,
            Self::MostActive => &MOST_ACTIVE,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum CategoryTarget {
    All,
    Places,
    Worlds,
}

impl CategoryTarget {
    pub fn parse(s: Option<&str>) -> Self {
        match s {
            Some("places") => Self::Places,
            Some("worlds") => Self::Worlds,
            _ => Self::All,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PlaceStatusRow {
    pub id: String,
    pub disabled: bool,
    pub world: bool,
    pub world_name: Option<String>,
    pub base_position: String,
}

/// A `timestamp without time zone` column (UTC) or a `timestamptz` expression.
fn ts(r: &sqlx::postgres::PgRow, col: &str) -> Option<DateTime<Utc>> {
    if let Ok(naive) = r.try_get::<Option<NaiveDateTime>, _>(col) {
        return naive.map(|n| n.and_utc());
    }
    r.try_get::<Option<DateTime<Utc>>, _>(col).unwrap_or(None)
}

pub(super) fn row_to_place(
    r: sqlx::postgres::PgRow,
    content_origin: Option<&ContentOrigin>,
) -> PlaceRow {
    let viewer_favorite = r.try_get::<bool, _>("viewer_favorite").ok();
    let viewer_like = r.try_get::<Option<bool>, _>("viewer_like").ok().flatten();
    let (user_favorite, user_like, user_dislike) =
        if viewer_favorite == Some(true) || viewer_like.is_some() {
            (
                viewer_favorite.unwrap_or(false),
                viewer_like == Some(true),
                viewer_like == Some(false),
            )
        } else {
            (
                r.try_get::<bool, _>("user_favorite").unwrap_or(false),
                r.try_get::<bool, _>("user_like").unwrap_or(false),
                r.try_get::<bool, _>("user_dislike").unwrap_or(false),
            )
        };
    PlaceRow {
        scene_place: false,
        id: r.get::<String, _>("id"),
        title: r.try_get::<Option<String>, _>("title").unwrap_or(None),
        description: sanitize_place_description(
            r.try_get::<Option<String>, _>("description")
                .unwrap_or(None)
                .as_deref(),
        ),
        image: sanitize_image_url(
            r.try_get::<Option<String>, _>("image")
                .unwrap_or(None)
                .as_deref(),
            content_origin,
        ),
        owner: r.try_get::<Option<String>, _>("owner").unwrap_or(None),
        positions: r.try_get::<Vec<String>, _>("positions").unwrap_or_default(),
        base_position: r.get::<String, _>("base_position"),
        contact_name: r
            .try_get::<Option<String>, _>("contact_name")
            .unwrap_or(None),
        contact_email: r
            .try_get::<Option<String>, _>("contact_email")
            .unwrap_or(None),
        content_rating: r
            .try_get::<Option<String>, _>("content_rating")
            .unwrap_or(None),
        disabled: r.try_get::<bool, _>("disabled").unwrap_or(false),
        disabled_at: ts(&r, "disabled_at"),
        disabled_reason: r
            .try_get::<Option<String>, _>("disabled_reason")
            .unwrap_or(None),
        created_at: ts(&r, "created_at"),
        updated_at: ts(&r, "updated_at"),
        favorites: r.try_get::<i32, _>("favorites").unwrap_or(0),
        likes: r.try_get::<i32, _>("likes").unwrap_or(0),
        dislikes: r.try_get::<i32, _>("dislikes").unwrap_or(0),
        categories: r
            .try_get::<Vec<String>, _>("categories")
            .unwrap_or_default(),
        highlighted: r.try_get::<bool, _>("highlighted").unwrap_or(false),
        highlighted_image: r
            .try_get::<Option<String>, _>("highlighted_image")
            .unwrap_or(None),
        ranking: r.try_get::<Option<f64>, _>("ranking").unwrap_or(None),
        exclude_from_ranking: r
            .try_get::<bool, _>("exclude_from_ranking")
            .unwrap_or(false),
        sdk: r.try_get::<Option<String>, _>("sdk").unwrap_or(None),
        creator_address: r
            .try_get::<Option<String>, _>("creator_address")
            .unwrap_or(None),
        world_id: r.try_get::<Option<String>, _>("world_id").unwrap_or(None),
        deployment_id: r
            .try_get::<Option<String>, _>("deployment_id")
            .unwrap_or(None),
        deployed_at: ts(&r, "deployed_at"),
        world: r.try_get::<bool, _>("world").unwrap_or(false),
        world_name: r.try_get::<Option<String>, _>("world_name").unwrap_or(None),
        is_private: r.try_get::<bool, _>("is_private").unwrap_or(false),
        show_in_places: r.try_get::<bool, _>("show_in_places").unwrap_or(true),
        single_player: r.try_get::<bool, _>("single_player").unwrap_or(false),
        skybox_time: r.try_get::<Option<f64>, _>("skybox_time").unwrap_or(None),
        settings_version: r
            .try_get::<Option<i32>, _>("settings_version")
            .unwrap_or(None),
        user_favorite,
        user_like,
        user_dislike,
        user_count: r.try_get::<Option<i32>, _>("user_count").unwrap_or(None),
        user_visits: r.try_get::<i32, _>("user_visits").unwrap_or(0),
        like_rate: r.try_get::<Option<f64>, _>("like_rate").unwrap_or(None),
        like_score: r.try_get::<Option<f64>, _>("like_score").unwrap_or(None),
        live: None,
        live_event_name: None,
        connected_addresses: None,
        realms_detail: None,
    }
}

#[cfg(test)]
pub(crate) fn sample_row() -> PlaceRow {
    PlaceRow {
        scene_place: false,
        id: "uuid-1".to_string(),
        title: Some("t".to_string()),
        description: None,
        image: None,
        owner: None,
        positions: vec!["1,2".to_string()],
        base_position: "1,2".to_string(),
        contact_name: None,
        contact_email: None,
        content_rating: None,
        disabled: false,
        disabled_at: None,
        disabled_reason: None,
        created_at: None,
        updated_at: None,
        favorites: 0,
        likes: 0,
        dislikes: 0,
        categories: vec![],
        highlighted: false,
        highlighted_image: None,
        ranking: None,
        exclude_from_ranking: false,
        sdk: None,
        creator_address: None,
        world_id: None,
        deployment_id: None,
        deployed_at: None,
        world: false,
        world_name: None,
        is_private: false,
        show_in_places: true,
        single_player: false,
        skybox_time: None,
        settings_version: None,
        user_favorite: false,
        user_like: false,
        user_dislike: false,
        user_count: Some(3),
        user_visits: 0,
        like_rate: None,
        like_score: None,
        live: None,
        live_event_name: None,
        connected_addresses: None,
        realms_detail: None,
    }
}

#[cfg(test)]
mod wire_tests {
    use super::*;

    #[test]
    fn base_place_omits_destination_only_fields_and_carries_read_path_fields() {
        let v = serde_json::to_value(sample_row()).unwrap();
        let obj = v.as_object().unwrap();
        for absent in [
            "featured",
            "featured_image",
            "live",
            "connected_addresses",
            "realms_detail",
        ] {
            assert!(
                !obj.contains_key(absent),
                "{absent} must be omitted on base Place"
            );
        }
        for key in [
            "highlighted_image",
            "sdk",
            "highlighted",
            "user_favorite",
            "user_like",
            "user_dislike",
            "user_count",
            "user_visits",
            "like_rate",
            "like_score",
            "creator_address",
            "world_name",
            "base_position",
            "positions",
            "deployment_id",
        ] {
            assert!(obj.contains_key(key), "{key} must be present on base Place");
        }
        for absent in [
            "is_private",
            "tags",
            "show_in_places",
            "single_player",
            "skybox_time",
            "settings_version",
        ] {
            assert!(
                !obj.contains_key(absent),
                "{absent} must not be serialized on base Place"
            );
        }
        assert!(obj["deployment_id"].is_null());

        let mut world = sample_row();
        world.world = true;
        world.settings_version = Some(3);
        let w = serde_json::to_value(WorldRow::from(world)).unwrap();
        assert_eq!(w["settings_version"], serde_json::json!(3));
        let w = serde_json::to_value(WorldRow::from(sample_row())).unwrap();
        assert!(
            w["settings_version"].is_null(),
            "a world serializes the counter, null when unset"
        );
    }

    #[test]
    fn realms_detail_is_gated_for_places_and_skipped_for_worlds() {
        let mut row = sample_row();
        row.user_count = Some(0);
        row.apply_realms_detail(None);
        let v = serde_json::to_value(&row).unwrap();
        assert!(!v.as_object().unwrap().contains_key("realms_detail"));
        row.apply_realms_detail(Some("main"));
        let v = serde_json::to_value(&row).unwrap();
        assert_eq!(v["realms_detail"], serde_json::json!([]));

        // this realm, with the row's headcount, when anyone is there
        let mut row = sample_row();
        row.user_count = Some(4);
        row.apply_realms_detail(Some("main"));
        let v = serde_json::to_value(&row).unwrap();
        assert_eq!(
            v["realms_detail"],
            serde_json::json!([{ "serverName": "main", "usersCount": 4 }])
        );

        let mut row = sample_row();
        row.world = true;
        row.user_count = Some(4);
        row.apply_realms_detail(Some("main"));
        let v = serde_json::to_value(&row).unwrap();
        assert!(!v.as_object().unwrap().contains_key("realms_detail"));
    }

    #[test]
    fn an_empty_room_and_an_unreachable_comms_service_read_differently() {
        let mut row = sample_row();
        row.connected_addresses = Some(Some(vec![]));
        row.live = Some(true);
        let v = serde_json::to_value(&row).unwrap();
        assert_eq!(v["connected_addresses"], serde_json::json!([]));
        assert_eq!(v["live"], serde_json::json!(true));

        let mut row = sample_row();
        row.connected_addresses = Some(None);
        let v = serde_json::to_value(&row).unwrap();
        assert!(v.as_object().unwrap().contains_key("connected_addresses"));
        assert_eq!(v["connected_addresses"], serde_json::Value::Null);
    }
}
