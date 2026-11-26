import type { ShopClient } from "../api/shop";
import { getUsdBalance, type UsdBalance } from "./credits";

export const CREDITS_BALANCE_QUERY_KEY = "shop-usd-balance";

const POLL_MS = 15_000;

export function nextBalancePoll(balance: UsdBalance | undefined, now: number = Date.now()): number | false {
  const held = balance?.held;
  if (!held) return false;
  const eta = held.releasesAtSeconds;
  if (eta == null) return POLL_MS;
  const untilDue = eta * 1000 - now + 1_000;
  return untilDue > 0 ? Math.min(untilDue, POLL_MS) : POLL_MS;
}

type BalanceState = { state: { data: UsdBalance | undefined } };

/** Both Sites and Shop observe this account key so purchases invalidate the same balance. */
export function creditsBalanceQuery(address: string | null | undefined, client: ShopClient) {
  return {
    queryKey: [CREDITS_BALANCE_QUERY_KEY, address?.toLowerCase() ?? null],
    enabled: !!address,
    staleTime: 30_000,
    refetchOnWindowFocus: (query: BalanceState) => !!query.state.data?.held,
    refetchInterval: (query: BalanceState) => nextBalancePoll(query.state.data),
    queryFn: ({ signal }: { signal: AbortSignal }) => getUsdBalance(address!, client, signal),
  };
}

/** Whole credits with digit grouping. */
export function formatCreditsBalance(credits: number): string {
  return Math.floor(credits).toLocaleString("en-US");
}
