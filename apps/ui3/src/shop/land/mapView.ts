// from: react-tile-map@0.4.1:src/render/map.ts, src/render/tile.ts (repository decentraland/tile-map@22670113)
// react-tile-map is under the ISC license (the `license` field of its package.json). The package has
// no LICENSE file and no copyright notice.
// The LAND map's geometry and its canvas painter, with no React: the view (the parcel at the middle of
// the shell and a zoom), pointer to parcel and back, zoom about a point, popup placement, and the
// layer functions drawn parcel by parcel. From react-tile-map (the canvas under decentraland-ui's
// Atlas): the layer contract (`MapLayer`, `MapLayerTile`) and, in `drawLayers`, the order of the
// layers, the gap on the left and top of a parcel unless it is connected there, and the gap width for
// each parcel size. Not from it: the view math, the popup placement, edges on device pixels, no gap
// under 4 px per parcel, `scale` about the parcel's middle, the mark of a parcel (`mark`, `edges`),
// the layer of marks alone for a map over the atlas image (`markLayer`).

import { GRID_PARCELS } from "../../explorer/components/MapSurface";
import type { Coord, LandTile, TileKind, Tiles } from "./tiles";

/** The first and the last parcel of the square on both axes. */
export const GRID_MIN = -170;
export const GRID_MAX = GRID_MIN + GRID_PARCELS - 1;
/** The parcel coordinate at the middle of the square: the corner between parcels -1 and 0. */
export const GRID_MID = GRID_MIN + GRID_PARCELS / 2 - 0.5;

/** The parcel coordinate at the middle of the shell (a parcel's own coordinate is its middle) and the zoom. */
export type MapView = { x: number; y: number; zoom: number };
export type MapSize = { w: number; h: number };
export type MapPoint = { x: number; y: number };
/** What the map shows: its centre, its zoom and the parcels at its top left and bottom right corners. */
export type MapViewport = { center: Coord; zoom: number; nw: Coord; se: Coord };

/**
 * One parcel of a layer: a CSS colour (a `var(--name)` reference is resolved on the map) and how it joins its neighbours.
 * `mark` is a second colour that marks the parcel: an outline on the sides in `edges` (all sides when absent), or the
 * colour of the whole parcel when the parcel is too small for an outline (`markLine`).
 */
export type MapLayerTile = { color: string; top?: boolean; left?: boolean; topLeft?: boolean; scale?: number; mark?: string; edges?: number };
/** A layer: the tile to draw at a parcel, or nothing. Called for every visible parcel on every draw. */
export type MapLayer = (x: number, y: number) => MapLayerTile | null | undefined;

const clamp = (v: number, lo: number, hi: number) => Math.max(lo, Math.min(hi, v));
const clampParcel = (v: number) => clamp(v, GRID_MIN, GRID_MAX) + 0;

/** Pixels per parcel: at zoom 1 the square of 340 parcels fills the shell's smaller side. */
export const parcelPx = (size: MapSize, zoom: number) => (Math.min(size.w, size.h) * zoom) / GRID_PARCELS;

/** The view inside its limits: the zoom range, and a centre that never shows more outside the square than it must. */
export function clampView(view: MapView, size: MapSize, minZoom: number, maxZoom: number): MapView {
  const zoom = clamp(view.zoom, minZoom, maxZoom);
  const s = parcelPx(size, zoom);
  const spanX = s > 0 ? Math.max(0, (GRID_PARCELS * s - size.w) / 2) / s : 0;
  const spanY = s > 0 ? Math.max(0, (GRID_PARCELS * s - size.h) / 2) / s : 0;
  const x = clamp(view.x, GRID_MID - spanX, GRID_MID + spanX);
  const y = clamp(view.y, GRID_MID - spanY, GRID_MID + spanY);
  return x === view.x && y === view.y && zoom === view.zoom ? view : { x, y, zoom };
}

/** The offset of the square's middle from the shell's middle, in pixels. */
export function stagePan(view: MapView, size: MapSize): MapPoint {
  const s = parcelPx(size, view.zoom);
  return { x: -(view.x - GRID_MID) * s + 0, y: (view.y - GRID_MID) * s + 0 };
}

/** The parcel under a point of the shell; null outside the square. */
export function pointToCoord(view: MapView, size: MapSize, p: MapPoint): Coord | null {
  const s = parcelPx(size, view.zoom);
  if (!(s > 0)) return null;
  const x = Math.floor(view.x + (p.x - size.w / 2) / s + 0.5);
  const y = Math.ceil(view.y - (p.y - size.h / 2) / s - 0.5);
  if (x < GRID_MIN || x > GRID_MAX || y < GRID_MIN || y > GRID_MAX) return null;
  return { x: x + 0, y: y + 0 };
}

/** The middle of a parcel, as a point of the shell. */
export function coordToPoint(view: MapView, size: MapSize, c: Coord): MapPoint {
  const s = parcelPx(size, view.zoom);
  return { x: size.w / 2 + (c.x - view.x) * s, y: size.h / 2 - (c.y - view.y) * s };
}

export function viewportOf(view: MapView, size: MapSize): MapViewport {
  const s = parcelPx(size, view.zoom);
  const halfW = s > 0 ? size.w / 2 / s : 0;
  const halfH = s > 0 ? size.h / 2 / s : 0;
  return {
    center: { x: view.x, y: view.y },
    zoom: view.zoom,
    nw: { x: clampParcel(Math.floor(view.x - halfW + 0.5)), y: clampParcel(Math.ceil(view.y + halfH - 0.5)) },
    se: { x: clampParcel(Math.floor(view.x + halfW + 0.5)), y: clampParcel(Math.ceil(view.y - halfH - 0.5)) },
  };
}

/** A new zoom that keeps the parcel under `anchor` (the shell's middle when absent) where it is. */
export function zoomAt(view: MapView, size: MapSize, zoom: number, anchor?: MapPoint): MapView {
  const s0 = parcelPx(size, view.zoom);
  const s1 = parcelPx(size, zoom);
  if (!anchor || !(s0 > 0) || !(s1 > 0)) return { ...view, zoom };
  const dx = anchor.x - size.w / 2;
  const dy = anchor.y - size.h / 2;
  return { x: view.x + dx / s0 - dx / s1, y: view.y - dy / s0 + dy / s1, zoom };
}

/** The view moved by the least that shows a parcel whole, `margin` parcels from the edge. */
export function keepInView(view: MapView, size: MapSize, c: Coord, margin = 1): MapView {
  const s = parcelPx(size, view.zoom);
  if (!(s > 0)) return view;
  const halfW = Math.max(0, size.w / 2 / s - 0.5 - margin);
  const halfH = Math.max(0, size.h / 2 / s - 0.5 - margin);
  const x = clamp(view.x, c.x - halfW, c.x + halfW);
  const y = clamp(view.y, c.y - halfH, c.y + halfH);
  return x === view.x && y === view.y ? view : { ...view, x, y };
}

export type PopupPlace = { left?: number; right?: number; top?: number; bottom?: number };
const POPUP_EDGE = 8;

/**
 * Where a popup goes for a point of the shell: below and right of it, on the other side when that
 * side has no room, and never past an edge. A flipped popup is anchored by its far edge (`right`,
 * `bottom`), so its real size does not have to be measured.
 */
export function placePopup(p: MapPoint, shell: MapSize, popup: MapSize = { w: 240, h: 120 }): PopupPlace {
  const roomX = Math.max(POPUP_EDGE, shell.w - popup.w - POPUP_EDGE);
  const roomY = Math.max(POPUP_EDGE, shell.h - popup.h - POPUP_EDGE);
  const out: PopupPlace = {};
  if (p.x > shell.w - (popup.w + 20) && p.x > shell.w / 2) out.right = clamp(shell.w - (p.x - 10), POPUP_EDGE, roomX);
  else out.left = clamp(p.x + 14, POPUP_EDGE, roomX);
  if (p.y > shell.h - (popup.h + 20) && p.y > shell.h / 2) out.bottom = clamp(shell.h - (p.y - 10), POPUP_EDGE, roomY);
  else out.top = clamp(p.y + 14, POPUP_EDGE, roomY);
  return out;
}

/** A layer colour as the canvas takes it: `var(--name, fallback)` looked up, anything else as written. Empty when unknown. */
export function cssColor(value: string, lookup: (name: string) => string, depth = 0): string {
  const v = value.trim();
  if (!v.startsWith("var(") || depth > 8) return v;
  const inner = v.slice(4, v.lastIndexOf(")"));
  const comma = inner.indexOf(",");
  const name = (comma < 0 ? inner : inner.slice(0, comma)).trim();
  const fallback = comma < 0 ? "" : inner.slice(comma + 1);
  return cssColor(lookup(name) || fallback, lookup, depth + 1);
}

/** The sides of a marked parcel, as the bits of `MapLayerTile.edges`. */
export const EDGE_TOP = 1;
export const EDGE_RIGHT = 2;
export const EDGE_BOTTOM = 4;
export const EDGE_LEFT = 8;
const EDGE_ALL = EDGE_TOP | EDGE_RIGHT | EDGE_BOTTOM | EDGE_LEFT;

/**
 * The width of a mark's outline for a parcel of `s` pixels: an eighth of the parcel, 2 px at the most, on whole
 * device pixels. 0 when that is under 1 px or under 1 device pixel: the mark is then the colour of the whole parcel.
 */
export function markLine(s: number, dpr: number): number {
  const width = Math.min(s / 8, 2);
  const device = Math.floor(width * dpr);
  return width < 1 || device < 1 ? 0 : device / dpr;
}

/** The colour of a marked parcel (LAND on sale or for rent), from the map's custom properties: each surface has its own. */
const MARK_COLOR = "var(--_sale)";

/** The colour of each tile kind, from the map's custom properties (LandMap.module.css). */
const KIND_COLOR: Record<TileKind, string> = {
  owned: "var(--_taken)",
  unowned: "var(--_unowned)",
  road: "var(--_road)",
  plaza: "var(--_plaza)",
  district: "var(--_district)",
};

/**
 * The base layer drawn from tile data: every known parcel in the colour of its kind, joined as the data
 * says, and the parcels `marked` chooses with the mark. It reads all tiles once, here, and works out
 * the sides of each mark here; a draw then costs one array read for each parcel in view.
 */
export function kindLayer(tiles: Tiles, marked?: (tile: LandTile) => boolean): MapLayer {
  const grid = new Array<MapLayerTile | undefined>(GRID_PARCELS * GRID_PARCELS);
  const marks: number[] = [];
  for (const t of Object.values(tiles)) {
    if (t.x < GRID_MIN || t.x > GRID_MAX || t.y < GRID_MIN || t.y > GRID_MAX) continue;
    const at = (t.y - GRID_MIN) * GRID_PARCELS + (t.x - GRID_MIN);
    if (marked?.(t)) {
      grid[at] = { color: KIND_COLOR[t.kind], top: t.top, left: t.left, topLeft: t.topLeft, mark: MARK_COLOR, edges: EDGE_ALL };
      marks.push(at);
    } else grid[at] = { color: KIND_COLOR[t.kind], top: t.top, left: t.left, topLeft: t.topLeft };
  }
  // Marked parcels that are joined (one estate) have one outline: no line on the side where they join.
  for (const at of marks) {
    const t = grid[at]!;
    const col = at % GRID_PARCELS;
    const above = grid[at + GRID_PARCELS];
    const below = grid[at - GRID_PARCELS];
    const before = col > 0 ? grid[at - 1] : undefined;
    const after = col < GRID_PARCELS - 1 ? grid[at + 1] : undefined;
    t.edges = (t.top && above?.mark ? 0 : EDGE_TOP) | (after?.left && after.mark ? 0 : EDGE_RIGHT) | (below?.top && below.mark ? 0 : EDGE_BOTTOM) | (t.left && before?.mark ? 0 : EDGE_LEFT);
  }
  return (x, y) => (x < GRID_MIN || x > GRID_MAX || y < GRID_MIN || y > GRID_MAX ? null : grid[(y - GRID_MIN) * GRID_PARCELS + (x - GRID_MIN)]);
}

/** A marked parcel over the atlas image: the whole parcel, joined on every side, so it has no gap. */
const MARK_TILE: MapLayerTile = { color: MARK_COLOR, top: true, left: true, topLeft: true };

/**
 * The layer of a map whose base is the atlas image: the parcels `marked` chooses, each a whole parcel
 * in the colour of the mark at every zoom, and nothing for the others, so the image shows. It reads
 * all tiles once, here.
 */
export function markLayer(tiles: Tiles, marked: (tile: LandTile) => boolean): MapLayer {
  const grid = new Uint8Array(GRID_PARCELS * GRID_PARCELS);
  for (const t of Object.values(tiles)) {
    if (t.x < GRID_MIN || t.x > GRID_MAX || t.y < GRID_MIN || t.y > GRID_MAX) continue;
    if (marked(t)) grid[(t.y - GRID_MIN) * GRID_PARCELS + (t.x - GRID_MIN)] = 1;
  }
  return (x, y) => (x < GRID_MIN || x > GRID_MAX || y < GRID_MIN || y > GRID_MAX || !grid[(y - GRID_MIN) * GRID_PARCELS + (x - GRID_MIN)] ? null : MARK_TILE);
}

export type LayerCanvas = Pick<CanvasRenderingContext2D, "setTransform" | "clearRect" | "fillRect" | "fillStyle">;

/**
 * Paints the layers for the parcels in view, and only those. Parcel edges are rounded to device
 * pixels, so neighbours share an edge without a seam at any pixel ratio. A marked parcel is painted
 * with its mark when its turn comes: there is no other pass for the marks. With `overscan`, that
 * number of parcels of the grid beyond each side of the view is painted too, for a layer whose tiles
 * are larger than a parcel (`scale`): such a tile next to the view reaches into it. Under one device
 * pixel for a parcel, two edges of a parcel can be on the same pixel line. Such a parcel has no area:
 * it is painted after the other parcels of its layer, in its device pixel, only if no parcel of the
 * layer has that pixel. Thus no pixel is painted twice, and a parcel between two painted pixels gets
 * no pixel of its own. This rule is not for a parcel with a `scale`: its paint does not change.
 */
export function drawLayers(ctx: LayerCanvas, o: { view: MapView; size: MapSize; dpr: number; layers: readonly MapLayer[]; color(value: string): string; overscan?: number }): void {
  const { view, size, dpr } = o;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, size.w, size.h);
  const s = parcelPx(size, view.zoom);
  if (!(s > 0) || o.layers.length === 0) return;
  const inView = viewportOf(view, size);
  const over = Math.max(0, Math.floor(o.overscan ?? 0));
  const nw = { x: clampParcel(inView.nw.x - over), y: clampParcel(inView.nw.y + over) };
  const se = { x: clampParcel(inView.se.x + over), y: clampParcel(inView.se.y - over) };
  const snap = (v: number) => Math.round(v * dpr) / dpr;
  const gap = s < 4 ? 0 : s < 7 ? 0.5 : s < 12 ? 1 : s < 18 ? 1.5 : 2;
  const line = markLine(s, dpr);
  const cols = se.x - nw.x + 1;
  const rows = nw.y - se.y + 1;
  const left = size.w / 2 + (nw.x - 0.5 - view.x) * s;
  const top = size.h / 2 - (nw.y + 0.5 - view.y) * s;
  const xs = new Float64Array(cols + 1);
  const ys = new Float64Array(rows + 1);
  for (let i = 0; i <= cols; i++) xs[i] = snap(left + i * s);
  for (let j = 0; j <= rows; j++) ys[j] = snap(top + j * s);
  let small = false;
  for (let i = 0; i < cols && !small; i++) small = xs[i] === xs[i + 1];
  for (let j = 0; j < rows && !small; j++) small = ys[j] === ys[j + 1];
  // The device pixels of the grid in view, for `small` only.
  const cell = (v: number) => Math.round(v * dpr);
  const cellX = cell(xs[0]!);
  const cellY = cell(ys[0]!);
  const cellW = cell(xs[cols]!) - cellX;
  const cellH = cell(ys[rows]!) - cellY;
  for (const layer of o.layers) {
    let asked = "";
    let fill = "";
    // With `small` only: the device pixels that the layer has painted, then the pixel and the colour
    // of each parcel with no area.
    const used = small && cellW > 0 && cellH > 0 ? new Uint8Array(cellW * cellH) : undefined;
    const late: number[] = [];
    const lateFill: string[] = [];
    for (let j = 0; j < rows; j++) {
      const y = nw.y - j;
      for (let i = 0; i < cols; i++) {
        const t = layer(nw.x + i, y);
        if (!t) continue;
        const mark = t.mark === undefined ? "" : o.color(t.mark);
        const marked = mark !== "" && mark !== "transparent";
        // Too small for an outline: the mark is the colour of the whole parcel.
        const want = marked && line === 0 ? t.mark! : t.color;
        if (want !== asked || !asked) {
          asked = want;
          fill = o.color(asked);
          if (fill && fill !== "transparent") ctx.fillStyle = fill;
        }
        if (!fill || fill === "transparent") continue;
        let x0 = xs[i]!;
        let y0 = ys[j]!;
        let x1 = xs[i + 1]!;
        let y1 = ys[j + 1]!;
        if (used && (t.scale === undefined || t.scale === 1)) {
          const cx = cell(x0) - cellX;
          const cy = cell(y0) - cellY;
          if (x1 === x0 || y1 === y0) {
            // On the last pixel line of the grid, the parcel gets the pixel before that line.
            late.push(Math.min(cy, cellH - 1) * cellW + Math.min(cx, cellW - 1));
            lateFill.push(fill);
            continue;
          }
          const cx1 = cell(x1) - cellX;
          const cy1 = cell(y1) - cellY;
          for (let v = cy; v < cy1; v++) used.fill(1, v * cellW + cx, v * cellW + cx1);
        }
        if (t.scale !== undefined && t.scale !== 1) {
          const grow = ((t.scale - 1) * s) / 2;
          x0 -= grow;
          y0 -= grow;
          x1 += grow;
          y1 += grow;
        }
        if (t.left && t.top) {
          if (t.topLeft) ctx.fillRect(x0, y0, x1 - x0, y1 - y0);
          else {
            // Joined on both sides but not across the corner: all but the corner, in two parts that do
            // not overlap, so a colour with an alpha is not painted twice.
            ctx.fillRect(x0, y0 + gap, x1 - x0, y1 - y0 - gap);
            if (gap > 0) ctx.fillRect(x0 + gap, y0, x1 - x0 - gap, gap);
          }
        } else if (t.left) ctx.fillRect(x0, y0 + gap, x1 - x0, y1 - y0 - gap);
        else if (t.top) ctx.fillRect(x0 + gap, y0, x1 - x0 - gap, y1 - y0);
        else ctx.fillRect(x0 + gap, y0 + gap, x1 - x0 - gap, y1 - y0 - gap);
        if (!marked || line === 0) continue;
        // The outline: a strip inside each side in `edges`. The strips do not overlap, so a colour with
        // an alpha is not painted twice. The next parcel sets its own colour again.
        const edges = t.edges ?? EDGE_ALL;
        ctx.fillStyle = mark;
        asked = "";
        const bx = t.left ? x0 : x0 + gap;
        const by = t.top ? y0 : y0 + gap;
        const ya = edges & EDGE_TOP ? by + line : by;
        const yb = edges & EDGE_BOTTOM ? y1 - line : y1;
        if (edges & EDGE_TOP) ctx.fillRect(bx, by, x1 - bx, line);
        if (edges & EDGE_BOTTOM) ctx.fillRect(bx, y1 - line, x1 - bx, line);
        if (edges & EDGE_LEFT) ctx.fillRect(bx, ya, line, yb - ya);
        if (edges & EDGE_RIGHT) ctx.fillRect(x1 - line, ya, line, yb - ya);
      }
    }
    if (!used) continue;
    let lateStyle = "";
    for (let n = 0; n < late.length; n++) {
      const at = late[n]!;
      if (used[at]) continue;
      used[at] = 1;
      if (lateFill[n] !== lateStyle) ctx.fillStyle = lateStyle = lateFill[n]!;
      ctx.fillRect((cellX + (at % cellW)) / dpr, (cellY + Math.floor(at / cellW)) / dpr, 1 / dpr, 1 / dpr);
    }
  }
}
