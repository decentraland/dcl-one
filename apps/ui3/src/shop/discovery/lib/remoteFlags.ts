// from: shop@3a322cf9:app/src/lib/featureFlags.ts (getIsFeatureEnabled), app/src/hooks/useSuggestedForYouEnabled.ts
// The rollout switches upstream reads from Decentraland's feature-flags service, read from the same file
// under the same keys (`${app}-${feature}` in `dapps.json`), so turning one on turns it on in both Shops.
// A host points at another service with `window.__SHOP_FEATURE_FLAGS__`. A flag is off while the file
// loads, when it cannot be read, and when it is absent, as upstream's are.
//
// As upstream, the file lives in a module snapshot for 60s rather than in the query cache, so a page
// invalidating its queries (after a purchase) does not switch a flag off while it refetches. A read gets
// 3s and one retry; a failed refresh keeps the last answer, and the next mount or focus tries again.

import { useEffect, useSyncExternalStore } from "react";

const DEFAULT_SERVICE = "https://feature-flags.decentraland.org";
const APP = "dapps";
const TTL_MS = 60_000;
const TIMEOUT_MS = 3_000;

export const REMOTE_FLAG = {
  /** Whether the Shop asks for personalised suggestions at all; the row still hides itself without a signal. */
  suggestedForYou: "shop-suggested-for-you",
} as const;

export type RemoteFlag = (typeof REMOTE_FLAG)[keyof typeof REMOTE_FLAG];

type FlagsWindow = Window & typeof globalThis & { __SHOP_FEATURE_FLAGS__?: string };
type Snapshot = { at: number; flags: Readonly<Record<string, boolean>> };

let snapshot: Snapshot | null = null;
let inflight: Promise<void> | null = null;
const listeners = new Set<() => void>();

function service(): string {
  const declared = typeof window !== "undefined" ? (window as FlagsWindow).__SHOP_FEATURE_FLAGS__ : undefined;
  return (declared || DEFAULT_SERVICE).replace(/\/$/, "");
}

function publish(next: Snapshot | null): void {
  snapshot = next;
  for (const l of listeners) l();
}

export async function fetchRemoteFlags(signal?: AbortSignal): Promise<Record<string, boolean>> {
  const res = await fetch(`${service()}/${APP}.json`, { signal });
  if (!res.ok) throw new Error(`feature flags: HTTP ${res.status}`);
  const body = (await res.json()) as { flags?: Record<string, unknown> } | null;
  const flags: Record<string, boolean> = {};
  for (const [key, value] of Object.entries(body?.flags ?? {})) flags[key] = value === true;
  return flags;
}

/** Reads the file again unless a fresh answer is held or a read is already running. */
export function refreshRemoteFlags(now = Date.now()): Promise<void> {
  if (snapshot && now - snapshot.at < TTL_MS) return Promise.resolve();
  if (inflight) return inflight;
  inflight = (async () => {
    for (let attempt = 0; attempt < 2; attempt++) {
      try {
        publish({ at: Date.now(), flags: await fetchRemoteFlags(AbortSignal.timeout(TIMEOUT_MS)) });
        return;
      } catch {
        // Retried once; after that the last answer (or "off") stands until the next mount or focus.
      }
    }
  })().finally(() => {
    inflight = null;
  });
  return inflight;
}

/** Hold these flags as the service's answer: a host that knows them, and tests, skip the read. Null forgets. */
export function primeRemoteFlags(flags: Record<string, boolean> | null): void {
  publish(flags ? { at: Date.now(), flags: { ...flags } } : null);
}

/** One rollout flag; false until the service answers yes. */
export function useRemoteFlag(flag: RemoteFlag): boolean {
  const current = useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => snapshot,
    () => snapshot,
  );
  useEffect(() => {
    void refreshRemoteFlags();
    const onFocus = () => void refreshRemoteFlags();
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, []);
  return current?.flags[`${APP}-${flag}`] === true;
}
