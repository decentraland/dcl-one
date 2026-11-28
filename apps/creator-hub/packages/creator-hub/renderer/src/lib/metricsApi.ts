import { localStorageGetIdentity } from '@dcl/single-sign-on-client';

import { buildMetricsUrl } from '/shared/urls';
import { chunk } from '/shared/utils';

import { metrics } from '#preload';
import { config } from '/@/config';

import { AuthServerProvider } from './auth';
import { signedHeaders } from './signedFetch';

const METRICS_API_URL = config.get('METRICS_API_URL');

/** The service answers a 400 above this many locations. */
export const MAX_LOCATIONS_PER_REQUEST = 100;

const PATH = '/metrics';

/** A world scene (`world` set) or a Genesis City scene. Coordinates are required for both. */
export type MetricLocation = {
  /** A valid ENS name — both `foo.dcl.eth` and plain `foo.eth` are real. */
  world?: string;
  x: number;
  y: number;
};

/** One row of the metrics bag: a value for a metric, optionally per series and period. */
export type MetricRow = {
  /** `null` for the five metrics that have a single series. */
  series: string | null;
  /** Week-start Monday (`2026-06-15`) for the `*_weekly` metrics; `null` for `_30d`/`_60d`/`_range`. */
  period: string | null;
  value: number;
};

/**
 * An inclusive span of UTC days, each `YYYY-MM-DD`. Sent, it makes the service answer only the
 * `*_range` metrics, summed over those days, in place of its fixed windows.
 */
export type DayRange = { from: string; to: string };

export type LocationMetrics = MetricLocation & {
  /** The service's own key, for its logs. Never built or parsed here — see `readPositionally`. */
  location_key: string;
  /** Enrichment, populated for roughly a third of locations. Not a key. */
  builder_project_id: string | null;
  /**
   * Empty means either "you may not read this" or "no rows in today's export",
   * deliberately indistinguishable. Renders as "no data yet", never as an error.
   */
  metrics: Record<string, MetricRow[]>;
};

export type MetricsBatch = {
  /** The warehouse's export stamp, not our load time. Present even when every location is empty. */
  exported_at: string;
  source: string;
  /** The span the metrics were summed over, echoed back; absent when none was asked for. */
  range?: DayRange;
  locations: LocationMetrics[];
};

/**
 * ADR-44 signed-fetch headers for a request the main process will make.
 *
 * The signed payload is `<method>:<pathname>:<timestamp>:<metadata>`, with the method and the
 * pathname in lower case. The body is not part of it, and the pathname comes from the
 * same helper main uses to resolve the URL, so any query string on `path` is
 * left out.
 *
 * The account and its identity are read before the first `await`, so a caller that checks the
 * signed-in account right before the call gets the signature of that account.
 *
 * @param metadata - signed with the payload and sent as `x-identity-metadata`, e.g. a body's `hashPayload`.
 * @throws when no account is connected or it has no stored identity.
 */
export async function buildSignedHeaders(
  baseUrl: string,
  path: string,
  method: 'get' | 'post' | 'put' = 'post',
  metadata: Record<string, string> = {},
): Promise<Record<string, string>> {
  const address = AuthServerProvider.getAccount();
  if (!address) throw new Error('No connected account found');

  const identity = localStorageGetIdentity(address);
  if (!identity) throw new Error('No identity found');

  const { pathname } = buildMetricsUrl(baseUrl, path);
  const headers = await signedHeaders(identity, method, pathname, metadata);
  return Object.fromEntries(headers.entries());
}

/**
 * Pairs each answered entry with the location that position asked for.
 *
 * The response is parallel to the request, duplicates included, and
 * `location_key` is for the service's logs — matching on it would be ambiguous
 * the moment the same location is requested twice. So identity comes from what
 * we sent, and only the metrics come from what came back.
 */
function readPositionally(
  requested: MetricLocation[],
  answered: LocationMetrics[],
): LocationMetrics[] {
  if (answered.length !== requested.length) {
    throw new Error(
      `The analytics API answered ${answered.length} of ${requested.length} locations, so the response cannot be read positionally`,
    );
  }
  return requested.map((location, index) => {
    const { location_key, builder_project_id, metrics: bag } = answered[index];
    return { ...location, location_key, builder_project_id, metrics: bag };
  });
}

/**
 * Every metric the service holds for the given locations, in one batched call
 * per 100 locations.
 *
 * Requests run in the main process: Electron's renderer sends `Origin: null`,
 * which the service's CORS allow-list rejects, so a renderer-side fetch never
 * sees the response.
 *
 * @param range - when given, the service answers only the `*_range` metrics, summed over these days.
 */
export async function fetchMetrics(
  locations: MetricLocation[],
  range?: DayRange,
): Promise<MetricsBatch> {
  const batches = await Promise.all(
    chunk(locations, MAX_LOCATIONS_PER_REQUEST).map(async group => {
      const response = await metrics.request<MetricsBatch>({
        baseUrl: METRICS_API_URL,
        path: PATH,
        headers: await buildSignedHeaders(METRICS_API_URL, PATH),
        body: range ? { locations: group, range } : { locations: group },
      });
      if (!response.ok) throw new Error(response.error);
      return { ...response.data, locations: readPositionally(group, response.data.locations) };
    }),
  );

  const [first] = batches;
  return {
    exported_at: first.exported_at,
    source: first.source,
    ...(first.range && { range: first.range }),
    locations: batches.flatMap(batch => batch.locations),
  };
}
