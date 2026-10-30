// from: shop@94584a5c:app/src/lib/mana.ts
// MANA reads for the MANA option: the buyer's balance on one chain (each over that chain's own RPC:
// MANA lives at different addresses on Polygon and Ethereum), both balances for display, and what a
// trade costs in MANA right now. None of this decides eligibility; it only fills the option's state.

import { decodeFunctionResult, encodeFunctionData, type Hex } from "viem";

import { CHAIN } from "../../model/chain";
import { AGGREGATOR_ABI, ERC20_ABI, MARKETPLACE_ABI } from "../chain/abi";
import { ethereumChainId, settlementChainId } from "../chain/config";
import { requireContracts } from "../chain/contracts";
import { shopChainReader, type ChainReader } from "../chain/rpc";
import { ASSET_TYPE, type SignedTrade } from "../purchase/encoding";

export { formatMana, manaWeiToNumber } from "./mana-format";

const PPM = 1_000_000n;

/** An address's MANA in wei on one chain (the settlement chain by default). */
export async function readManaBalanceWei(address: string, chainId: number = settlementChainId(), reader: ChainReader = shopChainReader()): Promise<bigint> {
  const mana = requireContracts(chainId).mana.address;
  const raw = await reader.call(chainId, mana, encodeFunctionData({ abi: ERC20_ABI, functionName: "balanceOf", args: [address as Hex] }));
  return decodeFunctionResult({ abi: ERC20_ABI, functionName: "balanceOf", data: raw });
}

/**
 * MANA on both chains, for display. A failed chain reads as 0 for that chain only, and is reported, so
 * one dead RPC does not blank the other balance.
 */
export async function readManaBalancesWei(
  address: string,
  opts: { reader?: ChainReader; onError?: (e: unknown, chainId: number) => void } = {},
): Promise<{ ethereum: bigint; matic: bigint }> {
  const reader = opts.reader ?? shopChainReader();
  const readOrZero = (chainId: number) =>
    readManaBalanceWei(address, chainId, reader).catch((e: unknown) => {
      (opts.onError ?? ((err) => console.warn("[shop/mana] balance read failed", chainId, err)))(e, chainId);
      return 0n;
    });
  const [ethereum, matic] = await Promise.all([readOrZero(ethereumChainId()), readOrZero(settlementChainId())]);
  return { ethereum, matic };
}

/** A listing's MANA with the creator's rate discount applied, rounded UP (it also sizes the allowance). */
export function discountedManaWei(listWei: bigint, coupon?: { discount: number } | null): bigint {
  if (!coupon) return listWei;
  const rate = PPM - BigInt(coupon.discount);
  return (listWei * rate + PPM - 1n) / PPM;
}

/**
 * What a trade costs in MANA right now: the signed amount for a MANA trade, or the USD-pegged amount
 * at that trade's own marketplace oracle, flooring (the chain recomputes it at settlement).
 */
export async function readTradeManaPriceWei(trade: SignedTrade, reader: ChainReader = shopChainReader()): Promise<bigint> {
  const price = trade.received[0];
  const amount = BigInt(price?.amount ?? price?.value ?? "0");
  if (price?.assetType !== ASSET_TYPE.USD_PEGGED_MANA) return amount;
  const chainId = trade.chainId || CHAIN.POLYGON;
  const aggregator = decodeFunctionResult({
    abi: MARKETPLACE_ABI,
    functionName: "manaUsdAggregator",
    data: await reader.call(chainId, trade.contract, encodeFunctionData({ abi: MARKETPLACE_ABI, functionName: "manaUsdAggregator" })),
  });
  const decimals = decodeFunctionResult({
    abi: AGGREGATOR_ABI,
    functionName: "decimals",
    data: await reader.call(chainId, aggregator, encodeFunctionData({ abi: AGGREGATOR_ABI, functionName: "decimals" })),
  });
  const [, answer] = decodeFunctionResult({
    abi: AGGREGATOR_ABI,
    functionName: "latestRoundData",
    data: await reader.call(chainId, aggregator, encodeFunctionData({ abi: AGGREGATOR_ABI, functionName: "latestRoundData" })),
  });
  if (answer <= 0n) throw new Error("mana rate unavailable");
  return (amount * 10n ** BigInt(decimals)) / answer;
}

export function hasEnoughMana(balanceWei: bigint, priceWei: bigint): boolean {
  return balanceWei >= priceWei;
}
