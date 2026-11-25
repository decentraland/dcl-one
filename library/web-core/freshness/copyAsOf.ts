import { coarseDuration } from "./coarseDuration";

export type CopyStamp = { asOf: number; staleAfterMs: number };

export function readCopy(body: unknown): CopyStamp | null {
  if (!body || typeof body !== "object" || Array.isArray(body)) return null;
  const { as_of: raw, stale_after_secs: budget } = body as { as_of?: unknown; stale_after_secs?: unknown };
  if (typeof raw !== "string") return null;
  const asOf = Date.parse(raw);
  if (!Number.isFinite(asOf)) return null;
  const staleAfterMs = typeof budget === "number" && Number.isFinite(budget) && budget >= 0 ? budget * 1000 : 0;
  return { asOf, staleAfterMs };
}

export function olderCopy(a: CopyStamp | null | undefined, b: CopyStamp | null | undefined): CopyStamp | null {
  if (!a) return b ?? null;
  if (!b) return a;
  const asOf = Math.min(a.asOf, b.asOf);
  const staleAt = Math.min(a.asOf + a.staleAfterMs, b.asOf + b.staleAfterMs);
  return { asOf, staleAfterMs: staleAt - asOf };
}

export function staleCopyAge(copy: CopyStamp | null | undefined, now = Date.now()): string | null {
  if (!copy || now - copy.asOf <= copy.staleAfterMs) return null;
  return coarseDuration(now - copy.asOf);
}

export type CopyDomain = "places" | "worlds" | "events" | "communities";

// The page's newest stamp per domain, noted by the readers that fetch it; one store per page.
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

/** Calls `listener` whenever a noted stamp changes; returns the unsubscribe. */
export function subscribeCopies(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** Increments on every change, for an external-store snapshot. */
export function copiesVersion(): number {
  return version;
}

export function resetCopies(): void {
  stamps.clear();
  version += 1;
  for (const listener of listeners) listener();
}
