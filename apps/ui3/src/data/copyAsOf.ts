import { useSyncExternalStore } from "react";

import { olderCopy, readCopy, type CopyStamp } from "@web-core/freshness/copyAsOf";

export { olderCopy, readCopy, type CopyStamp } from "@web-core/freshness/copyAsOf";

export type CopyDomain = "places" | "worlds" | "events" | "communities";

const stamps = new Map<CopyDomain, CopyStamp | null>();
const listeners = new Set<() => void>();
let version = 0;

export function noteCopy(domain: CopyDomain, body: unknown): void {
  const copy = readCopy(body);
  const seen = stamps.get(domain) ?? null;
  if (seen?.asOf === copy?.asOf && seen?.staleAfterMs === copy?.staleAfterMs) return;
  stamps.set(domain, copy);
  version += 1;
  for (const listener of listeners) listener();
}

export function copyAsOf(domains: readonly CopyDomain[]): CopyStamp | null {
  return domains.reduce<CopyStamp | null>((copy, domain) => olderCopy(copy, stamps.get(domain)), null);
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useCopyAsOf(domains: readonly CopyDomain[]): CopyStamp | null {
  const live = useSyncExternalStore(subscribe, () => version, () => -1) === -1;
  return live ? null : copyAsOf(domains);
}

export function resetCopies(): void {
  stamps.clear();
  version += 1;
  for (const listener of listeners) listener();
}
