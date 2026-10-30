// from: marketplace@f3956826:webapp/src/components/Atlas/Atlas.tsx, webapp/src/components/Atlas/Popup/Popup.tsx,
// webapp/src/components/AssetBrowse/MapBrowse/MapBrowse.tsx
// Copyright 2020 Metaverse Holdings Ltd. Licensed under the Apache License, Version 2.0.
// The LAND map: ui3's one Genesis City surface (explorer/components/MapSurface, the atlas image the
// Map panel and the minimap draw) with the Shop's layers on top: LAND on sale or for rent, the
// account's own and rented LAND, and a selection. decentraland-ui's canvas Atlas is not carried; the
// layers are one SVG in parcel units over the same square parcelRectPercent addresses.

import { useCallback, useEffect, useId, useMemo, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";

import MapSurface, { GRID_PARCELS } from "../../explorer/components/MapSurface";
import { landTokenId } from "../../data/catalyst/landListing";
import { shortAddress } from "../model/address";
import { landNetwork } from "./contracts";
import { Mana } from "./format";
import { coordsId, isTileInSelection, type Coord, type LandTile, type Tiles } from "./tiles";
import "./land.css";

const GRID_MIN = -170;
const GRID_MAX = GRID_MIN + GRID_PARCELS;
const ZOOM_MIN = 1;
const ZOOM_MAX = 12;
const ZOOM_STEP = 1.5;

export type LandMapLayers = {
  onSale?: boolean;
  forRent?: boolean;
  /** Coordinates of the account's LAND, and of its LAND currently rented out. */
  owned?: ReadonlySet<string> | null;
  rented?: ReadonlySet<string> | null;
};

export type LandMapProps = {
  tiles: Tiles | null;
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
};

/** The Shop token page a parcel is sold as: its estate when it belongs to one, else the LAND itself. */
export function tileHref(tile: LandTile, chainId?: number): string {
  const net = landNetwork(chainId);
  return tile.estateId ? `/shop/token/${net.estate}/${tile.estateId}` : `/shop/token/${net.land}/${landTokenId(tile.x, tile.y)}`;
}

const cellX = (x: number) => x - GRID_MIN;
const cellY = (y: number) => GRID_MAX - 1 - y;

type Hover = { tile: LandTile; left: number; top: number };

export default function LandMap({
  tiles,
  layers = {},
  selection = [],
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
}: LandMapProps) {
  const shellRef = useRef<HTMLDivElement | null>(null);
  const stageRef = useRef<HTMLDivElement | null>(null);
  const [square, setSquare] = useState(480);
  const [size, setSize] = useState({ w: 480, h: 480 });
  const [zoom, setZoom] = useState(initialZoom);
  const [pan, setPan] = useState({ x: 0, y: 0 });
  const [hover, setHover] = useState<Hover | null>(null);
  const [legendOpen, setLegendOpen] = useState(false);
  const drag = useRef<{ x: number; y: number; px: number; py: number; moved: boolean } | null>(null);
  const legendId = useId();

  useEffect(() => {
    const el = shellRef.current;
    if (!el) return;
    const measure = () => {
      const r = el.getBoundingClientRect();
      if (r.width > 0 && r.height > 0) {
        setSize({ w: r.width, h: r.height });
        setSquare(Math.min(r.width, r.height));
      }
    };
    measure();
    if (typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const clampPan = useCallback(
    (x: number, y: number, z: number) => {
      const maxX = Math.max(0, (square * z - size.w) / 2);
      const maxY = Math.max(0, (square * z - size.h) / 2);
      return { x: Math.max(-maxX, Math.min(maxX, x)), y: Math.max(-maxY, Math.min(maxY, y)) };
    },
    [square, size],
  );

  const centerOn = useCallback(
    (c: Coord, z: number) => {
      const u = (cellX(c.x) + 0.5) / GRID_PARCELS - 0.5;
      const v = (cellY(c.y) + 0.5) / GRID_PARCELS - 0.5;
      setPan(clampPan(-u * square * z, -v * square * z, z));
    },
    [clampPan, square],
  );

  useEffect(() => {
    if (center) centerOn(center, zoom);
    // Re-centre when the target moves (a filter change, a new asset), not on every zoom.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [center?.x, center?.y, square]);

  const zoomTo = (next: number) => {
    const z = Math.max(ZOOM_MIN, Math.min(ZOOM_MAX, next));
    const f = z / zoom;
    setZoom(z);
    setPan((p) => clampPan(p.x * f, p.y * f, z));
  };

  const coordsAt = (clientX: number, clientY: number): Coord | null => {
    const el = stageRef.current;
    if (!el) return null;
    const r = el.getBoundingClientRect();
    if (r.width === 0 || r.height === 0) return null;
    const u = (clientX - r.left) / r.width;
    const v = (clientY - r.top) / r.height;
    if (u < 0 || u >= 1 || v < 0 || v >= 1) return null;
    return { x: GRID_MIN + Math.floor(u * GRID_PARCELS), y: GRID_MAX - 1 - Math.floor(v * GRID_PARCELS) };
  };

  const onPointerDown = (e: ReactPointerEvent<HTMLDivElement>) => {
    drag.current = { x: e.clientX, y: e.clientY, px: pan.x, py: pan.y, moved: false };
  };
  const onPointerMove = (e: ReactPointerEvent<HTMLDivElement>) => {
    const d = drag.current;
    if (d) {
      const dx = e.clientX - d.x;
      const dy = e.clientY - d.y;
      if (Math.abs(dx) + Math.abs(dy) > 4) d.moved = true;
      if (d.moved) {
        setPan(clampPan(d.px + dx, d.py + dy, zoom));
        setHover(null);
        return;
      }
    }
    if (!withPopup || !tiles) return;
    const c = coordsAt(e.clientX, e.clientY);
    const tile = c ? tiles[coordsId(c.x, c.y)] : undefined;
    const shell = shellRef.current?.getBoundingClientRect();
    if (!tile || !shell) {
      setHover(null);
      return;
    }
    setHover({ tile, left: e.clientX - shell.left, top: e.clientY - shell.top });
  };
  const onPointerUp = (e: ReactPointerEvent<HTMLDivElement>) => {
    const d = drag.current;
    drag.current = null;
    if (d?.moved || !onOpen || !tiles) return;
    const c = coordsAt(e.clientX, e.clientY);
    const tile = c ? tiles[coordsId(c.x, c.y)] : undefined;
    if (tile && (tile.kind === "owned" || tile.kind === "unowned" || tile.estateId)) onOpen(tileHref(tile, chainId), tile);
  };

  useEffect(() => {
    const el = shellRef.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      if (!e.ctrlKey && !e.metaKey) return;
      e.preventDefault();
      setZoom((z) => {
        const next = Math.max(ZOOM_MIN, Math.min(ZOOM_MAX, z * Math.exp(-e.deltaY * 0.0015)));
        setPan((p) => clampPan(p.x * (next / z), p.y * (next / z), next));
        return next;
      });
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  }, [clampPan]);

  const selectionSet = useMemo(() => new Set(selection.map((c) => coordsId(c.x, c.y))), [selection]);

  const cells = useMemo(() => {
    const sale: LandTile[] = [];
    const mine: { tile: LandTile; rented: boolean }[] = [];
    const picked: LandTile[] = [];
    if (tiles) {
      for (const tile of Object.values(tiles)) {
        const listed = (layers.onSale && tile.price !== null) || (layers.forRent && tile.rentalPricePerDay !== null);
        if (listed) sale.push(tile);
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
    return { sale, mine, picked };
  }, [tiles, layers.onSale, layers.forRent, layers.owned, layers.rented, selectionSet, isEstate, strictSelection]);

  const hovered = hover?.tile;
  const popupLeft = hover ? (hover.left > size.w - 260 ? hover.left - 250 : hover.left + 14) : 0;

  return (
    <div
      ref={shellRef}
      className={`shop-landmap ${className}`}
      role="region"
      aria-label={label}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerLeave={() => {
        drag.current = null;
        setHover(null);
      }}
    >
      <div
        ref={stageRef}
        className="shop-landmap__stage"
        style={{ width: square, height: square, transform: `translate(-50%, -50%) translate(${pan.x}px, ${pan.y}px) scale(${zoom})` }}
      >
        <MapSurface view="map" zoom={zoom} viewport={{ w: size.w, h: size.h, square, panX: pan.x, panY: pan.y }} />
        <svg className="shop-landmap__layer" viewBox={`0 0 ${GRID_PARCELS} ${GRID_PARCELS}`} preserveAspectRatio="none" aria-hidden="true">
          {cells.sale.map((t) => (
            <rect key={`s${t.id}`} className="shop-landmap__sale" x={cellX(t.x)} y={cellY(t.y)} width={1} height={1} />
          ))}
          {cells.mine.map(({ tile, rented }) => (
            <rect key={`o${tile.id}`} className={rented ? "shop-landmap__rented" : "shop-landmap__owned"} x={cellX(tile.x)} y={cellY(tile.y)} width={1} height={1} />
          ))}
          {cells.picked.map((t) => (
            <rect key={`p${t.id}`} className="shop-landmap__selected" x={cellX(t.x)} y={cellY(t.y)} width={1} height={1} />
          ))}
        </svg>
      </div>

      <div className="shop-landmap__zoom">
        <button type="button" aria-label="Zoom in" onClick={() => zoomTo(zoom * ZOOM_STEP)} disabled={zoom >= ZOOM_MAX}>
          +
        </button>
        <button type="button" aria-label="Zoom out" onClick={() => zoomTo(zoom / ZOOM_STEP)} disabled={zoom <= ZOOM_MIN}>
          −
        </button>
      </div>

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

      {hovered && withPopup ? (
        <div className="shop-landmap__popup" role="tooltip" style={{ left: popupLeft, top: hover!.top + 14 }}>
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
