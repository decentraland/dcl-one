// from: shop@94584a5c:app/src/lib/suggestionEvents.ts
import type { SuggestionReasonKind } from "@catalyst-api/market/SuggestionReasonKind";

export type HiddenReason = "flag_off" | "no_signal" | "not_personalized" | "too_few" | "error";
export type ClickTarget = "card" | "reason";
export type PagedAction = "next" | "prev" | "dot";
export type SuggestionSurface = "home" | "pdp" | "cart" | "favorites";

/** How many rows each reason accounts for, in the rail as shown. */
export function reasonCounts(items: Array<{ reason: { kind: SuggestionReasonKind } }>): Record<string, number> {
  const counts: Partial<Record<SuggestionReasonKind, number>> = {};
  for (const item of items) counts[item.reason.kind] = (counts[item.reason.kind] ?? 0) + 1;
  return counts;
}

/** An empty personalised rail is worse than none. */
export const MIN_SUGGESTED_ROWS = 4;

/** Why the personal rail is absent (the first reason in decision order), or null when it shows. */
export function suggestedHiddenReason(input: {
  enabled: boolean;
  hasSignal: boolean;
  isLoading: boolean;
  isError: boolean;
  personalized?: boolean;
  rowCount: number;
}): HiddenReason | null {
  if (!input.enabled) return "flag_off";
  if (!input.hasSignal) return "no_signal";
  if (input.isLoading) return null;
  if (input.isError) return "error";
  if (input.personalized !== true) return "not_personalized";
  if (input.rowCount < MIN_SUGGESTED_ROWS) return "too_few";
  return null;
}
