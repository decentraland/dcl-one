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
const MAX_SELECTED: usize = 512;

static RENDERS: LazyLock<Arc<tokio::sync::Semaphore>> =
    LazyLock::new(|| Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_RENDERS)));

type Flights = parking_lot::Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>;

/// One lock per PNG key being rendered, so concurrent misses render it once.
static RENDER_FLIGHTS: LazyLock<Flights> = LazyLock::new(Default::default);

fn render_flight(key: &str) -> Arc<tokio::sync::Mutex<()>> {
    let mut flights = RENDER_FLIGHTS.lock();
    flights.retain(|_, lock| Arc::strong_count(lock) > 1);
    flights.entry(key.to_string()).or_default().clone()
}

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

/// A centre outside Genesis City is moved to its nearest edge.
fn clamp_center(c: Coord) -> Coord {
    let (x0, y0, x1, y1) = crate::shared::parcel::WORLD_BOUNDS;
    Coord {
        x: i64::from(c.x).clamp(x0, x1) as i32,
        y: i64::from(c.y).clamp(y0, y1) as i32,
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
    render_limited(RENDERS.clone(), grid, render).await
}

async fn render_limited<F>(
    renders: Arc<tokio::sync::Semaphore>,
    grid: Arc<Grid>,
    render: F,
) -> Result<Vec<u8>, Box<Response>>
where
    F: FnOnce(&Grid) -> Result<Vec<u8>, Box<Response>> + Send + 'static,
{
    let Ok(Ok(permit)) = tokio::time::timeout(RENDER_QUEUE_WAIT, renders.acquire_owned()).await
    else {
        return Err(renderer_busy());
    };
    // The slot is held by the render itself, even if the request goes away.
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        render(&grid)
    })
    .await
    .unwrap_or_else(|e| Err(render_error(e.to_string())))
}

fn renderer_busy() -> Box<Response> {
    Box::new(
        (
            StatusCode::SERVICE_UNAVAILABLE,
            [
                (header::RETRY_AFTER, "5"),
                (header::CACHE_CONTROL, "no-store"),
            ],
            "map renderer busy",
        )
            .into_response(),
    )
}

/// The render cache's key of `key` on `grid`: a body rendered from one grid
/// is never served for another.
fn png_cache_key(grid: &Grid, key: &str) -> String {
    format!("{key}@{:016x}", grid.content_tag)
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
    // One grid for the validators, the cache key and the body, so a swap
    // cannot pair one grid's body with another's validators. Without a grid,
    // the envelope as it is: never a 304 for a stamp no grid carries.
    let Some(grid) = state.map.snapshot() else {
        return state.map.unavailable();
    };
    let last = grid.last_updated_at;
    let etag = http_cache::etag_for(last, &key);
    if let Some(r) = http_cache::not_modified_etag(headers, last, &etag, max_age, swr) {
        return r;
    }
    let key = png_cache_key(&grid, &key);
    let flight;
    let mut _rendering = None;
    if state.map.cached_png(&key).is_none() {
        flight = render_flight(&key);
        // Waiting behind another render of this key has its own deadline,
        // before the render queue's.
        match tokio::time::timeout(RENDER_QUEUE_WAIT, flight.lock()).await {
            Ok(guard) => _rendering = Some(guard),
            Err(_) => match state.map.cached_png(&key) {
                Some(bytes) => {
                    let mut resp = png_response(bytes);
                    http_cache::apply_etag(&mut resp, last, &etag, max_age, swr);
                    return resp;
                }
                None => return *renderer_busy(),
            },
        }
    }
    let bytes = match state.map.cached_png(&key) {
        Some(b) => b,
        None => {
            match render_blocking(grid, render).await {
                Ok(b) => {
                    let b = Arc::new(b);
                    state.map.store_png(key, b.clone());
                    b
                }
                Err(mut resp) => {
                    // A transient failure must not be cached downstream.
                    if resp.status().is_server_error() {
                        resp.headers_mut().insert(
                            header::CACHE_CONTROL,
                            axum::http::HeaderValue::from_static("no-store"),
                        );
                    } else if !resp.headers().contains_key(header::CACHE_CONTROL) {
                        // An answer with its own policy keeps it (the
                        // dissolved-estate redirect).
                        http_cache::apply(&mut resp, last, max_age, swr);
                    }
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
    let key = format!("map:{}", p.key_suffix());
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
    let key = format!("parcel:{},{}:{}", center.x, center.y, p.key_suffix());
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
    // The estate's number as the grid spells it, as the metadata routes
    // read it; what is not a number matches no estate.
    let estate_id = super::meta::canonical_estate_id(&estate_id).unwrap_or(estate_id);
    // An estate the grid holds no parcel of (dissolved, or never minted)
    // redirects to the dissolved-estate image before any conditional check,
    // as the atlas does; no grid falls through to the render path's 503.
    if let Some(grid) = state.map.snapshot()
        && select_estate_coords(&grid, &estate_id).is_empty()
    {
        return dissolved_estate_redirect(&state.settings.map_dissolved_estate_image_url);
    }
    let dissolved = state.settings.map_dissolved_estate_image_url.clone();
    let p = extract_params(&q);
    let key = format!("estate:{estate_id}:{}", p.key_suffix());
    cached_png_render(
        &state,
        &headers,
        key,
        http_cache::DEFAULT_MAX_AGE,
        http_cache::DEFAULT_SWR,
        move |grid| render_estate(grid, &estate_id, &p, &dissolved),
    )
    .await
}

/// The estate's parcels selected and centred; the dissolved-estate redirect
/// when `grid` holds none (it swapped since the handler's check).
fn render_estate(
    grid: &Grid,
    estate_id: &str,
    p: &Params,
    dissolved: &str,
) -> Result<Vec<u8>, Box<Response>> {
    let selected = select_estate_coords(grid, estate_id);
    if selected.is_empty() {
        return Err(Box::new(dissolved_estate_redirect(dissolved)));
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
}

/// The atlas's redirect cache policy (`image-http-cache.ts`).
const REDIRECT_CACHE_CONTROL: &str = "max-age=60, s-maxage=60, public";

fn dissolved_estate_redirect(location: &str) -> Response {
    (
        StatusCode::FOUND,
        [
            (header::LOCATION, location),
            (header::CACHE_CONTROL, REDIRECT_CACHE_CONTROL),
        ],
    )
        .into_response()
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
        // Versioned with the renderer so cached ETags of the old numbering lapse.
        "estatemap-v2".to_string(),
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
        let grid = Grid::from_tiles(tiles, 0, chrono::Utc::now().naive_utc());

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
        assert_eq!(p.center, Coord { x: 163, y: -150 }, "the nearest city edge");
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

    #[tokio::test]
    async fn an_abandoned_render_keeps_its_slot_until_it_finishes() {
        let grid = Arc::new(Grid::from_tiles(
            HashMap::new(),
            0,
            chrono::Utc::now().naive_utc(),
        ));
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let (finish_tx, finish_rx) = std::sync::mpsc::channel::<()>();
        let renders = Arc::new(tokio::sync::Semaphore::new(1));
        let request = tokio::spawn(render_limited(renders.clone(), grid, move |_| {
            started_tx.send(()).unwrap();
            finish_rx.recv().unwrap();
            Ok(vec![])
        }));
        tokio::task::spawn_blocking(move || started_rx.recv().unwrap())
            .await
            .unwrap();
        request.abort();
        let _ = request.await;
        assert_eq!(
            renders.available_permits(),
            0,
            "the aborted request did not free the render's slot"
        );
        finish_tx.send(()).unwrap();
        for _ in 0..200 {
            if renders.available_permits() == 1 {
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        panic!("the slot came back when the render ended");
    }

    #[test]
    fn concurrent_png_misses_share_one_render_lock() {
        let a = render_flight("png-key");
        assert!(Arc::ptr_eq(&a, &render_flight("png-key")));
        assert!(!Arc::ptr_eq(&a, &render_flight("png-other")));
    }

    fn small_grid() -> Grid {
        Grid::from_tiles(HashMap::new(), 1, chrono::Utc::now().naive_utc())
    }

    #[tokio::test(start_paused = true)]
    async fn a_waiter_past_its_deadline_answers_an_uncached_503() {
        let state = crate::api::tests_support::state_without_db().await;
        state.map.set(small_grid());
        let key = "busy-test".to_string();
        let flight = render_flight(&png_cache_key(&state.map.snapshot().unwrap(), &key));
        let _held = flight.lock().await;
        let resp = cached_png_render(&state, &HeaderMap::new(), key, 60, 60, |_| Ok(vec![1])).await;
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(resp.headers()[header::CACHE_CONTROL], "no-store");
        assert_eq!(resp.headers()[header::RETRY_AFTER], "5");
    }

    #[tokio::test(start_paused = true)]
    async fn a_waiter_past_its_deadline_serves_a_png_stored_meanwhile() {
        let state = crate::api::tests_support::state_without_db().await;
        state.map.set(small_grid());
        let key = "stored-meanwhile".to_string();
        let cache_key = png_cache_key(&state.map.snapshot().unwrap(), &key);
        let flight = render_flight(&cache_key);
        let held = flight.lock().await;
        let waiter = {
            let state = state.clone();
            let key = key.clone();
            tokio::spawn(async move {
                cached_png_render(&state, &HeaderMap::new(), key, 60, 60, |_| Ok(vec![9])).await
            })
        };
        tokio::task::yield_now().await;
        state.map.store_png(cache_key, Arc::new(vec![7]));
        let resp = waiter.await.unwrap();
        drop(held);
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn a_node_without_a_grid_answers_an_uncached_503() {
        let state = crate::api::tests_support::state_without_db().await;
        let resp = cached_png_render(&state, &HeaderMap::new(), "no-grid".into(), 60, 60, |_| {
            Ok(vec![1])
        })
        .await;
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(resp.headers()[header::CACHE_CONTROL], "no-store");
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["source"], "map-grid", "the unavailable envelope");
        assert!(body.get("configure").is_none(), "no source is vouched for");
    }

    #[tokio::test]
    async fn a_failed_render_is_not_cached_but_a_missing_estate_is() {
        let state = crate::api::tests_support::state_without_db().await;
        state.map.set(small_grid());
        let failed = cached_png_render(&state, &HeaderMap::new(), "fail".into(), 60, 60, |_| {
            Err(render_error("boom".into()))
        })
        .await;
        assert_eq!(failed.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(failed.headers()[header::CACHE_CONTROL], "no-store");
        let missing =
            cached_png_render(&state, &HeaderMap::new(), "missing".into(), 60, 60, |_| {
                Err(Box::new(StatusCode::NOT_FOUND.into_response()))
            })
            .await;
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);
        assert_ne!(missing.headers()[header::CACHE_CONTROL], "no-store");
    }

    /// atlas-server `handlers.ts` `toPngErrorResponse`: an estate with no
    /// tiles (dissolved, or never minted) answers 302 to the dissolved-estate
    /// image, even to a conditional request; no grid stays 503.
    #[tokio::test]
    async fn an_estate_without_tiles_redirects_to_the_dissolved_estate_image() {
        let state = crate::api::tests_support::state_without_db().await;
        let call = |id: &str, headers: HeaderMap| {
            estate_map_png(
                State(state.clone()),
                headers,
                Path(id.to_string()),
                Query(HashMap::new()),
            )
        };
        let unready = call("500", HeaderMap::new()).await;
        assert_eq!(unready.status(), StatusCode::SERVICE_UNAVAILABLE);

        let mut tiles = HashMap::new();
        let t = tile(1, 1, Some("500"), true);
        tiles.insert(t.id.clone(), t);
        state
            .map
            .set(Grid::from_tiles(tiles, 1, chrono::Utc::now().naive_utc()));
        let mut conditional = HeaderMap::new();
        conditional.insert(header::IF_NONE_MATCH, "*".parse().unwrap());
        for (id, headers) in [
            ("501", HeaderMap::new()),
            ("99999999", HeaderMap::new()),
            ("501", conditional),
        ] {
            let resp = call(id, headers).await;
            assert_eq!(resp.status(), StatusCode::FOUND, "{id}");
            assert_eq!(
                resp.headers()[header::LOCATION],
                state.settings.map_dissolved_estate_image_url.as_str()
            );
            assert_eq!(
                resp.headers()[header::CACHE_CONTROL],
                "max-age=60, s-maxage=60, public"
            );
        }
        let live = call("500", HeaderMap::new()).await;
        assert_eq!(
            live.status(),
            StatusCode::OK,
            "an estate with tiles renders"
        );
        // Another spelling of its number is the same estate; what is not a
        // number matches none.
        for id in ["0500", "+500"] {
            let resp = call(id, HeaderMap::new()).await;
            assert_eq!(resp.status(), StatusCode::OK, "{id}");
            assert_eq!(
                resp.headers()[header::ETAG],
                live.headers()[header::ETAG],
                "{id}"
            );
        }
        assert_eq!(
            call("abc", HeaderMap::new()).await.status(),
            StatusCode::FOUND
        );

        // The grid losing the estate between the handler's check and the
        // render answers the same redirect, headers included.
        let checked = call("501", HeaderMap::new()).await;
        let location = state.settings.map_dissolved_estate_image_url.clone();
        let raced = cached_png_render(
            &state,
            &HeaderMap::new(),
            "estate:501:raced".into(),
            http_cache::DEFAULT_MAX_AGE,
            http_cache::DEFAULT_SWR,
            move |grid| render_estate(grid, "501", &extract_params(&HashMap::new()), &location),
        )
        .await;
        assert_eq!(raced.status(), StatusCode::FOUND);
        assert_eq!(raced.headers(), checked.headers());
        assert!(!raced.headers().contains_key(header::LAST_MODIFIED));
    }
}
