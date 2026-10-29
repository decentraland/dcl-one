//! The atlas renderer (tiny-skia, pure Rust): `map.png` viewports with the
//! on-sale / for-rent tint and the selection layers, and the two 512x512
//! minimaps the explorer's map reads flags from.

use std::collections::HashSet;

use tiny_skia::{Color, Paint, Pixmap, Rect, Transform};

use super::Grid;
use super::tiles::{Tile, TileRentalListing, TileType, coords_to_id};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Coord {
    pub x: i32,
    pub y: i32,
}

struct Viewport {
    nw: Coord,
    se: Coord,
}

fn get_viewport(width: u32, height: u32, center: Coord, size: u32) -> Viewport {
    let padding = 1i32;
    let dim_w = ((width as f64 / size as f64).ceil() as i32) + padding;
    let dim_h = ((height as f64 / size as f64).ceil() as i32) + padding;
    let nw = Coord {
        x: center.x - div_ceil(dim_w, 2),
        y: center.y + div_ceil(dim_h, 2),
    };
    let se = Coord {
        x: center.x + div_ceil(dim_w, 2),
        y: center.y - div_ceil(dim_h, 2),
    };
    Viewport { nw, se }
}

#[inline]
fn div_ceil(a: i32, b: i32) -> i32 {
    (a as f64 / b as f64).ceil() as i32
}

struct RenderTile {
    color: Color,
    top: bool,
    left: bool,
    top_left: bool,
    scale: Option<f64>,
}

fn parse_hex(hex: &str) -> Color {
    let h = hex.trim_start_matches('#');
    let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(0);
    let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(0);
    let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(0);
    Color::from_rgba8(r, g, b, 255)
}

fn type_color(t: TileType) -> Color {
    match t {
        TileType::District => parse_hex("#5054D4"),
        TileType::Plaza => parse_hex("#70AC76"),
        TileType::Road => parse_hex("#716C7A"),
        TileType::Owned => parse_hex("#3D3A46"),
        TileType::Unowned => parse_hex("#09080A"),
    }
}

fn is_order_expired(tile: &Tile, now_secs: i64) -> bool {
    tile.expires_at.is_some_and(|e| e <= now_secs)
}

fn is_rental_expired(rl: &TileRentalListing, now_ms: i64) -> bool {
    rl.expiration < now_ms
}

#[derive(Clone, Copy)]
pub struct MapView<'a> {
    pub width: u32,
    pub height: u32,
    pub size: u32,
    pub center: Coord,
    pub selected: &'a [Coord],
    pub show_on_sale: bool,
    pub show_on_rent: bool,
}

struct TileMetrics {
    size: f64,
    padding: f64,
    offset: f64,
}

pub fn render_png(grid: &Grid, view: &MapView) -> Result<Vec<u8>, String> {
    let MapView {
        width,
        height,
        size,
        center,
        selected,
        show_on_sale,
        show_on_rent,
    } = *view;
    let mut pixmap = Pixmap::new(width, height).ok_or("invalid pixmap dimensions")?;
    pixmap.fill(parse_hex("#18141a"));

    let vp = get_viewport(width, height, center, size);
    let now_secs = chrono::Utc::now().timestamp();
    let now_ms = chrono::Utc::now().timestamp_millis();

    let half_width = width as f64 / 2.0;
    let half_height = height as f64 / 2.0;
    let size_f = size as f64;
    // The gap between parcels not joined into one estate (top/left flags): at
    // size=6 a half pixel disappears without anti-aliasing and every parcel
    // ran together, hence the 1px line from size 5 up.
    let padding: f64 = if size < 5 {
        0.5
    } else if size < 12 {
        1.0
    } else if size < 18 {
        1.5
    } else {
        2.0
    };
    let metrics = TileMetrics {
        size: size_f,
        padding,
        offset: 1.0,
    };
    let selection: HashSet<String> = selected.iter().map(|c| coords_to_id(c.x, c.y)).collect();
    let mut paint = Paint {
        anti_alias: false,
        ..Default::default()
    };

    for layer_idx in 0..(if selection.is_empty() { 1 } else { 3 }) {
        for x in vp.nw.x..vp.se.x {
            for y in vp.se.y..vp.nw.y {
                let offset_x = (center.x - x) as f64 * size_f;
                let offset_y = (y - center.y) as f64 * size_f;
                let rt: Option<RenderTile> = match layer_idx {
                    0 => Some(match grid.tiles.get(&coords_to_id(x, y)) {
                        Some(tile) => {
                            let on_sale = show_on_sale
                                && tile.price.is_some()
                                && !is_order_expired(tile, now_secs);
                            let on_rent = show_on_rent
                                && tile
                                    .rental_listing
                                    .as_ref()
                                    .is_some_and(|rl| !is_rental_expired(rl, now_ms));
                            let color = if on_sale || on_rent {
                                parse_hex("#1FBCFF")
                            } else {
                                type_color(tile.tile_type)
                            };
                            RenderTile {
                                color,
                                top: tile.top,
                                left: tile.left,
                                top_left: tile.top_left,
                                scale: None,
                            }
                        }
                        None => {
                            let color = if (x + y) % 2 == 0 {
                                parse_hex("#110e13")
                            } else {
                                parse_hex("#0d0b0e")
                            };
                            RenderTile {
                                color,
                                top: false,
                                left: false,
                                top_left: false,
                                scale: None,
                            }
                        }
                    }),
                    1 => selection.contains(&coords_to_id(x, y)).then(|| RenderTile {
                        color: parse_hex("#ff0044"),
                        top: false,
                        left: false,
                        top_left: false,
                        scale: Some(1.4),
                    }),
                    _ => selection.contains(&coords_to_id(x, y)).then(|| RenderTile {
                        color: parse_hex("#ff9990"),
                        top: false,
                        left: false,
                        top_left: false,
                        scale: Some(1.2),
                    }),
                };
                let Some(rt) = rt else { continue };
                let half_size = match rt.scale {
                    Some(s) => (size_f * s) / 2.0,
                    None => size_f / 2.0,
                };
                let px = half_width - offset_x + half_size;
                let py = half_height - offset_y + half_size;
                paint.set_color(rt.color);
                draw_tile(&mut pixmap, &paint, px, py, &metrics, &rt);
            }
        }
    }
    pixmap.encode_png().map_err(|e| e.to_string())
}

fn draw_tile(
    pixmap: &mut Pixmap,
    paint: &Paint,
    x: f64,
    y: f64,
    metrics: &TileMetrics,
    rt: &RenderTile,
) {
    let TileMetrics {
        size,
        padding,
        offset,
    } = *metrics;
    let tile_size = match rt.scale {
        Some(s) => size * s,
        None => size,
    };
    if !rt.top && !rt.left {
        fill_rect(
            pixmap,
            paint,
            x - tile_size + padding,
            y - tile_size + padding,
            tile_size - padding,
            tile_size - padding,
        );
    } else if rt.top && rt.left && rt.top_left {
        fill_rect(
            pixmap,
            paint,
            x - tile_size - offset,
            y - tile_size - offset,
            tile_size + offset,
            tile_size + offset,
        );
    } else {
        if rt.left {
            fill_rect(
                pixmap,
                paint,
                x - tile_size - offset,
                y - tile_size + padding,
                tile_size + offset,
                tile_size - padding,
            );
        }
        if rt.top {
            fill_rect(
                pixmap,
                paint,
                x - tile_size + padding,
                y - tile_size - offset,
                tile_size - padding,
                tile_size + offset,
            );
        }
    }
}

fn fill_rect(pixmap: &mut Pixmap, paint: &Paint, x: f64, y: f64, w: f64, h: f64) {
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    if let Some(rect) = Rect::from_xywh(x as f32, y as f32, w as f32, h as f32) {
        pixmap.fill_rect(rect, paint, Transform::identity(), None);
    }
}

const MINIMAP_DIM: usize = 512;

fn minimap_pixmap(rgba: &[u8]) -> Result<Vec<u8>, String> {
    let mut pixmap =
        Pixmap::new(MINIMAP_DIM as u32, MINIMAP_DIM as u32).ok_or("invalid pixmap dimensions")?;
    pixmap.data_mut().copy_from_slice(rgba);
    pixmap.encode_png().map_err(|e| e.to_string())
}

fn minimap_pointer(x: i32, y: i32) -> Option<usize> {
    if !(-256..=255).contains(&x) || !(-256..=255).contains(&y) {
        return None;
    }
    let absolute_y = MINIMAP_DIM as i32 - (y + 256);
    if !(0..MINIMAP_DIM as i32).contains(&absolute_y) {
        return None;
    }
    Some(((x + 256 + absolute_y * MINIMAP_DIM as i32) * 4) as usize)
}

/// One pixel per parcel: R carries the top/left/topLeft flags, G the type.
pub fn render_minimap(grid: &Grid) -> Result<Vec<u8>, String> {
    let mut rgba = vec![0u8; MINIMAP_DIM * MINIMAP_DIM * 4];
    for tile in grid.tiles.values() {
        let mut flags_r: u8 = 0;
        if tile.top {
            flags_r |= 8;
        }
        if tile.left {
            flags_r |= 16;
        }
        if tile.top_left {
            flags_r |= 32;
        }
        let flags_g: u8 = match tile.tile_type {
            TileType::District => 32,
            TileType::Road => 64,
            TileType::Owned => 128,
            _ => 0,
        };
        let Some(pointer) = minimap_pointer(tile.x, tile.y) else {
            continue;
        };
        rgba[pointer] = flags_r;
        rgba[pointer + 1] = flags_g;
        rgba[pointer + 2] = 0;
        rgba[pointer + 3] = 255;
    }
    minimap_pixmap(&rgba)
}

/// One pixel per parcel carrying its estate's ordinal in RGB.
pub fn render_estate_minimap(grid: &Grid) -> Result<Vec<u8>, String> {
    let mut rgba = vec![0u8; MINIMAP_DIM * MINIMAP_DIM * 4];
    let mut order: Vec<String> = Vec::new();
    for tile in grid.tiles.values() {
        let mut index: u32 = 0;
        if let Some(estate_id) = &tile.estate_id {
            index = match order.iter().position(|e| e == estate_id) {
                Some(pos) => pos as u32,
                None => {
                    order.push(estate_id.clone());
                    order.len() as u32
                }
            };
        }
        let Some(pointer) = minimap_pointer(tile.x, tile.y) else {
            continue;
        };
        rgba[pointer] = ((index >> 16) & 0xff) as u8;
        rgba[pointer + 1] = ((index >> 8) & 0xff) as u8;
        rgba[pointer + 2] = (index & 0xff) as u8;
        rgba[pointer + 3] = 255;
    }
    minimap_pixmap(&rgba)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn grid() -> Grid {
        let mut tiles = HashMap::new();
        let tile: Tile = serde_json::from_value(serde_json::json!({
            "id": "0,0", "x": 0, "y": 0, "type": "plaza", "top": true, "left": true, "topLeft": true, "name": "Genesis Plaza", "estateId": "1164"
        }))
        .unwrap();
        tiles.insert("0,0".to_string(), tile);
        Grid::from_tiles(tiles, 1, None, chrono::Utc::now().naive_utc())
    }

    #[test]
    fn map_png_is_a_png_of_the_requested_size() {
        let png = render_png(
            &grid(),
            &MapView {
                width: 64,
                height: 64,
                size: 5,
                center: Coord { x: 0, y: 0 },
                selected: &[],
                show_on_sale: false,
                show_on_rent: false,
            },
        )
        .unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        let width = u32::from_be_bytes([png[16], png[17], png[18], png[19]]);
        let height = u32::from_be_bytes([png[20], png[21], png[22], png[23]]);
        assert_eq!((width, height), (64, 64));
        let selected = render_png(
            &grid(),
            &MapView {
                width: 100,
                height: 100,
                size: 20,
                center: Coord { x: 0, y: 0 },
                selected: &[Coord { x: 0, y: 0 }],
                show_on_sale: true,
                show_on_rent: true,
            },
        )
        .unwrap();
        assert_ne!(png, selected);
    }

    #[test]
    fn minimaps_are_512_square() {
        for png in [
            render_minimap(&grid()).unwrap(),
            render_estate_minimap(&grid()).unwrap(),
        ] {
            let width = u32::from_be_bytes([png[16], png[17], png[18], png[19]]);
            assert_eq!(width, 512);
        }
        assert_eq!(
            minimap_pointer(-256, 255),
            Some(MINIMAP_DIM * 4),
            "the top row is y=255, one row below the pixmap edge"
        );
        assert_eq!(
            minimap_pointer(255, -255),
            Some((MINIMAP_DIM * MINIMAP_DIM - 1) * 4),
            "the bottom row is y=-255"
        );
        assert_eq!(minimap_pointer(0, -256), None);
        assert_eq!(minimap_pointer(256, 0), None);
    }
}
