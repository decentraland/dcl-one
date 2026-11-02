// from: shop@3a322cf9:app/src/lib/featureFlags.ts (getIsFeatureEnabled), app/src/hooks/useSuggestedForYouEnabled.ts
// The rollout switches upstream reads from Decentraland's feature-flags service, read from the same file
// under the same keys (`${app}-${feature}` in `dapps.json`), so turning one on turns it on in both Shops.
// A host points at another service with `window.__SHOP_FEATURE_FLAGS__`. A flag is off while the file
// loads, when it cannot be read, and when it is absent, as upstream's are.
//
// As upstream, the file lives in a module snapshot for 60s rather than in the query cache, so a page
// invalidating its queries (after a purchase) does not switch a flag off while it refetches. A read gets
// 3s and one retry; a failed refresh keeps the last answer, and a mount or focus 10s later tries again.
// Flags a host (or a test) primes are kept: no read replaces them.

import { useEffect, useSyncExternalStore } from "react";

const DEFAULT_SERVICE = "https://feature-flags.decentraland.org";
const APP = "dapps";
const TTL_MS = 60_000;
const TIMEOUT_MS = 3_000;
const BACKOFF_MS = 10_000;

export const REMOTE_FLAG = {
  /** Whether the Shop asks for personalised suggestions at all; the row still hides itself without a signal. */
  suggestedForYou: "shop-suggested-for-you",
} as const;

export type RemoteFlag = (typeof REMOTE_FLAG)[keyof typeof REMOTE_FLAG];

type FlagsWindow = Window & typeof globalThis & { __SHOP_FEATURE_FLAGS__?: string };
type Snapshot = { at: number; primed: boolean; flags: Readonly<Record<string, boolean>> };

let snapshot: Snapshot | null = null;
let inflight: Promise<void> | null = null;
let failedAt: number | null = null;
/** Bumped by a prime, so a read started before it cannot publish over it. */
let generation = 0;
const listeners = new Set<() => void>();

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

const getSnapshot = () => snapshot;

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

/**
 * Reads the file again unless the flags were primed, a fresh answer is held, a read just failed or one is running.
 * `now` is the clock for all of it: the answer's age and the backoff are both measured from it.
 */
export function refreshRemoteFlags(now = Date.now()): Promise<void> {
  if (snapshot?.primed || (snapshot && now - snapshot.at < TTL_MS)) return Promise.resolve();
  if (failedAt !== null && now - failedAt < BACKOFF_MS) return Promise.resolve();
  if (inflight) return inflight;
  const started = generation;
  // On `now`'s clock, but at the moment the read ends: a slow read must not age its own answer or shorten the backoff.
  const t0 = Date.now();
  const stamp = () => now + (Date.now() - t0);
  const read: Promise<void> = (async () => {
    for (let attempt = 0; attempt < 2; attempt++) {
      try {
        const flags = await fetchRemoteFlags(AbortSignal.timeout(TIMEOUT_MS));
        if (generation !== started) return;
        failedAt = null;
        publish({ at: stamp(), primed: false, flags });
        return;
      } catch {
        // Retried once; after that the last answer (or "off") stands until the backoff passes.
      }
    }
    if (generation === started) failedAt = stamp();
  })().finally(() => {
    if (inflight === read) inflight = null;
  });
  inflight = read;
  return read;
}

/** Hold these flags as the service's answer: a host that knows them, and tests, skip the read. Null forgets them. */
export function primeRemoteFlags(flags: Record<string, boolean> | null): void {
  generation++;
  failedAt = null;
  // A read already running belongs to the old generation; the next refresh starts its own.
  inflight = null;
  publish(flags ? { at: Date.now(), primed: true, flags: { ...flags } } : null);
}

/** One rollout flag; false until the service answers yes. */
export function useRemoteFlag(flag: RemoteFlag): boolean {
  const current = useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
  useEffect(() => {
    void refreshRemoteFlags();
    const onFocus = () => void refreshRemoteFlags();
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, []);
  return current?.flags[`${APP}-${flag}`] === true;
}
