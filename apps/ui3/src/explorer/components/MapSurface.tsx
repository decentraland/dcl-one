import { useRef, useState, useSyncExternalStore } from "react";
import { serviceHref } from "../../data/catalyst/client";
import "./mapsurface.css";

/**
 * The one way Genesis City is drawn: the atlas image (every parcel at 6px, one request,
 * cached) or the satellite tiles, inside a square that parcelRectPercent's percentages
 * address. The Map panel puts it under its pins and drags it around; the minimap puts
 * it under the player and clips it to a circle. Both read the same view choice.
 */

export type MapView = "map" | "satellite";
export const MAP_VIEW_STORAGE = "dcl.map.view";
/** Parcels across the square: -170..169 on both axes. */
export const GRID_PARCELS = 340;

let fallbackView: MapView = "satellite";
const viewListeners = new Set<() => void>();

export function readMapView(): MapView {
  try {
    return localStorage.getItem(MAP_VIEW_STORAGE) === "map" ? "map" : "satellite";
  } catch {
    return fallbackView;
  }
}

export function setMapView(next: MapView): void {
  fallbackView = next;
  try { localStorage.setItem(MAP_VIEW_STORAGE, next); } catch {}
  viewListeners.forEach((l) => l());
}

function subscribeView(listener: () => void): () => void {
  viewListeners.add(listener);
  const onStorage = (e: StorageEvent) => { if (e.key === MAP_VIEW_STORAGE) listener(); };
  window.addEventListener("storage", onStorage);
  return () => { viewListeners.delete(listener); window.removeEventListener("storage", onStorage); };
}

/** The remembered view, shared by every map on the page. */
export function useMapView(): [MapView, (next: MapView) => void] {
  const view = useSyncExternalStore(subscribeView, readMapView, () => "satellite" as MapView);
  return [view, setMapView];
}

const SAT_BASE = serviceHref("satellite");
const SAT_WORLD_MIN = -256;
const SAT_WORLD_SPAN = 512;
const SAT_GRID_MIN = -170;
const SAT_GRID_SPAN = GRID_PARCELS;
const SAT_TILE = (z: number, x: number, y: number) => `${SAT_BASE}/${z}/${x}/${y}.png`;
/** The whole grid at 6px per parcel. The renderer puts the CENTER of parcel (cx, cy) at pixel
    (1020, 1020) of a 2040px image, so with center=-1,0 the image covers -170..169 on both axes,
    half a parcel right of and below parcelRectPercent's grid; mapsurface.css shifts it back
    (measured against /v2/tiles, 2026-09-23). */
const ATLAS_SRC = `${serviceHref("map")}/v1/map.png?width=2040&height=2040&size=6&center=-1,0&on-sale=true`;

export type SlippyView = { w: number; h: number; square: number; panX: number; panY: number };
export type Tile = { key: string; src: string; left: number; top: number; size: number };

export function slippyTilesFor(zoom: number, view: SlippyView | null): Tile[] {
  if (!SAT_BASE) return [];
  const z = Math.max(1, Math.min(7, Math.round(2.6 + Math.log2(zoom))));
  const n = 1 << z;
  const span = SAT_WORLD_SPAN / n;

  let west;
  let east;
  let south;
  let north;
  if (view && view.square > 0) {
    const scaled = view.square * zoom;
    const u0 = 0.5 + (-view.w / 2 - view.panX) / scaled;
    const u1 = 0.5 + (view.w / 2 - view.panX) / scaled;
    const v0 = 0.5 + (-view.h / 2 - view.panY) / scaled;
    const v1 = 0.5 + (view.h / 2 - view.panY) / scaled;
    west = SAT_GRID_MIN + u0 * SAT_GRID_SPAN;
    east = SAT_GRID_MIN + u1 * SAT_GRID_SPAN;
    north = SAT_GRID_MIN + SAT_GRID_SPAN - v0 * SAT_GRID_SPAN;
    south = SAT_GRID_MIN + SAT_GRID_SPAN - v1 * SAT_GRID_SPAN;
  } else {
    west = -170 / zoom;
    east = 170 / zoom;
    south = -170 / zoom;
    north = 170 / zoom;
  }

  const xIdx = (w: number) => Math.floor((w - SAT_WORLD_MIN) / span);
  const yIdx = (w: number) => Math.floor((SAT_WORLD_MIN + SAT_WORLD_SPAN - w) / span);
  const loX = Math.max(0, xIdx(west) - 1);
  const hiX = Math.min(n - 1, xIdx(east) + 1);
  const loY = Math.max(0, yIdx(north) - 1);
  const hiY = Math.min(n - 1, yIdx(south) + 1);

  const tiles: Tile[] = [];
  for (let x = loX; x <= hiX; x++) {
    const wx0 = SAT_WORLD_MIN + x * span;
    for (let y = loY; y <= hiY; y++) {
      const wyTop = SAT_WORLD_MIN + SAT_WORLD_SPAN - y * span;
      tiles.push({
        key: `${z}/${x}/${y}`,
        src: SAT_TILE(z, x, y),
        left: ((wx0 - SAT_GRID_MIN) / SAT_GRID_SPAN) * 100,
        top: ((SAT_GRID_MIN + SAT_GRID_SPAN - wyTop) / SAT_GRID_SPAN) * 100,
        size: (span / SAT_GRID_SPAN) * 100,
      });
    }
  }
  return tiles;
}

/** Tiles to keep mounted: the wanted set plus, until every wanted tile has loaded, the already-loaded
    tiles of the previous render (the last zoom level) underneath it, so a level change never paints blank. */
export function retainTiles(shown: Tile[], wanted: Tile[], loaded: ReadonlySet<string>): Tile[] {
  if (wanted.every((t) => loaded.has(t.key))) return wanted;
  const keys = new Set(wanted.map((t) => t.key));
  const under = shown.filter((t) => !keys.has(t.key) && loaded.has(t.key));
  return under.length ? [...under, ...wanted] : wanted;
}


type Props = {
  view: MapView;
  /** The scale of the square relative to its natural size (the shell's smaller side). */
  zoom: number;
  /** The shell the square is shown through, for the satellite tiles in range; null before measuring. */
  viewport: SlippyView | null;
};

export default function MapSurface({ view, zoom, viewport }: Props) {
  const loadedRef = useRef(new Set<string>());
  const shownRef = useRef<Tile[]>([]);
  const [, setTick] = useState(0);
  const markLoaded = (key: string) => {
    if (loadedRef.current.has(key)) return;
    loadedRef.current.add(key);
    setTick((t) => t + 1);
  };
  const tiles = view === "satellite" ? retainTiles(shownRef.current, slippyTilesFor(zoom, viewport), loadedRef.current) : [];
  shownRef.current = tiles;
  return (
    <>
      <div className="map__grid" />
      {view === "satellite" && <div className="map__roads" />}
      {view === "map" && <img className="map__atlas" src={ATLAS_SRC} alt="" draggable={false} decoding="async" />}
      {tiles.map((t) => (
        <img
          key={t.key}
          className="map__satellite"
          src={t.src}
          alt=""
          draggable={false}
          decoding="async"
          onLoad={() => markLoaded(t.key)}
          ref={(el) => { if (el?.complete && el.naturalWidth) markLoaded(t.key); }}
          style={{ left: t.left + "%", top: t.top + "%", width: `calc(${t.size}% + 0.5px)`, height: `calc(${t.size}% + 0.5px)` }}
        />
      ))}
    </>
  );
}
