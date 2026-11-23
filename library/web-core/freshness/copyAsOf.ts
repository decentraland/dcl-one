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
