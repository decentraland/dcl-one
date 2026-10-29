import { apiSource } from "../api";
import { dbSource } from "../db.server";
import { CatalystError } from "../client";
import type { GetOptions } from "../client";
import type { Place, Category } from "../schema";
import type { FetchPlacesParams } from "../types";
import type { CopyStamp } from "@ui/data/copyAsOf";

function dbConfigured(): boolean {
  if (typeof process === "undefined") return false;
  return Boolean(
    process.env.CATALYST_DATABASE_URL ||
      process.env.PLACES_PG_COMPONENT_PSQL_CONNECTION_STRING,
  );
}

let warned = false;
function warnFallback(err: unknown): void {
  if (warned) return;
  warned = true;
  console.warn(
    "[catalyst] direct DB read failed; falling back to HTTP API:",
    err instanceof Error ? err.message : err,
  );
}

const COPY_PROBE_TTL_MS = 60_000;
/** Hang guard: the rows never wait on the stamp longer than this. */
const COPY_PROBE_TIMEOUT_MS = 5_000;
let copyProbe: { at: number; value: Promise<CopyStamp | null> } | null = null;

/** The places copy's stamp for rows read straight from the database: the API stamps the same rows
 *  (`as_of`, `stale_after_secs`), so one small read of it, reused for a minute, says how old they are
 *  and when the node calls them stale. Null when the API answers live or not at all; only an answer
 *  is reused, so a failed probe is tried again by the next request. */
export function placesCopy(opts: GetOptions = {}, now = Date.now()): Promise<CopyStamp | null> {
  if (!copyProbe || now - copyProbe.at >= COPY_PROBE_TTL_MS) {
    // Shared by every request in the minute: no one request's signal may cancel it, only its guard.
    const guard = new AbortController();
    const timer = setTimeout(() => guard.abort(), COPY_PROBE_TIMEOUT_MS);
    const probe = apiSource
      .fetchPlaces({ limit: 1 }, { ...opts, signal: guard.signal })
      .finally(() => clearTimeout(timer));
    const entry = {
      at: now,
      value: probe.then(
        (res) => res.copy ?? null,
        () => {
          if (copyProbe === entry) copyProbe = null;
          return null;
        },
      ),
    };
    copyProbe = entry;
  }
  return copyProbe.value;
}

export async function loadPlaces(
  params: FetchPlacesParams = {},
  opts: GetOptions = {},
): Promise<{ data: Place[]; total: number; copy?: CopyStamp | null }> {
  if (dbConfigured()) {
    try {
      const [res, copy] = await Promise.all([dbSource.fetchPlaces(params), placesCopy(opts)]);
      return { ...res, copy };
    } catch (err) {
      warnFallback(err);
    }
  }
  return apiSource.fetchPlaces(params, opts);
}

export async function loadPlace(
  id: string,
  opts: GetOptions = {},
): Promise<Place | null> {
  if (dbConfigured()) {
    try {
      return await dbSource.fetchPlace(id);
    } catch (err) {
      // A miss is not a failure: the API also resolves derive-era parcel ids the table no
      // longer carries, so it gets the last word on whether the place exists.
      if (!(err instanceof CatalystError && err.status === 404)) warnFallback(err);
    }
  }
  return apiSource.fetchPlace(id, opts);
}

const CATEGORIES_TTL_MS = 5 * 60_000;
let categoriesCache: { at: number; value: Category[] } | null = null;

export async function loadCategories(
  opts: GetOptions = {},
): Promise<Category[]> {
  if (categoriesCache && Date.now() - categoriesCache.at < CATEGORIES_TTL_MS) {
    return categoriesCache.value;
  }
  if (dbConfigured()) {
    try {
      const value = await dbSource.fetchCategories();
      categoriesCache = { at: Date.now(), value };
      return value;
    } catch (err) {
      warnFallback(err);
    }
  }
  const value = await apiSource.fetchCategories(opts);
  if (value.length > 0) categoriesCache = { at: Date.now(), value };
  return value;
}
