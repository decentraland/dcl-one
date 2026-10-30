// from: marketplace@3bf3662f:webapp/src/components/AssetImage/AssetImage.tsx (PARCEL / ESTATE cases),
// webapp/src/components/Atlas/Atlas.tsx (selectedStrokeLayer, selectedFillLayer, Popup); decentraland-ui
// src/components/Atlas/Atlas.tsx; tile-map src/components/TileMap/TileMap.tsx (size 14 x zoom 0.5, drag bounds).
// Copyright 2020 Metaverse Holdings Ltd. (marketplace, decentraland-ui, tile-map), Apache-2.0.

import { useEffect, useMemo, useRef, useState, type CSSProperties, type PointerEvent as ReactPointerEvent } from "react";
import { useNavigate } from "react-router";

import { GRID_PARCELS } from "../../../explorer/components/MapSurface";
import { serviceHref } from "../../../data/catalyst/client";
import { shortAddress } from "../../model/address";
import { tileHref } from "../../land/LandMap";
import { Mana } from "../../land/format";
import { coordsId, isTileInSelection, selectionCenter, type Coord, type LandTile, type Tiles } from "../../land/tiles";
import "../../land/land.css";

const GRID_MIN = -170;
const GRID_MAX = GRID_MIN + GRID_PARCELS - 1;
const PARCEL_PX = 7;
const ATLAS_SRC = `${serviceHref("map")}/v1/map.png?width=2040&height=2040&size=6&center=-1,0&on-sale=false`;

const cellX = (x: number) => x - GRID_MIN;
const cellY = (y: number) => GRID_MAX - y;
const clampCoord = (v: number) => Math.max(GRID_MIN, Math.min(GRID_MAX, v));

type LandAtlasProps = {
  tiles: Tiles | null;
  selection: readonly Coord[];
  isEstate?: boolean;
  withPopup?: boolean;
  withNavigation?: boolean;
  chainId?: number;
  label?: string;
  className?: string;
};

type Hover = { tile: LandTile; left: number; top: number };

export default function LandAtlas({ tiles, selection, isEstate = false, withPopup = true, withNavigation = true, chainId, label = "LAND map", className = "" }: LandAtlasProps) {
  const navigate = useNavigate();
  const shellRef = useRef<HTMLDivElement | null>(null);
  const [size, setSize] = useState({ w: 0, h: 0 });
  const home = useMemo(() => (selection.length ? selectionCenter(selection) : { x: 0, y: 0 }), [selection]);
  const [center, setCenter] = useState<Coord>(home);
  const [hover, setHover] = useState<Hover | null>(null);
  const drag = useRef<{
    x: number;
    y: number;
    cx: number;
    cy: number;
    moved: boolean;
  } | null>(null);

  useEffect(() => setCenter(home), [home.x, home.y]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    const el = shellRef.current;
    if (!el) return;
    const measure = () => {
      const r = el.getBoundingClientRect();
      if (r.width > 0 && r.height > 0) setSize({ w: r.width, h: r.height });
    };
    measure();
    if (typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const selectionSet = useMemo(() => new Set(selection.map((c) => coordsId(c.x, c.y))), [selection]);
  const picked = useMemo(() => {
    const out = new Map<string, Coord>();
    for (const c of selection) out.set(coordsId(c.x, c.y), c);
    if (tiles && isEstate) {
      for (const tile of Object.values(tiles)) {
        if (!out.has(tile.id) && isTileInSelection(selectionSet, tiles, tile.x, tile.y, true, false)) out.set(tile.id, tile);
      }
    }
    return [...out.values()];
  }, [selection, selectionSet, tiles, isEstate]);

  const stageLeft = size.w / 2 - (cellX(center.x) + 0.5) * PARCEL_PX;
  const stageTop = size.h / 2 - (cellY(center.y) + 0.5) * PARCEL_PX;

  const tileAt = (clientX: number, clientY: number): LandTile | undefined => {
    const shell = shellRef.current?.getBoundingClientRect();
    if (!shell || !tiles) return undefined;
    const cx = Math.floor((clientX - shell.left - stageLeft) / PARCEL_PX);
    const cy = Math.floor((clientY - shell.top - stageTop) / PARCEL_PX);
    if (cx < 0 || cy < 0 || cx >= GRID_PARCELS || cy >= GRID_PARCELS) return undefined;
    return tiles[coordsId(GRID_MIN + cx, GRID_MAX - cy)];
  };

  const onPointerDown = (e: ReactPointerEvent<HTMLDivElement>) => {
    drag.current = { x: e.clientX, y: e.clientY, cx: center.x, cy: center.y, moved: false };
  };
  const onPointerMove = (e: ReactPointerEvent<HTMLDivElement>) => {
    const d = drag.current;
    if (d) {
      const dx = e.clientX - d.x;
      const dy = e.clientY - d.y;
      if (Math.abs(dx) + Math.abs(dy) > 4) d.moved = true;
      if (d.moved) {
        setCenter({ x: clampCoord(d.cx - dx / PARCEL_PX), y: clampCoord(d.cy + dy / PARCEL_PX) });
        setHover(null);
        return;
      }
    }
    if (!withPopup) return;
    const tile = tileAt(e.clientX, e.clientY);
    const shell = shellRef.current?.getBoundingClientRect();
    setHover(tile && shell && (tile.kind === "owned" || tile.kind === "unowned") ? { tile, left: e.clientX - shell.left, top: e.clientY - shell.top } : null);
  };
  const onPointerUp = (e: ReactPointerEvent<HTMLDivElement>) => {
    const d = drag.current;
    drag.current = null;
    if (d?.moved || !withNavigation) return;
    const tile = tileAt(e.clientX, e.clientY);
    if (tile && (tile.kind === "owned" || tile.kind === "unowned" || tile.estateId)) navigate(tileHref(tile, chainId));
  };

  const hovered = hover?.tile;
  const popupLeft = hover ? (hover.left > size.w - 260 ? hover.left - 250 : hover.left + 14) : 0;
  const popupTop = hover ? (hover.top > size.h - 120 ? hover.top - 110 : hover.top + 14) : 0;

  return (
    <div
      ref={shellRef}
      className={`shop-landatlas ${className}`}
      role="region"
      aria-label={label}
      data-testid="land-atlas"
      style={
        {
          "--atlas-x": `${stageLeft - 0.5}px`,
          "--atlas-y": `${stageTop - 0.5}px`,
        } as CSSProperties
      }
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerLeave={() => {
        drag.current = null;
        setHover(null);
      }}
    >
      {size.w > 0 ? (
        <div className="shop-landatlas__stage" style={{ width: GRID_PARCELS * PARCEL_PX, height: GRID_PARCELS * PARCEL_PX, transform: `translate(${stageLeft}px, ${stageTop}px)` }}>
          <img className="shop-landatlas__image" src={ATLAS_SRC} alt="" draggable={false} decoding="async" />
          <svg className="shop-landatlas__layer" viewBox={`0 0 ${GRID_PARCELS} ${GRID_PARCELS}`} preserveAspectRatio="none" aria-hidden="true">
            {picked.map((t) => (
              <rect key={`s${t.x},${t.y}`} className="shop-landatlas__stroke" x={cellX(t.x) - 0.2} y={cellY(t.y) - 0.2} width={1.4} height={1.4} />
            ))}
            {picked.map((t) => (
              <rect key={`f${t.x},${t.y}`} className="shop-landatlas__fill" x={cellX(t.x) - 0.1} y={cellY(t.y) - 0.1} width={1.2} height={1.2} />
            ))}
          </svg>
        </div>
      ) : null}

      {hovered && withPopup ? (
        <div className="shop-landmap__popup" role="tooltip" style={{ left: popupLeft, top: popupTop }}>
          <div className="shop-landmap__popuphead">
            <strong>{hovered.name || (hovered.estateId ? "Estate" : "Parcel")}</strong>
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
          {hovered.price !== null ? (
            <div className="shop-landmap__popuprow">
              <span>Price</span>
              <span className="shop-landmap__prices">
                <Mana text={hovered.price.toLocaleString("en-US", { maximumFractionDigits: 18 })} />
              </span>
            </div>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}
