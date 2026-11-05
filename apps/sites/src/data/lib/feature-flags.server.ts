// from: shop@94584a5c:app/src/lib/featureFlags.ts (getIsFeatureEnabled: `${FEATURE_FLAGS_URL}/dapps.json`, a flag on
//       only when the file says `true`)
// The `dapps` rollout flags decentraland.org's sites read, read here by the server. The service answers by the site a
// browser's request comes from (its Referer), so a browser on any other domain (a preview, this node's own) would be
// told every flag is off: the server asks as decentraland.org (or the site FEATURE_FLAGS_ORIGIN names) and hands the
// answer to the page. One read a minute whatever the request; a failed or slow one keeps the last good answer for that
// minute ("all off" only before any read has succeeded).

import { withDeadline } from "./request-deadline";
import { ttlMemo } from "./ttl-memo";

const DEFAULT_SERVICE = "https://feature-flags.decentraland.org";
const DEFAULT_SITE = "https://decentraland.org";
const TIMEOUT_MS = 1_500;

export type DappsFlags = Readonly<Record<string, boolean>>;

function service(): string {
  return (process.env.FEATURE_FLAGS_URL || DEFAULT_SERVICE).replace(/\/+$/, "");
}

function site(): string {
  return (process.env.FEATURE_FLAGS_ORIGIN || DEFAULT_SITE).replace(/\/+$/, "");
}

const lastGood = new Map<string, DappsFlags>();

async function load(origin: string): Promise<DappsFlags> {
  try {
    const flags = await withDeadline(async (signal) => {
      const res = await fetch(`${service()}/dapps.json`, { headers: { Referer: `${origin}/` }, signal });
      if (!res.ok) {
        await res.body?.cancel().catch(() => {});
        throw new Error(`feature flags answered ${res.status}`);
      }
      const body = (await res.json()) as { flags?: Record<string, unknown> } | null;
      const flags: Record<string, boolean> = {};
      for (const [key, value] of Object.entries(body?.flags ?? {})) flags[key] = value === true;
      return flags;
    }, TIMEOUT_MS);
    lastGood.set(origin, flags);
    return flags;
  } catch {
    return lastGood.get(origin) ?? {};
  }
}

const read = ttlMemo({ ttlMs: 60_000, keyOf: (origin: string) => origin, load });

/** decentraland.org's `dapps` flags, each true only where the service says so. */
export function dappsFlags(): Promise<DappsFlags> {
  return read(site());
}

export const resetDappsFlags = (): void => {
  read.reset();
  lastGood.clear();
};
