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
  /** Creators' running sales: discounted prices, the Best Deals rail and the deals filter (shop@94584a5c useCreatorSalesEnabled). */
  creatorSales: "shop-creator-sales",
  /**
   * The Marketplace inside the Shop: resales, bids, LAND, NAME resale, listing and selling, an item's owners and
   * sales, favorites lists. Shown only with this on AND a web3 wallet session (market/access.tsx). That only decides
   * what is drawn: the lock is the node's shop service reading the same key and refusing those calls while it is
   * off, so the flag is turned on only where the node does.
   */
  marketplaceFeatures: "shop-marketplace-features",
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

/** Bumped whenever the answer changes or a read gives up, so a gate waiting for an answer re-renders either way. */
let version = 0;

function publish(next: Snapshot | null): void {
  snapshot = next;
  version++;
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
    if (generation === started) {
      failedAt = stamp();
      version++;
      for (const l of listeners) l();
    }
  })().finally(() => {
    if (inflight === read) inflight = null;
  });
  inflight = read;
  return read;
}

/** Whether the flags were primed (by a server render, a host or a test) rather than read here. */
export function remoteFlagsPrimed(): boolean {
  return snapshot?.primed === true;
}

/**
 * Hold these flags as the service's answer: a host that knows them, and tests, skip the read. Null forgets them. A
 * primed answer is kept for the document's life (every server-rendered Shop page primes one): a flag switched off
 * upstream reaches an open tab on its next full load, not within the session.
 */
export function primeRemoteFlags(flags: Record<string, boolean> | null): void {
  generation++;
  failedAt = null;
  // A read already running belongs to the old generation; the next refresh starts its own.
  inflight = null;
  publish(flags ? { at: Date.now(), primed: true, flags: { ...flags } } : null);
}

/**
 * One rollout flag as last answered, outside React: false until an answer says yes. A page the server rendered has
 * its answer primed before its first render (sites primes the loader's flags on both sides), so the two agree.
 */
export function remoteFlag(flag: RemoteFlag): boolean {
  return snapshot?.flags[`${APP}-${flag}`] === true;
}

function useRefreshOnMount(): void {
  useEffect(() => {
    void refreshRemoteFlags();
    const onFocus = () => void refreshRemoteFlags();
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, []);
}

/** One rollout flag; false until the service answers yes. */
export function useRemoteFlag(flag: RemoteFlag): boolean {
  const current = useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
  useRefreshOnMount();
  return current?.flags[`${APP}-${flag}`] === true;
}

/**
 * One rollout flag, or null while nothing has answered yet: a gate that would otherwise send a visitor away can
 * wait for the answer instead. A read that gave up answers false, as a flag that cannot be read is off.
 */
export function remoteFlagState(flag: RemoteFlag): boolean | null {
  if (snapshot) return snapshot.flags[`${APP}-${flag}`] === true;
  return failedAt !== null ? false : null;
}

const getVersion = () => version;

export function useRemoteFlagState(flag: RemoteFlag): boolean | null {
  useSyncExternalStore(subscribe, getVersion, getVersion);
  useRefreshOnMount();
  return remoteFlagState(flag);
}
