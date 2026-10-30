// from: shop@2d3bf62:app/src/lib/dead-listings.ts

import type { QueryClient } from "@tanstack/react-query";

const KEY = ["cancelled-listings"];

export function markListingCancelled(qc: QueryClient, tradeId: string): void {
  if (!tradeId) return;
  qc.setQueryData<string[]>(KEY, (prev) => (prev?.includes(tradeId) ? prev : [...(prev ?? []), tradeId]));
}

export function liveTradeId(qc: QueryClient, tradeId?: string | null): string | undefined {
  if (!tradeId) return undefined;
  return qc.getQueryData<string[]>(KEY)?.includes(tradeId) ? undefined : tradeId;
}
