// Whether a captured page counts as identical, and which unanswered requests its spec declares. Kept
// apart from run.mts so the tests need neither a browser nor playwright-core.
import type { PageSpec, Side } from "../pages.mts";
import type { Miss } from "./recording.mts";
import type { PageResult } from "./run.mts";

/**
 * Pixel-identical (outside masks and deliberate regions), no unexpected structural difference, and no
 * request left unanswered (by the recording, or to a host the harness does not know) unless the page
 * declares it: two sides missing the same data render the same empty state, which compares nothing.
 */
export function isIdentical(r: Pick<PageResult, "pixels" | "structure" | "sides" | "misses" | "unknown">, tolerance = 0): boolean {
  if (r.sides.reference.error || r.sides["dcl-one"].error) return false;
  return r.pixels.diff <= tolerance && r.structure.every((d) => d.expected) && (r.misses ?? []).every((m) => m.expected) && (r.unknown ?? []).every((u) => u.expected);
}

/** A miss with the page's rule attached when one of its `unrecorded` entries covers it. */
export function classifyMiss(m: Miss, spec: Pick<PageSpec, "unrecorded">): Miss & { expected?: string } {
  const text = `${m.method} ${m.service} ${m.path} ${m.request === undefined ? "" : JSON.stringify(m.request)}`;
  const rule = spec.unrecorded?.find((u) => u.match.test(text));
  return rule ? { ...m, expected: rule.why } : m;
}

/** A request to an unknown host (`GET host/path`), with the page's rule attached when one covers it. */
export function classifyUnknown(side: Side, request: string, spec: Pick<PageSpec, "unrecorded">): PageResult["unknown"][number] {
  const rule = spec.unrecorded?.find((u) => u.match.test(request));
  return rule ? { side, request, expected: rule.why } : { side, request };
}
