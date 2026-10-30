// from: shop@94584a5c:app/src/lib/mana-rate.ts
// The reference MANA/USD rate: the marketplace's own `manaUsdAggregator`, read over our RPC. A
// REFERENCE rate for display and sizing only; what a USD-pegged trade settles at is read from that
// trade's own marketplace (lib/mana.ts). Stale (older than the heartbeat plus an hour), incomplete or
// non-positive rounds throw, so callers show "price unavailable" instead of pricing off a bad rate.

import { decodeFunctionResult, encodeFunctionData, type Hex } from "viem";

import { manaUsdRate } from "../../model/money";
import { AGGREGATOR_ABI, MARKETPLACE_ABI } from "../chain/abi";
import { settlementChainId } from "../chain/config";
import { shopChainReader, type ChainReader } from "../chain/rpc";
import type { ManaRate } from "./currency";
import { getLatestOffChainMarketplaceContract } from "./marketplace";

export * from "./currency";

const MAX_STALENESS_SECONDS = 90_000;

export async function readAggregatorAddress(reader: ChainReader, chainId: number, marketplace: string): Promise<Hex> {
  const raw = await reader.call(chainId, marketplace, encodeFunctionData({ abi: MARKETPLACE_ABI, functionName: "manaUsdAggregator" }));
  return decodeFunctionResult({ abi: MARKETPLACE_ABI, functionName: "manaUsdAggregator", data: raw });
}

export async function readRate(reader: ChainReader, chainId: number, aggregator: string, nowSeconds: number = Math.floor(Date.now() / 1000)): Promise<ManaRate> {
  const decimals = decodeFunctionResult({
    abi: AGGREGATOR_ABI,
    functionName: "decimals",
    data: await reader.call(chainId, aggregator, encodeFunctionData({ abi: AGGREGATOR_ABI, functionName: "decimals" })),
  });
  const [roundId, answer, , updatedAt, answeredInRound] = decodeFunctionResult({
    abi: AGGREGATOR_ABI,
    functionName: "latestRoundData",
    data: await reader.call(chainId, aggregator, encodeFunctionData({ abi: AGGREGATOR_ABI, functionName: "latestRoundData" })),
  });
  if (answer <= 0n) throw new Error("mana rate unavailable");
  if (answeredInRound < roundId) throw new Error("mana rate incomplete");
  const age = nowSeconds - Number(updatedAt);
  if (updatedAt <= 0n || age > MAX_STALENESS_SECONDS) throw new Error("mana rate stale");
  const rate = manaUsdRate(answer, Number(decimals));
  if (!rate) throw new Error("mana rate unavailable");
  return rate;
}

/** The reference rate from a marketplace's aggregator (upstream `readManaUsdRate`). */
export async function readManaUsdRate(marketplaceAddress: string, opts: { reader?: ChainReader; chainId?: number } = {}): Promise<ManaRate> {
  const reader = opts.reader ?? shopChainReader();
  const chainId = opts.chainId ?? settlementChainId();
  const aggregator = await readAggregatorAddress(reader, chainId, marketplaceAddress);
  return readRate(reader, chainId, aggregator);
}

/** One react-query definition for the reference rate, keyed by chain (upstream `manaRateQueryOptions`). */
export function manaRateQueryOptions(opts: { reader?: ChainReader; chainId?: number } = {}) {
  const chainId = opts.chainId ?? settlementChainId();
  return {
    queryKey: ["shop-mana-rate", chainId] as const,
    queryFn: () => readManaUsdRate(getLatestOffChainMarketplaceContract(chainId).address, { ...opts, chainId }),
    staleTime: 60_000,
  };
}
