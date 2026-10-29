// from: marketplace@f3956826:webapp/src/modules/proximity/{sagas,utils,types,hooks}.ts,
// indexer/scripts/calculateProximities.ts (the metric)
// Copyright 2020 Metaverse Holdings Ltd. Licensed under the Apache License, Version 2.0.
// FETCH_PROXIMITY_REQUEST: how far a parcel or an estate is from a road, a plaza and a district.
// Upstream shipped a precomputed webapp/public/proximity.json built from the tile grid; here the same
// numbers are derived from the tiles the LAND map already loaded, with the generator's metric
// (Manhattan distance minus one, so an adjacent tile is 0) and its cut-off (only distances under 10).

import type { Coord, LandTile, Tiles } from "./tiles";

export type Proximity = { district?: number; road?: number; plaza?: number };
export type ProximityKind = keyof Proximity;

export const PROXIMITY_MAX_DISTANCE = 10;
const KINDS: ProximityKind[] = ["plaza", "road", "district"];

export type ProximityIndex = Record<ProximityKind, Coord[]>;

/** The special tiles of the grid, grouped by kind, once per grid. */
export function proximityIndex(tiles: Tiles): ProximityIndex {
  const index: ProximityIndex = { plaza: [], road: [], district: [] };
  for (const tile of Object.values(tiles)) {
    if (tile.kind === "plaza" || tile.kind === "road" || tile.kind === "district") index[tile.kind].push({ x: tile.x, y: tile.y });
  }
  return index;
}

const distance = (a: Coord, b: Coord) => {
  const d = Math.abs(a.x - b.x) + Math.abs(a.y - b.y);
  return d === 0 ? 0 : d - 1;
};

function minDistance(from: Coord, targets: readonly Coord[]): number | undefined {
  let best: number | undefined;
  for (const t of targets) {
    const d = distance(from, t);
    if (best === undefined || d < best) best = d;
    if (best === 0) break;
  }
  return best;
}

/** A parcel's proximity; undefined when nothing special is within reach. */
export function parcelProximity(parcel: Coord, index: ProximityIndex, self?: LandTile): Proximity | undefined {
  if (self && (self.kind === "road" || self.kind === "plaza")) return undefined;
  const out: Proximity = {};
  for (const kind of KINDS) {
    const d = minDistance(parcel, index[kind]);
    if (d !== undefined && d < PROXIMITY_MAX_DISTANCE) out[kind] = d;
  }
  return Object.keys(out).length ? out : undefined;
}

/** An estate's proximity: for each kind, its closest parcel's (upstream `getEstateProximity`). */
export function estateProximity(parcels: readonly Coord[], index: ProximityIndex): Proximity | undefined {
  let out: Proximity | undefined;
  for (const parcel of parcels) {
    const p = parcelProximity(parcel, index);
    if (!p) continue;
    out ??= {};
    for (const kind of KINDS) {
      const d = p[kind];
      if (d !== undefined && (out[kind] === undefined || d < out[kind]!)) out[kind] = d;
    }
  }
  return out;
}

/** "Adjacent" or "N parcels away" (upstream `getDistanceText`). */
export function distanceText(d: number): string {
  if (d === 0) return "Adjacent";
  return `${d} ${d === 1 ? "parcel" : "parcels"} away`;
}

export const PROXIMITY_TITLE: Record<ProximityKind, string> = { plaza: "Plaza", road: "Road", district: "District" };
