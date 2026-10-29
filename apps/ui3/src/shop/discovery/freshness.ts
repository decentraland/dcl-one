
import { isShopApiError } from "../api/client";

export type IndexUnavailable = {
  kind: "unavailable";
  source: string | null;
  message: string;
  configure: string | null;
};

export type IndexFreshness = {
  kind: "fresh";
  asOf: number | null;
  lagSeconds: number | null;
  complete: boolean;
  source: string | null;
  staleAfterSeconds?: number;
};

export type Freshness = IndexUnavailable | IndexFreshness;

type Json = Record<string, unknown>;

function obj(v: unknown): Json | null {
  return v && typeof v === "object" && !Array.isArray(v) ? (v as Json) : null;
}

function epochMs(v: unknown): number | null {
  if (typeof v === "number" && Number.isFinite(v)) return v < 1e12 ? v * 1000 : v;
  if (typeof v === "string") {
    const at = Date.parse(v);
    return Number.isFinite(at) ? at : null;
  }
  return null;
}

export function indexUnavailable(error: unknown): IndexUnavailable | null {
  if (!isShopApiError(error) || error.status !== 503) return null;
  const body = obj(error.body);
  if (!body) return { kind: "unavailable", source: null, message: error.message, configure: null };
  return {
    kind: "unavailable",
    source: typeof body.source === "string" ? body.source : null,
    message: typeof body.message === "string" ? body.message : error.message,
    configure: typeof body.configure === "string" ? body.configure : null,
  };
}

export function readFreshness(body: unknown, now = Date.now()): IndexFreshness | null {
  const root = obj(body);
  const raw = obj(root?.freshness) ?? obj(obj(root?.meta)?.freshness) ?? (root && "completedAt" in root && "inProgress" in root ? root : null);
  if (!raw) {
    const asOf = typeof root?.as_of === "string" ? epochMs(root.as_of) : null;
    if (asOf === null) return null;
    const budget = root?.stale_after_secs;
    return {
      kind: "fresh",
      asOf,
      lagSeconds: Math.max(0, Math.round((now - asOf) / 1000)),
      complete: true,
      source: null,
      ...(typeof budget === "number" && Number.isFinite(budget) && budget >= 0 ? { staleAfterSeconds: budget } : {}),
    };
  }
  const asOf = epochMs(raw.asOf ?? raw.indexedAt ?? raw.completedAt);
  const lag = typeof raw.lagSeconds === "number" && Number.isFinite(raw.lagSeconds) ? raw.lagSeconds : null;
  const complete = typeof raw.complete === "boolean" ? raw.complete : raw.inProgress === true ? false : true;
  return { kind: "fresh", asOf, lagSeconds: lag, complete, source: typeof raw.source === "string" ? raw.source : null };
}

export function readMirrorFreshness(body: unknown): IndexFreshness | null {
  const data = obj(obj(body)?.data);
  if (!data || data.enabled === false) return null;
  const asOf = epochMs(data.completedAt);
  return { kind: "fresh", asOf, lagSeconds: null, complete: asOf !== null, source: "tradesMirror" };
}

export function needsNotice(f: Freshness | null | undefined, lagBudgetSeconds = 600): boolean {
  if (!f) return false;
  if (f.kind === "unavailable") return true;
  const budget = f.staleAfterSeconds ?? lagBudgetSeconds;
  return !f.complete || (f.lagSeconds !== null && f.lagSeconds > budget);
}

export function coarseDuration(ms: number): string {
  const s = Math.max(0, Math.round(ms / 1000));
  const units: [number, string][] = [
    [86_400, "day"],
    [3_600, "hour"],
    [60, "minute"],
  ];
  for (const [span, name] of units) {
    if (s >= span) {
      const n = Math.floor(s / span);
      return `${n} ${name}${n === 1 ? "" : "s"}`;
    }
  }
  return `${s} second${s === 1 ? "" : "s"}`;
}
