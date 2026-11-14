//! The place/world wire rows and the SQL that projects `explore.place_indexed`
//! onto them. Every raw jsonb key a third party wrote is read through a
//! `jsonb_typeof` guard, at write time into the stored `*_num`, `*_ts` and
//! `textsearch` columns (schemas/10_explore.sql) or here (query.rs): one
//! malformed value must cost one field, never the whole listing.

use std::sync::LazyLock;

use chrono::{DateTime, NaiveDateTime, Utc};
use serde::Serialize;
use sqlx::Row;

use super::query::{
    EXCLUDE_FROM_RANKING_SQL, IS_PRIVATE_SQL, SHOW_IN_PLACES_SQL, SINGLE_PLAYER_SQL,
    USER_DISLIKE_SQL, USER_FAVORITE_SQL, USER_LIKE_SQL, textsearch_column_sql,
};
use crate::sanitize::{
    ContentOrigin, sanitize_image_url, sanitize_place_description, strip_all_markup_opt,
};

pub(super) fn place_columns() -> &'static str {
    static SQL: LazyLock<String> = LazyLock::new(|| {
        let textsearch = textsearch_column_sql();
        format!(
            r#"
    id, title, description, raw->>'image' AS image,
    LOWER(COALESCE(raw->>'owner', creator_address)) AS owner,
    creator_address,
    COALESCE((SELECT array_agg(p::text) FROM jsonb_array_elements_text(raw->'positions') p), ARRAY[]::text[]) AS positions,
    base_position,
    raw->>'contact_name' AS contact_name,
    raw->>'contact_email' AS contact_email,
    content_rating,
    disabled,
    disabled_at_ts AS disabled_at,
    raw->>'disabled_reason' AS disabled_reason,
    created_at_ts AS created_at,
    updated_at_ts AS updated_at,
    favorites + COALESCE((SELECT ic.favorites FROM explore.place_interaction_counts ic WHERE ic.entity_id = place_indexed.id), 0) AS favorites,
    COALESCE((SELECT CASE WHEN ic.imported > 0 THEN ic.likes ELSE place_indexed.likes + ic.likes END FROM explore.place_interaction_counts ic WHERE ic.entity_id = place_indexed.id), likes) AS likes,
    COALESCE((SELECT CASE WHEN ic.imported > 0 THEN ic.dislikes ELSE place_indexed.dislikes + ic.dislikes END FROM explore.place_interaction_counts ic WHERE ic.entity_id = place_indexed.id), dislikes) AS dislikes,
    categories,
    highlighted,
    raw->>'highlighted_image' AS highlighted_image,
    ranking_num AS ranking,
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
    skybox_time_num AS skybox_time,
    {USER_FAVORITE_SQL} AS user_favorite,
    {USER_LIKE_SQL} AS user_like,
    {USER_DISLIKE_SQL} AS user_dislike,
    user_count_num AS user_count,
    COALESCE(user_visits_num, 0) AS user_visits,
    settings_version_num AS settings_version,
    COALESCE((SELECT CASE WHEN ic.imported > 0 THEN ic.like_rate ELSE COALESCE(ic.like_rate, like_rate_num) END FROM explore.place_interaction_counts ic WHERE ic.entity_id = place_indexed.id), CASE WHEN EXISTS (SELECT 1 FROM explore.place_interaction_counts ic WHERE ic.entity_id = place_indexed.id AND ic.imported > 0) THEN NULL ELSE like_rate_num END) AS like_rate,
    COALESCE((SELECT CASE WHEN ic.imported > 0 THEN ic.like_score ELSE COALESCE(ic.like_score, like_score_num) END FROM explore.place_interaction_counts ic WHERE ic.entity_id = place_indexed.id), CASE WHEN EXISTS (SELECT 1 FROM explore.place_interaction_counts ic WHERE ic.entity_id = place_indexed.id AND ic.imported > 0) THEN NULL ELSE like_score_num END) AS like_score,
    {textsearch} AS textsearch
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
    #[serde(serialize_with = "crate::json_date::serialize_opt")]
    pub disabled_at: Option<DateTime<Utc>>,
    pub disabled_reason: Option<String>,
    #[cfg_attr(feature = "ts", ts(type = "string | null"))]
    #[serde(serialize_with = "crate::json_date::serialize_opt")]
    pub created_at: Option<DateTime<Utc>>,
    #[cfg_attr(feature = "ts", ts(type = "string | null"))]
    #[serde(serialize_with = "crate::json_date::serialize_opt")]
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
    #[serde(serialize_with = "crate::json_date::serialize_opt")]
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
    /// The serialized `textsearch` tsvector of upstream's places table (`p.*`,
    /// places `Place/types.ts:64`): the mirrored value when upstream sent one,
    /// otherwise the same vector computed here (`query::textsearch_sql`).
    pub textsearch: Option<String>,
    /// `ts_rank_cd(textsearch, query)`, selected only when `search` is set
    /// (places `Place/model.ts:305,323-328`).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub rank: Option<f32>,

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
    #[serde(serialize_with = "crate::json_date::serialize_opt")]
    pub created_at: Option<DateTime<Utc>>,
    #[cfg_attr(feature = "ts", ts(type = "string | null"))]
    #[serde(serialize_with = "crate::json_date::serialize_opt")]
    pub updated_at: Option<DateTime<Utc>>,
    pub disabled: bool,
    #[cfg_attr(feature = "ts", ts(type = "string | null"))]
    #[serde(serialize_with = "crate::json_date::serialize_opt")]
    pub disabled_at: Option<DateTime<Utc>>,
    pub base_position: String,
    pub contact_name: Option<String>,
    #[cfg_attr(feature = "ts", ts(type = "string | null"))]
    #[serde(serialize_with = "crate::json_date::serialize_opt")]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub rank: Option<f32>,
}

impl PlaceRow {
    /// A row of upstream's `worlds` table rather than of its `places` table:
    /// the catalogue keys a world by its lowercased name (places
    /// `World/model.ts:466-469`; `derive.rs` `UPSERT_LOCAL_WORLDS`), while a
    /// world scene keeps its own id and names its world in `world_name`.
    pub fn is_world_catalogue_row(&self) -> bool {
        self.world
            && !self.scene_place
            && self
                .world_name
                .as_deref()
                .is_some_and(|name| name.to_lowercase() == self.id)
    }

    /// The constants upstream selects for a world row in place of the places
    /// columns it lacks (`World/model.ts:207-215`, `Destination/model.ts:39-49`):
    /// `'0,0' as base_position`, `'{}' as positions`.
    pub fn synthesize_world_fields(&mut self) {
        if self.is_world_catalogue_row() {
            self.base_position = "0,0".to_string();
            self.positions.clear();
        }
    }
}

impl From<PlaceRow> for WorldRow {
    fn from(mut p: PlaceRow) -> Self {
        p.synthesize_world_fields();
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
            rank: p.rank,
        }
    }
}

/// The rows that lead `order_by=most_active`: on `GET /api/places` the base
/// positions with players (`Place/model.ts:291-303,630`); without a presence
/// snapshot, there and on the destinations, the mirrored headcount.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ActiveRows {
    #[default]
    All,
    Live(Vec<String>),
    Mirrored,
}

#[derive(Debug, Default, Clone)]
pub struct PlaceListFilters {
    pub limit: i64,
    pub offset: i64,
    pub positions: Vec<String>,
    /// World names matched exactly after lowercasing (`/api/places?names=`,
    /// `/api/worlds?names=`, `/api/destinations?world_names=`).
    pub names: Vec<String>,
    /// `/api/destinations?names=` matches a substring (`World/model.ts:121-130`).
    pub partial_names: Vec<String>,
    /// `/api/map/places?names=`: `world_name IN (...)` verbatim, case-sensitive
    /// (`Place/model.ts:1348-1351`).
    pub exact_world_names: Vec<String>,
    pub categories: Vec<String>,
    pub only_highlighted: bool,
    pub only_excluded_from_ranking: bool,
    pub search: Option<String>,
    pub creator_address: Option<String>,
    pub sdk: Option<String>,
    pub order_by: PlaceOrderBy,
    pub order_desc: bool,
    pub ids: Vec<String>,
    /// Upstream's `worlds` table rows only (`PlaceRow::is_world_catalogue_row`).
    pub only_worlds: bool,
    /// Genesis places only (`world is false`).
    pub only_places: bool,
    /// Upstream's `places` table: genesis places and world scenes, never a
    /// world catalogue row (`/api/places` once `names`, `ids` or
    /// `only_highlighted` lift `world is false`, `Place/model.ts:150-156`;
    /// `/api/map/places`, `:1331-1354`).
    pub places_table: bool,
    /// `/api/map` lists categorized places only (upstream's
    /// `array_length(p.categories, 1) > 0`, applied to the count as well).
    pub only_categorized: bool,
    pub operated_positions: Vec<String>,
    pub owner_filtered: bool,
    pub owner: Option<String>,
    pub active: ActiveRows,
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

    /// Upstream's `oneOf(options.order_by, [...]) ?? default` (`World/model.ts:351-355`,
    /// `Place/model.ts:1150-1156,1319-1325`, `Destination/model.ts:189-195`):
    /// a value outside the route's own list is the route default, never a
    /// column another route knows.
    pub fn parse_allowed(s: Option<&str>, allowed: &[&str], default: Self) -> Self {
        match s {
            Some(value) if allowed.contains(&value) => Self::parse(Some(value)),
            _ => default,
        }
    }
    /// The stored read of the raw key (schemas/10_explore.sql, both legs):
    /// the `most_active` filter and the order of every value but the like
    /// score, which `order_expr` renders.
    pub(super) fn column(self) -> &'static str {
        match self {
            Self::LikeScore => "like_score_num",
            Self::UpdatedAt => "updated_at_ts",
            Self::CreatedAt => "created_at_ts",
            Self::UserVisits => "COALESCE(user_visits_num, 0)",
            Self::MostActive => "COALESCE(user_count_num, 0)",
        }
    }

    /// What a list orders on: the value the row answers. The like score is
    /// `query::LIKE_SCORE_ORDER_SQL` (this node's recount, else the mirrored
    /// value, as `place_columns` serves it), read from the `recount` alias
    /// `query::like_score_recount_join` puts beside the row; the rest is the
    /// stored column.
    pub(super) fn order_expr(self) -> &'static str {
        match self {
            Self::LikeScore => super::query::LIKE_SCORE_ORDER_SQL,
            other => other.column(),
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
    pub disabled_reason: Option<String>,
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
        title: strip_all_markup_opt(r.try_get::<Option<String>, _>("title").unwrap_or(None)),
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
        contact_name: strip_all_markup_opt(
            r.try_get::<Option<String>, _>("contact_name")
                .unwrap_or(None),
        ),
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
        textsearch: r.try_get::<Option<String>, _>("textsearch").unwrap_or(None),
        rank: r.try_get::<Option<f32>, _>("rank").unwrap_or(None),
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
        textsearch: None,
        rank: None,
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
