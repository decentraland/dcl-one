// from: marketplace@f3956826:webapp/src/modules/tile/sagas.ts, webapp/src/modules/tile/reducer.ts,
// webapp/src/modules/vendor/decentraland/land/api.ts, webapp/src/components/Atlas/Atlas.utils.ts,
// webapp/src/components/AssetBrowse/MapBrowse/utils.ts, webapp/src/modules/nft/estate/utils.ts (getSelection, getCenter)
// Copyright 2020 Metaverse Holdings Ltd. Licensed under the Apache License, Version 2.0.
// FETCH_TILES_REQUEST: the atlas grid the LAND map draws, from the map service's legacy `/v1/tiles`
// (v2 serves it from its mirrored api.decentraland.org grid). A grid that is not mirrored yet answers
// 503 "Not ready": that is an outage of the map, never an empty Genesis City.

import { getJSON, serviceBase, CatalystError, type RequestOpts } from "../../data/catalyst/client";

export type TileKind = "owned" | "unowned" | "road" | "plaza" | "district";

export type LandTile = {
  id: string;
  x: number;
  y: number;
  kind: TileKind;
  top: boolean;
  left: boolean;
  topLeft: boolean;
  owner: string | null;
  name: string | null;
  estateId: string | null;
  /** The live sale price in MANA (the map service's number), null when not on sale. */
  price: number | null;
  /** The highest price per day of an open rental listing, in MANA wei; null when not for rent. */
  rentalPricePerDay: string | null;
};

export type Tiles = Record<string, LandTile>;

/** The legacy numeric tile types (decentraland-ui AtlasTileType), as the map service's `/v1/tiles` sends them. */
const LEGACY_KIND: Record<number, TileKind> = { 5: "district", 7: "road", 8: "plaza", 9: "owned", 10: "owned", 11: "unowned" };

type LegacyTile = {
  type?: number;
  x?: number;
  y?: number;
  top?: number | boolean;
  left?: number | boolean;
  topLeft?: number | boolean;
  owner?: string;
  name?: string;
  estate_id?: string;
  price?: number;
  rentalPricePerDay?: string;
};

export const coordsId = (x: number | string, y: number | string) => `${x},${y}`;

export function parseLegacyTile(id: string, raw: unknown): LandTile | null {
  if (!raw || typeof raw !== "object") return null;
  const t = raw as LegacyTile;
  const [xs, ys] = id.split(",");
  const x = typeof t.x === "number" ? t.x : Number(xs);
  const y = typeof t.y === "number" ? t.y : Number(ys);
  if (!Number.isSafeInteger(x) || !Number.isSafeInteger(y)) return null;
  const kind = LEGACY_KIND[t.type ?? 11] ?? "unowned";
  const price = typeof t.price === "number" && Number.isFinite(t.price) && t.price > 0 ? t.price : null;
  const rent = typeof t.rentalPricePerDay === "string" && /^[0-9]+$/.test(t.rentalPricePerDay) && t.rentalPricePerDay !== "0" ? t.rentalPricePerDay : null;
  return {
    id: coordsId(x, y),
    x,
    y,
    kind,
    top: !!t.top,
    left: !!t.left,
    topLeft: !!t.topLeft,
    owner: typeof t.owner === "string" && t.owner ? t.owner.toLowerCase() : null,
    name: typeof t.name === "string" && t.name ? t.name : null,
    estateId: typeof t.estate_id === "string" && t.estate_id ? t.estate_id : null,
    price,
    rentalPricePerDay: rent,
  };
}

export type TilesResult = { tiles: Tiles; lastModified: Date | null };

export class MapNotReadyError extends Error {
  override name = "MapNotReadyError";
  constructor() {
    super("The LAND map is catching up and cannot be shown yet.");
  }
}

/** The whole atlas grid (upstream `atlasAPI.fetchTiles`). */
export async function fetchTiles(opts: RequestOpts = {}): Promise<TilesResult> {
  let body: { data?: Record<string, unknown> };
  const fetchImpl = opts.fetchImpl ?? ((...a: Parameters<typeof fetch>) => fetch(...a));
  let lastModified: Date | null = null;
  try {
    body = await getJSON<{ data?: Record<string, unknown> }>("/v1/tiles", {
      service: "map",
      ...opts,
      fetchImpl: async (input, init) => {
        const res = await fetchImpl(input, init);
        const stamp = res.headers.get("last-modified");
        if (stamp) lastModified = new Date(stamp);
        return res;
      },
    });
  } catch (e) {
    if (e instanceof CatalystError && e.status === 503) throw new MapNotReadyError();
    throw e;
  }
  const tiles: Tiles = {};
  for (const [id, raw] of Object.entries(body.data ?? {})) {
    const tile = parseLegacyTile(id, raw);
    if (tile) tiles[tile.id] = tile;
  }
  return { tiles, lastModified };
}

export function tilesUrl(): string {
  return `${serviceBase("map")}/v1/tiles`;
}

export function tilesByEstateId(tiles: Tiles): Record<string, LandTile[]> {
  const out: Record<string, LandTile[]> = {};
  for (const tile of Object.values(tiles)) {
    if (!tile.estateId) continue;
    (out[tile.estateId] ??= []).push(tile);
  }
  return out;
}

export type Coord = { x: number; y: number };

const manhattan = (a: Coord, b: Coord) => Math.abs(a.x - b.x) + Math.abs(a.y - b.y);

/** The coordinate of `coords` nearest to `from` (MapBrowse's re-centring on a filter change). */
export function nearestTile(from: Coord, coords: readonly Coord[]): Coord | undefined {
  let best: Coord | undefined;
  let bestDistance = Infinity;
  for (const c of coords) {
    const d = manhattan(from, c);
    if (d < bestDistance) {
      best = c;
      bestDistance = d;
    }
  }
  return best;
}

/**
 * Whether a coordinate is highlighted as part of `selection`. An indexed estate selection can be
 * incomplete, so it expands to every tile sharing the centre's estate id, unless `strict` says the
 * selection is the registry's authoritative composition.
 */
export function isTileInSelection(selection: ReadonlySet<string>, tiles: Tiles | null, x: number, y: number, isEstate: boolean, strict: boolean): boolean {
  if (selection.has(coordsId(x, y))) return true;
  if (strict || !tiles) return false;
  const first = selection.values().next().value as string | undefined;
  if (!first) return false;
  const center = tiles[first];
  const tile = tiles[coordsId(x, y)];
  return !!(center && tile && center.estateId && tile.estateId && center.estateId === tile.estateId && isEstate);
}

/** The parcels an estate NFT spans, as coordinates. */
export function estateSelection(parcels: readonly { x: number | string; y: number | string }[]): Coord[] {
  return parcels.map((p) => ({ x: Number(p.x), y: Number(p.y) }));
}

/** The middle of a selection, per axis (upstream `getCenter`). */
export function selectionCenter(selection: readonly Coord[]): Coord {
  if (selection.length === 0) return { x: 0, y: 0 };
  const xs = [...new Set(selection.map((c) => c.x))].sort((a, b) => a - b);
  const ys = [...new Set(selection.map((c) => c.y))].sort((a, b) => a - b);
  return { x: xs[(xs.length / 2) | 0]!, y: ys[(ys.length / 2) | 0]! };
}

/** LAND token ids pack x and y as two's-complement int128 halves (upstream `decodeLandTokenId`). */
export function decodeLandTokenId(tokenId: string | bigint): Coord {
  const id = BigInt(tokenId);
  const mask = (1n << 128n) - 1n;
  const signed = (half: bigint) => Number(BigInt.asIntN(128, half));
  return { x: signed((id >> 128n) & mask), y: signed(id & mask) };
}
