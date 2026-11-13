//! `map.png` in its four selections (viewport, parcel, estate, none) and the
//! two minimaps, each rendered once per canonical key and served from the
//! response cache with the conditional headers.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use axum::Json;
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde_json::json;

use super::super::Shared;
use super::Grid;
use super::http_cache;
use super::render::{Coord, MapView, render_estate_minimap, render_minimap, render_png};

/// Renders run on the blocking pool, at most this many at once.
const MAX_CONCURRENT_RENDERS: usize = 2;
const RENDER_QUEUE_WAIT: Duration = Duration::from_secs(10);
/// A centre is clamped to this many parcels from the origin (the map is ~±150).
const MAX_CENTER_COORD: i32 = 300;
const MAX_SELECTED: usize = 512;

static RENDERS: LazyLock<tokio::sync::Semaphore> =
    LazyLock::new(|| tokio::sync::Semaphore::new(MAX_CONCURRENT_RENDERS));

/// Owned parcels first, then every parcel carrying the estate id.
pub fn select_estate_coords(grid: &Grid, estate_id: &str) -> Vec<Coord> {
    grid.estates_owned
        .get(estate_id)
        .filter(|v| !v.is_empty())
        .or_else(|| grid.estates_all.get(estate_id))
        .map(|v| v.iter().map(|&(x, y)| Coord { x, y }).collect())
        .unwrap_or_default()
}

struct Params {
    width: u32,
    height: u32,
    size: u32,
    center: Coord,
    show_on_sale: bool,
    show_on_rent: bool,
    selected: Vec<Coord>,
}

fn parse_clamped(q: &HashMap<String, String>, name: &str, default: i64, min: i64, max: i64) -> i64 {
    q.get(name)
        .and_then(|s| s.trim().parse::<i64>().ok())
        .unwrap_or(default)
        .clamp(min, max)
}

fn parse_coord(s: &str) -> Option<Coord> {
    let mut parts = s.split(',');
    let x = parts.next()?.trim().parse::<i32>().ok()?;
    let y = parts.next()?.trim().parse::<i32>().ok()?;
    Some(Coord { x, y })
}

fn extract_params(q: &HashMap<String, String>) -> Params {
    Params {
        width: parse_clamped(q, "width", 1024, 100, 4096) as u32,
        height: parse_clamped(q, "height", 1024, 100, 4096) as u32,
        size: parse_clamped(q, "size", 20, 5, 50) as u32,
        center: q
            .get("center")
            .and_then(|s| parse_coord(s))
            .map(clamp_center)
            .unwrap_or(Coord { x: 0, y: 0 }),
        show_on_sale: q.get("on-sale").is_some_and(|s| s == "true"),
        show_on_rent: q.get("listed-for-rent").is_some_and(|s| s == "true"),
        selected: q
            .get("selected")
            .map(|s| {
                let mut sel: Vec<Coord> = s
                    .split(';')
                    .filter_map(parse_coord)
                    .take(MAX_SELECTED)
                    .collect();
                sel.sort_unstable_by_key(|c| (c.x, c.y));
                sel.dedup();
                sel
            })
            .unwrap_or_default(),
    }
}

fn clamp_center(c: Coord) -> Coord {
    Coord {
        x: c.x.clamp(-MAX_CENTER_COORD, MAX_CENTER_COORD),
        y: c.y.clamp(-MAX_CENTER_COORD, MAX_CENTER_COORD),
    }
}

impl Params {
    fn key_suffix(&self) -> String {
        let mut sel: Vec<(i32, i32)> = self.selected.iter().map(|c| (c.x, c.y)).collect();
        sel.sort_unstable();
        let sel: Vec<String> = sel.iter().map(|(x, y)| format!("{x},{y}")).collect();
        format!(
            "w{}h{}s{}c{},{}os{}or{}sel{}",
            self.width,
            self.height,
            self.size,
            self.center.x,
            self.center.y,
            self.show_on_sale as u8,
            self.show_on_rent as u8,
            sel.join(";")
        )
    }

    fn view<'a>(&self, center: Coord, selected: &'a [Coord]) -> MapView<'a> {
        MapView {
            width: self.width,
            height: self.height,
            size: self.size,
            center,
            selected,
            show_on_sale: self.show_on_sale,
            show_on_rent: self.show_on_rent,
        }
    }
}

fn png_response(bytes: Arc<Vec<u8>>) -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "image/png")],
        Body::from(bytes.as_ref().clone()),
    )
        .into_response()
}

fn render_error(e: String) -> Box<Response> {
    Box::new(
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": e })),
        )
            .into_response(),
    )
}

/// Runs `render` on the blocking pool behind the render limit; a request that
/// waits too long for a slot answers 503.
async fn render_blocking<F>(grid: Arc<Grid>, render: F) -> Result<Vec<u8>, Box<Response>>
where
    F: FnOnce(&Grid) -> Result<Vec<u8>, Box<Response>> + Send + 'static,
{
    let busy = || {
        Box::new(
            (
                StatusCode::SERVICE_UNAVAILABLE,
                [(header::RETRY_AFTER, "5")],
                "map renderer busy",
            )
                .into_response(),
        )
    };
    let Ok(Ok(_permit)) = tokio::time::timeout(RENDER_QUEUE_WAIT, RENDERS.acquire()).await else {
        return Err(busy());
    };
    tokio::task::spawn_blocking(move || render(&grid))
        .await
        .unwrap_or_else(|e| Err(render_error(e.to_string())))
}

async fn cached_png_render<F>(
    state: &Shared,
    headers: &HeaderMap,
    key: String,
    max_age: u32,
    swr: u32,
    render: F,
) -> Response
where
    F: FnOnce(&Grid) -> Result<Vec<u8>, Box<Response>> + Send + 'static,
{
    let last = state.map.last_updated_at();
    let etag = http_cache::etag_for(last, &key);
    if let Some(r) = http_cache::not_modified_etag(headers, last, &etag, max_age, swr) {
        return r;
    }
    let bytes = match state.map.cached_png(&key) {
        Some(b) => b,
        None => {
            let Some(grid) = state.map.snapshot() else {
                let mut resp = (StatusCode::SERVICE_UNAVAILABLE, "Not ready").into_response();
                http_cache::apply(&mut resp, last, max_age, swr);
                return resp;
            };
            match render_blocking(grid, render).await {
                Ok(b) => {
                    let b = Arc::new(b);
                    state.map.store_png(key, b.clone());
                    b
                }
                Err(mut resp) => {
                    http_cache::apply(&mut resp, last, max_age, swr);
                    return *resp;
                }
            }
        }
    };
    let mut resp = png_response(bytes);
    http_cache::apply_etag(&mut resp, last, &etag, max_age, swr);
    resp
}

pub async fn map_png(
    State(state): State<Shared>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    let p = extract_params(&q);
    let key = format!("map:{}:{}", state.map.content_tag(), p.key_suffix());
    cached_png_render(
        &state,
        &headers,
        key,
        http_cache::DEFAULT_MAX_AGE,
        http_cache::DEFAULT_SWR,
        move |grid| render_png(grid, &p.view(p.center, &p.selected)).map_err(render_error),
    )
    .await
}

pub async fn parcel_map_png(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path((x, y)): Path<(String, String)>,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    let p = extract_params(&q);
    let center = clamp_center(Coord {
        x: x.parse().unwrap_or(0),
        y: y.parse().unwrap_or(0),
    });
    let selected = vec![center];
    let key = format!(
        "parcel:{},{}:{}:{}",
        center.x,
        center.y,
        state.map.content_tag(),
        p.key_suffix()
    );
    cached_png_render(
        &state,
        &headers,
        key,
        http_cache::DEFAULT_MAX_AGE,
        http_cache::DEFAULT_SWR,
        move |grid| render_png(grid, &p.view(center, &selected)).map_err(render_error),
    )
    .await
}

pub async fn estate_map_png(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(estate_id): Path<String>,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    let p = extract_params(&q);
    let key = format!(
        "estate:{estate_id}:{}:{}",
        state.map.content_tag(),
        p.key_suffix()
    );
    cached_png_render(
        &state,
        &headers,
        key,
        http_cache::DEFAULT_MAX_AGE,
        http_cache::DEFAULT_SWR,
        move |grid| {
            let selected = select_estate_coords(grid, &estate_id);
            if selected.is_empty() {
                return Err(Box::new(StatusCode::NOT_FOUND.into_response()));
            }
            let mut xs: Vec<i32> = selected.iter().map(|c| c.x).collect();
            let mut ys: Vec<i32> = selected.iter().map(|c| c.y).collect();
            xs.sort_unstable();
            ys.sort_unstable();
            let center = Coord {
                x: xs[xs.len() / 2],
                y: ys[ys.len() / 2],
            };
            render_png(grid, &p.view(center, &selected)).map_err(render_error)
        },
    )
    .await
}

pub async fn minimap_png(State(state): State<Shared>, headers: HeaderMap) -> Response {
    cached_png_render(
        &state,
        &headers,
        "minimap".to_string(),
        http_cache::MINIMAP_MAX_AGE,
        http_cache::MINIMAP_SWR,
        |grid| render_minimap(grid).map_err(render_error),
    )
    .await
}

pub async fn estate_minimap_png(State(state): State<Shared>, headers: HeaderMap) -> Response {
    cached_png_render(
        &state,
        &headers,
        "estatemap".to_string(),
        http_cache::MINIMAP_MAX_AGE,
        http_cache::MINIMAP_SWR,
        |grid| render_estate_minimap(grid).map_err(render_error),
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::super::tiles::{Tile, TileType};
    use super::*;

    fn tile(x: i32, y: i32, estate_id: Option<&str>, owned: bool) -> Tile {
        Tile {
            id: format!("{x},{y}"),
            x,
            y,
            nft_id: None,
            tile_type: if owned {
                TileType::Owned
            } else {
                TileType::Unowned
            },
            top: false,
            left: false,
            top_left: false,
            updated_at: 0,
            name: None,
            owner: None,
            estate_id: estate_id.map(|s| s.to_string()),
            token_id: None,
            price: None,
            expires_at: None,
            rental_listing: None,
        }
    }

    #[test]
    fn estate_lookup_prefers_owned_parcels_and_falls_back_to_all() {
        let mut tiles = HashMap::new();
        for i in 0..3i32 {
            let t = tile(i, 5, Some("99"), false);
            tiles.insert(t.id.clone(), t);
        }
        let t = tile(9, 9, Some("77"), true);
        tiles.insert(t.id.clone(), t);
        let t = tile(8, 9, Some("77"), false);
        tiles.insert(t.id.clone(), t);
        let grid = Grid::from_tiles(tiles, 0, None, chrono::Utc::now().naive_utc());

        let mut got: Vec<(i32, i32)> = select_estate_coords(&grid, "99")
            .iter()
            .map(|c| (c.x, c.y))
            .collect();
        got.sort_unstable();
        assert_eq!(got, vec![(0, 5), (1, 5), (2, 5)]);
        assert_eq!(
            select_estate_coords(&grid, "77"),
            vec![Coord { x: 9, y: 9 }],
            "owned parcels win"
        );
        assert!(select_estate_coords(&grid, "does-not-exist").is_empty());
    }

    #[test]
    fn params_are_clamped_and_the_key_is_order_insensitive() {
        let q: HashMap<String, String> = [
            ("width", "9999"),
            ("height", "1"),
            ("size", "5"),
            ("center", "3,4"),
            ("selected", "1,1;0,0"),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        let p = extract_params(&q);
        assert_eq!((p.width, p.height, p.size), (4096, 100, 5));
        assert_eq!(p.center, Coord { x: 3, y: 4 });
        let q2: HashMap<String, String> = q
            .iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    if k == "selected" {
                        "0,0;1,1".into()
                    } else {
                        v.clone()
                    },
                )
            })
            .collect();
        assert_eq!(p.key_suffix(), extract_params(&q2).key_suffix());
    }

    #[test]
    fn centre_and_selection_are_bounded() {
        let many: Vec<String> = (0..2000).map(|i| format!("{i},0")).collect();
        let q: HashMap<String, String> = [
            ("center".to_string(), "99999,-99999".to_string()),
            (
                "selected".to_string(),
                format!("1,1;1,1;{}", many.join(";")),
            ),
        ]
        .into();
        let p = extract_params(&q);
        assert_eq!(
            p.center,
            Coord {
                x: MAX_CENTER_COORD,
                y: -MAX_CENTER_COORD
            }
        );
        assert!(p.selected.len() <= MAX_SELECTED, "{}", p.selected.len());
        assert_eq!(
            p.selected
                .iter()
                .filter(|c| **c == Coord { x: 1, y: 1 })
                .count(),
            1,
            "duplicates collapse"
        );
        let q: HashMap<String, String> = [("center".to_string(), "-20,35".to_string())].into();
        assert_eq!(extract_params(&q).center, Coord { x: -20, y: 35 });
    }

    #[tokio::test]
    async fn renders_run_off_the_async_workers() {
        let grid = Arc::new(Grid::from_tiles(
            HashMap::new(),
            0,
            None,
            chrono::Utc::now().naive_utc(),
        ));
        let worker = std::thread::current().id();
        let out = render_blocking(grid, move |_| {
            assert_ne!(std::thread::current().id(), worker);
            Ok(vec![1])
        })
        .await;
        assert_eq!(out.ok(), Some(vec![1]));
    }
}
