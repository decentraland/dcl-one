// from: shop@3a322cf9:app/src/lib/featureFlags.ts (getIsFeatureEnabled), app/src/hooks/useSuggestedForYouEnabled.ts
// The browser host's hold on the rollout switches (rolloutFlags.ts reads and evaluates the file): which service it
// asks (`window.__SHOP_FEATURE_FLAGS__`, else Decentraland's), how long an answer lasts, and the hooks. A flag is
// off while the file loads, when it cannot be read, and when it is absent, as upstream's are.
//
// As upstream, the file lives in a module snapshot for 60s rather than in the query cache, so a page
// invalidating its queries (after a purchase) does not switch a flag off while it refetches. A read gets
// 3s and one retry; a failed refresh keeps the last answer, and a mount or focus 10s later tries again.
// Flags a host (or a test) primes are kept: no read replaces them. As upstream, a variant keeps only its string
// payload (an address list, for one).

import { useEffect, useSyncExternalStore } from "react";
import {
  FLAGS_SERVICE,
  readFlagsFile,
  remoteFlagOn,
  remoteFlagVariantIn,
  type FlagsTransport,
  type RemoteFlag,
  type RemoteFlagsAnswer,
} from "@shop/discovery/lib/rolloutFlags";

export { parseRemoteFlags, REMOTE_FLAG, type RemoteFlag, type RemoteFlagsAnswer } from "@shop/discovery/lib/rolloutFlags";

const TTL_MS = 60_000;
const TIMEOUT_MS = 3_000;
const BACKOFF_MS = 10_000;

type FlagsWindow = Window & typeof globalThis & { __SHOP_FEATURE_FLAGS__?: string };
type Snapshot = { at: number; primed: boolean; flags: Readonly<Record<string, boolean>>; variants: Readonly<Record<string, string>> };

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

/** The flags service this page asks: the host's, else Decentraland's. */
export function flagsService(): string {
  const declared = typeof window !== "undefined" ? (window as FlagsWindow).__SHOP_FEATURE_FLAGS__ : undefined;
  return (declared || FLAGS_SERVICE).replace(/\/$/, "");
}

/** Bumped whenever the answer changes or a read gives up, so a gate waiting for an answer re-renders either way. */
let version = 0;

function notify(): void {
  version++;
  for (const l of listeners) l();
}

function publish(next: Snapshot | null): void {
  snapshot = next;
  notify();
}

const fetchFlags: FlagsTransport = async (url, { signal }) => {
  const res = await fetch(url, { signal });
  if (!res.ok) {
    // An error's body is never read: it is let go without waiting on it (it may fail, or never end), so the status
    // refuses the answer at once.
    void res.body?.cancel().catch(() => undefined);
    return { status: res.status, body: "" };
  }
  return { status: res.status, body: await res.text() };
};

export async function readRemoteFlags(signal?: AbortSignal): Promise<RemoteFlagsAnswer> {
  return readFlagsFile(flagsService(), fetchFlags, signal);
}

export async function fetchRemoteFlags(signal?: AbortSignal): Promise<Record<string, boolean>> {
  return (await readRemoteFlags(signal)).flags;
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
        const { flags, variants } = await readRemoteFlags(AbortSignal.timeout(TIMEOUT_MS));
        if (generation !== started) return;
        failedAt = null;
        publish({ at: stamp(), primed: false, flags, variants });
        return;
      } catch {
        // Retried once; after that the last answer (or "off") stands until the backoff passes.
      }
    }
    if (generation === started) {
      // The answer stays as it was, but a gate waiting for one moves on: an unread flag is off.
      failedAt = stamp();
      notify();
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
 * upstream reaches an open tab on its next full load, not within the session. Variants not given are none.
 */
export function primeRemoteFlags(flags: Record<string, boolean> | null, variants: Record<string, string> = {}): void {
  generation++;
  failedAt = null;
  // A read already running belongs to the old generation; the next refresh starts its own.
  inflight = null;
  publish(flags ? { at: Date.now(), primed: true, flags: { ...flags }, variants: { ...variants } } : null);
}

/** A rollout flag's variant payload as last answered (upstream's getAddressListVariant reads it), or null without one. */
export function remoteFlagVariant(flag: RemoteFlag): string | null {
  return remoteFlagVariantIn(snapshot, flag);
}

/**
 * One rollout flag as last answered, outside React: false until an answer says yes. A page the server rendered has
 * its answer primed before its first render (sites primes the loader's flags on both sides), so the two agree.
 */
export function remoteFlag(flag: RemoteFlag): boolean {
  return remoteFlagOn(snapshot, flag);
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
  return remoteFlagOn(current, flag);
}

/**
 * One rollout flag, or null while nothing has answered yet: a gate that would otherwise send a visitor away can
 * wait for the answer instead. A read that gave up answers false, as a flag that cannot be read is off.
 */
export function remoteFlagState(flag: RemoteFlag): boolean | null {
  if (snapshot) return remoteFlagOn(snapshot, flag);
  return failedAt !== null ? false : null;
}

const getVersion = () => version;

export function useRemoteFlagState(flag: RemoteFlag): boolean | null {
  useSyncExternalStore(subscribe, getVersion, getVersion);
  useRefreshOnMount();
  return remoteFlagState(flag);
}
