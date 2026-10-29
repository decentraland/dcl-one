// from: shop@94584a5c:app/src/hooks/useBalance.ts, app/src/hooks/useCreditPacks.ts, app/src/hooks/useManaBalance.ts,
//       app/src/hooks/useManaRate.ts
// The buyer's balances as react-query hooks over the services (checkout/services.tsx). The credit
// balance polls ONLY while credits are held, aimed at the moment they can come back (the server's own
// sweep, 15 s cadence otherwise) and refetched on focus while held; an ordinary session never polls.

import { useQuery } from "@tanstack/react-query";

import { SHOP_QUERY_KEYS } from "../checkout/lib/after-purchase";
import { readManaBalanceWei } from "../checkout/lib/mana";
import { manaRateQueryOptions } from "../checkout/lib/mana-rate";
import { settlementChainId } from "../checkout/chain/config";
import { useShopServices } from "../checkout/services";
import { getUsdBalance, type UsdBalance } from "./credits";
import { CREDIT_PACKS, fetchCreditPacks, type CreditPack } from "./payments";

const POLL_MS = 15_000;

export function nextBalancePoll(balance: UsdBalance | undefined, now: number = Date.now()): number | false {
  const held = balance?.held;
  if (!held) return false;
  const eta = held.releasesAtSeconds;
  if (eta == null) return POLL_MS;
  const untilDue = eta * 1000 - now + 1_000;
  return untilDue > 0 ? Math.min(untilDue, POLL_MS) : POLL_MS;
}

/** The signed-in account's spendable credits (1 credit = $0.10). */
export function useBalance(address: string | null | undefined) {
  const { client } = useShopServices();
  return useQuery<UsdBalance>({
    queryKey: [SHOP_QUERY_KEYS.balance, address?.toLowerCase() ?? null],
    enabled: !!address,
    staleTime: 30_000,
    refetchOnWindowFocus: (query) => !!query.state.data?.held,
    refetchInterval: (query) => nextBalancePoll(query.state.data),
    queryFn: ({ signal }) => getUsdBalance(address!, client, signal),
  });
}

/** A dash when the balance failed to load: a misleading 0 reads as "you have no credits". */
export function balanceLabel(balance: UsdBalance | undefined, isError: boolean): string | number {
  return isError ? "—" : (balance?.credits ?? 0);
}

/** The pack catalogue; always populated (the bundled fallback) so the no-funds pickers never break. */
export function useCreditPacks(): { packs: CreditPack[]; isLoading: boolean; isError: boolean } {
  const { client } = useShopServices();
  const { data, isLoading, isError } = useQuery({
    queryKey: ["shop-credit-packs"],
    queryFn: ({ signal }) => fetchCreditPacks(client, signal),
    staleTime: 5 * 60_000,
    retry: false,
  });
  const packs = data && data.length > 0 ? data : CREDIT_PACKS;
  return { packs, isLoading: isLoading && data === undefined, isError };
}

/** MANA the account holds on one chain (the settlement chain by default), over our RPC. */
export function useManaBalance(address: string | null | undefined, chainId: number = settlementChainId()) {
  const { reader } = useShopServices();
  return useQuery<bigint>({
    queryKey: [SHOP_QUERY_KEYS.manaBalance, address?.toLowerCase() ?? null, chainId],
    enabled: !!address,
    staleTime: 30_000,
    queryFn: () => readManaBalanceWei(address!, chainId, reader),
  });
}

/** The reference MANA/USD rate. */
export function useManaRate(enabled = true) {
  const { reader } = useShopServices();
  return useQuery({ ...manaRateQueryOptions({ reader }), enabled });
}
