import { getJSON, sendSignedJSON, type QueryParams, type RequestOpts } from "./client";
import { noteCopy } from "../copyAsOf";
import {
  PLACES_LIMIT,
  isRenderablePlace,
  isRenderablePlaceCategory,
  normalizePlace,
  normalizePlaceCategory,
  placeViewerOf,
  toPlaceView,
  toCategoryView,
} from "./places";
import type { PlaceView, CategoryView, PlaceViewer } from "./places";
import { field, isRecord, keepRow, keepRows, listOf } from "./rows";
import {
  CategoriesEnvelope,
  CategorySchema,
  ItemEnvelope,
  ListEnvelope,
  PlaceSchema,
  PlaceViewerSchema,
} from "./schemas/places";

export { PlaceSchema };

export async function fetchPlaces(
  params: QueryParams = {},
  opts: RequestOpts & { authenticated?: boolean } = {},
): Promise<PlaceView[]> {
  const env = await (opts.authenticated ? sendSignedJSON : getJSON)("/api/places", {
    method: "GET",
    service: "places",
    ...opts,
    query: { limit: PLACES_LIMIT, ...params, ...(opts.query ?? {}) },
  });
  noteCopy("places", env);
  const parsed = ListEnvelope.safeParse(env);
  return keepRows(
    parsed.success ? field(parsed.data, "data") : [],
    PlaceSchema,
    isRenderablePlace,
    (row) => toPlaceView(normalizePlace(row)),
  );
}

export async function fetchPlace(
  id?: string | null,
  opts: RequestOpts = {},
): Promise<PlaceView | null> {
  if (!id) return null;
  const env = await getJSON(`/api/places/${encodeURIComponent(id)}`, {
    service: "places",
    ...opts,
  });
  const parsed = ItemEnvelope.safeParse(env);
  const row = keepRow(
    parsed.success ? field(parsed.data, "data") : null,
    PlaceSchema,
    isRenderablePlace,
  );
  return row ? toPlaceView(normalizePlace(row)) : null;
}

export async function fetchPlaceViewer(id: string): Promise<PlaceViewer | null> {
  const env = await sendSignedJSON(`/api/places/${encodeURIComponent(id)}`, { method: "GET", service: "places" });
  const parsed = ItemEnvelope.safeParse(env);
  const row = PlaceViewerSchema.safeParse(parsed.success ? field(parsed.data, "data") : null);
  return row.success ? placeViewerOf(row.data) : null;
}

export async function fetchWorlds(
  params: QueryParams = {},
  opts: RequestOpts = {},
): Promise<PlaceView[]> {
  const env = await getJSON("/api/worlds", {
    service: "places",
    ...opts,
    query: { limit: PLACES_LIMIT, ...params, ...(opts.query ?? {}) },
  });
  noteCopy("worlds", env);
  const parsed = ListEnvelope.safeParse(env);
  return keepRows(
    listOf(parsed.success ? field(parsed.data, "data") : []).map(withParcels),
    PlaceSchema,
    isRenderablePlace,
    (row) => toPlaceView(normalizePlace(row)),
  );
}

function withParcels(row: unknown): unknown {
  return isRecord(row) && !Array.isArray(row.positions) ? { ...row, positions: [] } : row;
}

export async function fetchCategories(opts: RequestOpts = {}): Promise<CategoryView[]> {
  const env = await getJSON("/api/categories", { service: "places", ...opts });
  const parsed = CategoriesEnvelope.safeParse(env);
  return keepRows(
    parsed.success ? field(parsed.data, "data") : [],
    CategorySchema,
    isRenderablePlaceCategory,
    (row) => toCategoryView(normalizePlaceCategory(row)),
  );
}
