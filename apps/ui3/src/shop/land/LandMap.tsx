// from: marketplace@f3956826:webapp/src/components/Atlas/Atlas.tsx, webapp/src/components/Atlas/Popup/Popup.tsx,
// webapp/src/components/AssetBrowse/MapBrowse/MapBrowse.tsx
// Copyright 2020 Metaverse Holdings Ltd. Licensed under the Apache License, Version 2.0.
// The LAND map on one of two bases (`surface`). "atlas", the Shop's: ui3's atlas image of Genesis City
// (explorer/components/MapSurface), and a canvas over it that paints only the LAND on sale or for rent.
// "tiles", for a host with its own colours: no image, and every parcel painted from the tile data on
// the canvas (mapView.ts), LAND on sale or for rent with a mark in the same paint. On top of both, the
// Shop's other layers as one SVG in parcel units: the account's own and rented LAND, and a selection.

import { memo, useEffect, useId, useLayoutEffect, useMemo, useRef, useState, type PointerEvent as ReactPointerEvent, type ReactNode } from "react";

import MapSurface, { GRID_PARCELS } from "../../explorer/components/MapSurface";
import { landTokenId } from "../../data/catalyst/landListing";
import { shortAddress } from "@shop/model/address";
import { landChainId } from "./chain";
import { landNetwork } from "@shop/land/contracts";
import { Mana } from "./format";
import { GRID_MAX, GRID_MID, GRID_MIN, clampView, cssColor, drawLayers, kindLayer, markLayer, parcelPx, placePopup, pointToCoord, stagePan, viewportOf, zoomAt, type MapLayer, type MapPoint, type MapSize, type MapView, type MapViewport } from "./mapView";
import { coordsId, isTileInSelection, type Coord, type LandTile, type Tiles } from "./tiles";
import "./land.css";
import styles from "./LandMap.module.css";

const ZOOM_MIN = 1;
const ZOOM_MAX = 12;
const ZOOM_STEP = 1.5;
/** A press that moves further than this is a drag, not a click. */
const DRAG_SLOP = 4;

// One value for every render with the prop absent, so the memos that depend on them hold.
const NO_SELECTION: readonly Coord[] = [];
const NO_LAYERS: LandMapLayers = {};
const NO_BASE: readonly MapLayer[] = [];
const NO_PAINT: readonly MapLayer[] = [];
const NO_ZOOM_BUTTONS: NonNullable<LandMapProps["zoomButtons"]> = {};
const defaultZoomStep = (zoom: number, direction: 1 | -1) => (direction > 0 ? zoom * ZOOM_STEP : zoom / ZOOM_STEP);

export type LandMapLayers = {
  onSale?: boolean;
  forRent?: boolean;
  /** Coordinates of the account's LAND, and of its LAND currently rented out. */
  owned?: ReadonlySet<string> | null;
  rented?: ReadonlySet<string> | null;
};

export type LandMapProps = {
  tiles: Tiles | null;
  /**
   * The base of the map. "atlas" (the default) is the map service's image, before and after the tile
   * data arrives; the canvas then paints only the LAND on sale or for rent, as whole parcels. "tiles"
   * shows no image: the canvas paints every parcel in the colour of its kind when the data arrives.
   */
  surface?: "atlas" | "tiles";
  layers?: LandMapLayers;
  selection?: readonly Coord[];
  strictSelection?: boolean;
  isEstate?: boolean;
  center?: Coord;
  initialZoom?: number;
  /** Clicking a parcel opens its token page (its estate's when it belongs to one). */
  onOpen?(href: string, tile: LandTile): void;
  withPopup?: boolean;
  withLegend?: boolean;
  chainId?: number;
  label?: string;
  className?: string;
  /** Layers painted on the canvas over the map's own layer, in this order (`MapLayer` of mapView.ts). */
  paintLayers?: readonly MapLayer[];
  /** The zoom range; the default is 1 to 12. */
  minZoom?: number;
  maxZoom?: number;
  /** False: a drag does not move the map. The pointer callbacks below still run. */
  draggable?: boolean;
  /** True: a drag with any mouse button moves the map, not only the primary button. */
  dragWithAnyButton?: boolean;
  /** "always": every wheel turn zooms. The default zooms only with Ctrl or the command key held. */
  wheelZoom?: "modifier" | "always";
  /** False: no zoom buttons. */
  withZoomControls?: boolean;
  /** Shown over the map's top-left corner, where the buttons and the page's filters leave room: the
      atlas's age (`MapGridAge`) on the Shop's pages. */
  status?: ReactNode;
  /** The names and the texts of the zoom buttons; the default is "Zoom in" with + and "Zoom out" with the minus sign. */
  zoomButtons?: { inLabel?: string; outLabel?: string; inText?: string; outText?: string };
  /** The zoom after a press of a zoom button (1 in, -1 out); the default multiplies or divides by 1.5. */
  zoomStep?(zoom: number, direction: 1 | -1): number;
  /** True: the canvas is painted in the task of the change, not on the next frame, for a host that reads it at once. Over the atlas it always is. */
  paintInTask?: boolean;
  /** Parcels beyond each side of the view that the layers paint too, for a layer with tiles larger than a parcel; the default is 0. */
  overscan?: number;
  /** The parcel under the pointer, called when it changes. */
  onParcelHover?(x: number, y: number): void;
  /** A press and release of the primary button on one parcel with no drag between them. */
  onParcelClick?(x: number, y: number): void;
  onParcelDown?(x: number, y: number): void;
  onParcelUp?(x: number, y: number): void;
  /** The centre, the zoom and the corner parcels of the view, after each change of them. */
  onViewChange?(viewport: MapViewport): void;
};

/** The Shop token page a parcel is sold as: its estate when it belongs to one, else the LAND itself. */
export function tileHref(tile: LandTile, chainId?: number): string {
  const net = landNetwork(chainId ?? landChainId());
  return tile.estateId ? `/shop/token/${net.estate}/${tile.estateId}` : `/shop/token/${net.land}/${landTokenId(tile.x, tile.y)}`;
}

const cellX = (x: number) => x - GRID_MIN;
const cellY = (y: number) => GRID_MAX - y;

type Hover = { tile: LandTile; at: MapPoint };
type Drag = { id: number; x: number; y: number; from: MapView; moved: boolean };
type Cells = { mine: { tile: LandTile; rented: boolean }[]; picked: LandTile[] };

/** The account's parcels and the selection, one element each. Memoised: a pan, a zoom or a hover does not touch them. */
const ShopLayer = memo(function ShopLayer({ cells }: { cells: Cells }) {
  return (
    <svg className="shop-landmap__layer" viewBox={`0 0 ${GRID_PARCELS} ${GRID_PARCELS}`} preserveAspectRatio="none" aria-hidden="true">
      {cells.mine.map(({ tile, rented }) => (
        <rect key={`o${tile.id}`} className={rented ? "shop-landmap__rented" : "shop-landmap__owned"} x={cellX(tile.x)} y={cellY(tile.y)} width={1} height={1} />
      ))}
      {cells.picked.map((t) => (
        <rect key={`p${t.id}`} className="shop-landmap__selected" x={cellX(t.x)} y={cellY(t.y)} width={1} height={1} />
      ))}
    </svg>
  );
});

export default function LandMap({
  tiles,
  surface = "atlas",
  layers = NO_LAYERS,
  selection = NO_SELECTION,
  strictSelection = false,
  isEstate = false,
  center,
  initialZoom = 3,
  onOpen,
  withPopup = true,
  withLegend = false,
  chainId,
  label = "LAND map",
  className = "",
  paintLayers = NO_PAINT,
  minZoom = ZOOM_MIN,
  maxZoom = ZOOM_MAX,
  draggable = true,
  dragWithAnyButton = false,
  wheelZoom = "modifier",
  withZoomControls = true,
  status,
  zoomButtons = NO_ZOOM_BUTTONS,
  zoomStep = defaultZoomStep,
  paintInTask = false,
  overscan = 0,
  onParcelHover,
  onParcelClick,
  onParcelDown,
  onParcelUp,
  onViewChange,
}: LandMapProps) {
  const shellRef = useRef<HTMLDivElement | null>(null);
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const [size, setSize] = useState<MapSize>({ w: 480, h: 480 });
  // The view as asked for: the parcel at the middle of the shell, and the zoom. It is kept through a
  // resize, and `view` is what the shell's size allows of it.
  const [asked, setAsked] = useState<MapView>(() => ({ x: center?.x ?? GRID_MID, y: center?.y ?? GRID_MID, zoom: initialZoom }));
  const [dpr, setDpr] = useState(1);
  const [theme, setTheme] = useState(0);
  const [hover, setHover] = useState<Hover | null>(null);
  const [legendOpen, setLegendOpen] = useState(false);
  const drag = useRef<Drag | null>(null);
  const legendId = useId();

  const view = useMemo(() => clampView(asked, size, minZoom, maxZoom), [asked, size, minZoom, maxZoom]);
  const viewRef = useRef(view);
  viewRef.current = view;
  const sizeRef = useRef(size);
  sizeRef.current = size;
  // The props that the handlers read at the time of the event.
  const opts = useRef({ minZoom, maxZoom, draggable, wheelZoom, onViewChange });
  opts.current = { minZoom, maxZoom, draggable, wheelZoom, onViewChange };
  const limitZoom = (zoom: number) => Math.max(opts.current.minZoom, Math.min(opts.current.maxZoom, zoom));
  const hoverCoord = useRef<Coord | null>(null);

  /** Changes the view from the one before it, so two changes between two renders both count. */
  const change = (next: (view: MapView, size: MapSize) => MapView) =>
    setAsked((prev) => {
      const s = sizeRef.current;
      const { minZoom: lo, maxZoom: hi } = opts.current;
      const from = clampView(prev, s, lo, hi);
      const to = clampView(next(from, s), s, lo, hi);
      return to.x === prev.x && to.y === prev.y && to.zoom === prev.zoom ? prev : to;
    });

  useEffect(() => {
    const el = shellRef.current;
    if (!el) return;
    const measure = () => {
      const r = el.getBoundingClientRect();
      if (r.width > 0 && r.height > 0) setSize((s) => (s.w === r.width && s.h === r.height ? s : { w: r.width, h: r.height }));
    };
    measure();
    if (typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  // The size before the first paint in the task: the observer above tells it one frame later.
  useLayoutEffect(() => {
    const el = shellRef.current;
    if (!paintInTask || !el) return;
    const r = el.getBoundingClientRect();
    if (r.width > 0 && r.height > 0) setSize((s) => (s.w === r.width && s.h === r.height ? s : { w: r.width, h: r.height }));
  }, [paintInTask]);

  useEffect(() => {
    opts.current.onViewChange?.(viewportOf(view, size));
  }, [view, size]);

  // Re-centre when the target moves (a filter change, a new asset), not on a zoom or a resize.
  const cx = center?.x;
  const cy = center?.y;
  useEffect(() => {
    if (cx === undefined || cy === undefined) return;
    setAsked((v) => (v.x === cx && v.y === cy ? v : { ...v, x: cx, y: cy }));
  }, [cx, cy]);

  // The canvas follows the pixel ratio (a move to another screen, a browser zoom) and the theme.
  useEffect(() => {
    let stop = () => {};
    const watch = () => {
      const ratio = window.devicePixelRatio || 1;
      setDpr(ratio);
      if (typeof window.matchMedia !== "function") return;
      const mq = window.matchMedia(`(resolution: ${ratio}dppx)`);
      const onChange = () => {
        stop();
        watch();
      };
      mq.addEventListener("change", onChange);
      stop = () => mq.removeEventListener("change", onChange);
    };
    watch();
    let mo: MutationObserver | null = null;
    if (typeof MutationObserver !== "undefined") {
      mo = new MutationObserver(() => setTheme((n) => n + 1));
      mo.observe(document.documentElement, { attributes: true, attributeFilter: ["class"] });
    }
    return () => {
      stop();
      mo?.disconnect();
    };
  }, []);

  const pointOf = (e: { clientX: number; clientY: number }): MapPoint => {
    const r = shellRef.current?.getBoundingClientRect();
    return { x: e.clientX - (r?.left ?? 0), y: e.clientY - (r?.top ?? 0) };
  };
  const tileAt = (p: MapPoint): LandTile | undefined => {
    const c = tiles ? pointToCoord(viewRef.current, sizeRef.current, p) : null;
    return c ? tiles![coordsId(c.x, c.y)] : undefined;
  };

  const onPointerDown = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (e.button !== 0 && !dragWithAnyButton) return;
    const el = e.currentTarget;
    const held = drag.current;
    // A second finger does not take the drag from the first while the first is still down.
    if (held && held.id !== e.pointerId && el.hasPointerCapture?.(held.id)) return;
    drag.current = { id: e.pointerId, x: e.clientX, y: e.clientY, from: viewRef.current, moved: false };
    try {
      el.setPointerCapture?.(e.pointerId);
    } catch {
      // The pointer is already gone: the drag ends on the next leave.
    }
    if (!onParcelDown) return;
    const c = pointToCoord(viewRef.current, sizeRef.current, pointOf(e));
    if (c) onParcelDown(c.x, c.y);
  };
  const onPointerMove = (e: ReactPointerEvent<HTMLDivElement>) => {
    const d = drag.current;
    if (d) {
      if (d.id !== e.pointerId) return;
      const dx = e.clientX - d.x;
      const dy = e.clientY - d.y;
      if (Math.abs(dx) + Math.abs(dy) > DRAG_SLOP) d.moved = true;
      if (d.moved && opts.current.draggable) {
        const px = parcelPx(sizeRef.current, d.from.zoom);
        if (px > 0) change(() => ({ x: d.from.x - dx / px, y: d.from.y + dy / px, zoom: d.from.zoom }));
        setHover(null);
        return;
      }
    }
    const at = pointOf(e);
    if (onParcelHover) {
      const c = pointToCoord(viewRef.current, sizeRef.current, at);
      const last = hoverCoord.current;
      if (c && (!last || last.x !== c.x || last.y !== c.y)) {
        hoverCoord.current = c;
        onParcelHover(c.x, c.y);
      }
    }
    if (!withPopup || !tiles) return;
    const tile = tileAt(at);
    setHover(tile ? { tile, at } : null);
  };
  const onPointerUp = (e: ReactPointerEvent<HTMLDivElement>) => {
    const d = drag.current;
    if (!d || d.id !== e.pointerId) return;
    drag.current = null;
    const at = pointOf(e);
    const c = onParcelUp || onParcelClick ? pointToCoord(viewRef.current, sizeRef.current, at) : null;
    if (c) onParcelUp?.(c.x, c.y);
    if (d.moved || e.button !== 0) return;
    if (c) onParcelClick?.(c.x, c.y);
    if (!onOpen) return;
    const tile = tileAt(at);
    if (tile && (tile.kind === "owned" || tile.kind === "unowned" || tile.estateId)) onOpen(tileHref(tile, chainId), tile);
  };
  const endDrag = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (drag.current?.id === e.pointerId) drag.current = null;
  };
  const onPointerLeave = (e: ReactPointerEvent<HTMLDivElement>) => {
    // With the pointer captured there is no leave during a drag; without capture, a drag ends here.
    if (drag.current?.id === e.pointerId && !e.currentTarget.hasPointerCapture?.(e.pointerId)) drag.current = null;
    setHover(null);
    hoverCoord.current = null;
  };

  useEffect(() => {
    const el = shellRef.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      if (opts.current.wheelZoom !== "always" && !e.ctrlKey && !e.metaKey) return;
      e.preventDefault();
      const r = el.getBoundingClientRect();
      const anchor = { x: e.clientX - r.left, y: e.clientY - r.top };
      const factor = Math.exp(-e.deltaY * 0.0015);
      change((v, s) => zoomAt(v, s, limitZoom(v.zoom * factor), anchor));
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  }, []);

  const selectionSet = useMemo(() => new Set(selection.map((c) => coordsId(c.x, c.y))), [selection]);

  const cells = useMemo<Cells>(() => {
    const mine: { tile: LandTile; rented: boolean }[] = [];
    const picked: LandTile[] = [];
    if (tiles) {
      for (const tile of Object.values(tiles)) {
        if (layers.owned?.has(tile.id)) mine.push({ tile, rented: !!layers.rented?.has(tile.id) });
        if (selectionSet.size && isTileInSelection(selectionSet, tiles, tile.x, tile.y, isEstate, strictSelection)) picked.push(tile);
      }
    }
    if (!tiles || strictSelection) {
      for (const id of selectionSet) {
        if (tiles?.[id] && picked.some((t) => t.id === id)) continue;
        const [x, y] = id.split(",").map(Number) as [number, number];
        if (!picked.some((t) => t.x === x && t.y === y)) picked.push({ id, x, y, kind: "owned", top: false, left: false, topLeft: false, owner: null, name: null, estateId: null, price: null, rentalPricePerDay: null });
      }
    }
    return { mine, picked };
  }, [tiles, layers.owned, layers.rented, selectionSet, isEstate, strictSelection]);

  // The map's own layer on the canvas. Over the atlas image: only the LAND on sale or for rent that
  // the layers ask for. With no image: every known parcel in the colour of its kind, and the mark of
  // that LAND in the same paint. One draw for each frame, of the parcels in view only.
  const { onSale, forRent } = layers;
  const kinds = useMemo(() => {
    if (!tiles) return NO_BASE;
    const listed = onSale || forRent ? (tile: LandTile) => (!!onSale && tile.price !== null) || (!!forRent && tile.rentalPricePerDay !== null) : undefined;
    if (surface === "tiles") return [kindLayer(tiles, listed)];
    return listed ? [markLayer(tiles, listed)] : NO_BASE;
  }, [tiles, surface, onSale, forRent]);
  const base = useMemo(() => (paintLayers.length ? [...kinds, ...paintLayers] : kinds), [kinds, paintLayers]);
  const paint = useRef(() => {});
  paint.current = () => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const w = Math.max(1, Math.round(size.w * dpr));
    const h = Math.max(1, Math.round(size.h * dpr));
    if (canvas.width !== w) canvas.width = w;
    if (canvas.height !== h) canvas.height = h;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    const computed = window.getComputedStyle(canvas);
    const known = new Map<string, string>();
    const color = (value: string) => {
      let c = known.get(value);
      if (c === undefined) known.set(value, (c = cssColor(value, (name) => computed.getPropertyValue(name))));
      return c;
    };
    drawLayers(ctx, { view, size, dpr, layers: base, color, overscan });
  };
  // Over the atlas image the canvas paints in the commit that moves the image: on the next frame its marks trail the map.
  const inTask = paintInTask || surface !== "tiles";
  const frame = useRef({ queued: false, id: 0 });
  useEffect(() => {
    const f = frame.current;
    if (!base.length || f.queued || inTask) return;
    f.queued = true;
    f.id = requestAnimationFrame(() => {
      f.queued = false;
      paint.current();
    });
  }, [base, view, size, dpr, theme, inTask, overscan]);
  useLayoutEffect(() => {
    if (inTask && base.length) paint.current();
  }, [base, view, size, dpr, theme, inTask, overscan]);
  useEffect(() => {
    const f = frame.current;
    return () => {
      if (f.queued) cancelAnimationFrame(f.id);
      f.queued = false;
    };
  }, []);

  const square = Math.min(size.w, size.h);
  const pan = stagePan(view, size);
  const hovered = hover?.tile;
  const atlas = surface !== "tiles";
  const stage = { width: square, height: square, transform: `translate(-50%, -50%) translate(${pan.x}px, ${pan.y}px) scale(${view.zoom})` };

  return (
    <div ref={shellRef} className={`shop-landmap ${styles.root} ${atlas ? styles.atlas : "shop-landmap--tiles"} ${className}`} role="region" aria-label={label}>
      {/* The pointer handlers are on the view only: a press on the buttons below is not a press on the map. */}
      <div className={styles.view} onPointerDown={onPointerDown} onPointerMove={onPointerMove} onPointerUp={onPointerUp} onPointerCancel={endDrag} onLostPointerCapture={endDrag} onPointerLeave={onPointerLeave}>
        {/* From the bottom: the atlas image, the canvas, the Shop's layers. */}
        {atlas ? (
          <div className="shop-landmap__stage" style={stage}>
            <MapSurface view="map" zoom={view.zoom} viewport={{ w: size.w, h: size.h, square, panX: pan.x, panY: pan.y }} />
          </div>
        ) : null}
        {base.length ? <canvas ref={canvasRef} className={styles.canvas} aria-hidden="true" /> : null}
        <div className="shop-landmap__stage" style={stage}>
          <ShopLayer cells={cells} />
        </div>
      </div>

      {status != null ? <div className={`shop-landmap__status ${styles.status}`}>{status}</div> : null}

      {withZoomControls ? (
        <div className="shop-landmap__zoom">
          <button type="button" aria-label={zoomButtons.inLabel ?? "Zoom in"} onClick={() => change((v, s) => zoomAt(v, s, limitZoom(zoomStep(v.zoom, 1))))} disabled={view.zoom >= maxZoom}>
            {zoomButtons.inText ?? "+"}
          </button>
          <button type="button" aria-label={zoomButtons.outLabel ?? "Zoom out"} onClick={() => change((v, s) => zoomAt(v, s, limitZoom(zoomStep(v.zoom, -1))))} disabled={view.zoom <= minZoom}>
            {zoomButtons.outText ?? "−"}
          </button>
        </div>
      ) : null}

      {withLegend ? (
        <div className="shop-landmap__legend">
          <button type="button" aria-expanded={legendOpen} aria-controls={legendId} onClick={() => setLegendOpen((o) => !o)}>
            Map colors
          </button>
          {legendOpen ? (
            <ul id={legendId}>
              <li>
                <span className="shop-landmap__swatch shop-landmap__swatch--plaza" aria-hidden="true" /> Plaza
              </li>
              {atlas ? null : (
                <li>
                  <span className="shop-landmap__swatch shop-landmap__swatch--district" aria-hidden="true" /> District
                </li>
              )}
              <li>
                <span className="shop-landmap__swatch shop-landmap__swatch--owned" aria-hidden="true" /> Your LAND
              </li>
              <li>
                <span className="shop-landmap__swatch shop-landmap__swatch--rented" aria-hidden="true" /> Your rented LAND
              </li>
              <li>
                <span className="shop-landmap__swatch shop-landmap__swatch--sale" aria-hidden="true" /> On sale or on rent
              </li>
              <li>
                <span className="shop-landmap__swatch shop-landmap__swatch--taken" aria-hidden="true" /> Taken
              </li>
            </ul>
          ) : null}
        </div>
      ) : null}

      {hover && hovered && withPopup ? (
        <div className="shop-landmap__popup" role="tooltip" style={placePopup(hover.at, size)}>
          <div className="shop-landmap__popuphead">
            <strong>{hovered.name || (hovered.estateId ? "Estate" : hovered.kind === "road" ? "Road" : hovered.kind === "plaza" ? "Plaza" : hovered.kind === "district" ? "District" : "Parcel")}</strong>
            <span className="shop-landmap__coords">
              {hovered.x},{hovered.y}
            </span>
          </div>
          {hovered.owner ? (
            <div className="shop-landmap__popuprow">
              <span>Owner</span>
              <span>{shortAddress(hovered.owner)}</span>
            </div>
          ) : null}
          {hovered.price !== null || hovered.rentalPricePerDay !== null ? (
            <div className="shop-landmap__popuprow">
              <span>{hovered.price !== null && hovered.rentalPricePerDay !== null ? "Available for sale or rent" : hovered.price !== null ? "Available for sale" : "Available for rent"}</span>
              <span className="shop-landmap__prices">
                {hovered.price !== null ? <Mana text={hovered.price.toLocaleString("en-US", { maximumFractionDigits: 18 })} /> : null}
                {hovered.rentalPricePerDay !== null ? <Mana wei={hovered.rentalPricePerDay} suffix="/day" /> : null}
              </span>
            </div>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}
