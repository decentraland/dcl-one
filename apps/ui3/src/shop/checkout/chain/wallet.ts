// from: shop@94584a5c:app/src/lib/buy.ts (sendUseCredits), app/src/lib/trade-encoding.ts (amoyGasOverrides),
//       app/src/lib/network.ts (requireChain before a gas-paying leg)
// The buyer-submitted rail over ui3's EIP-1193 seam: the wallet broadcasts and pays gas, so it must
// already be on the settlement chain (checked, never switched: wallet/chain requireChain), and the
// receipt is then read over our own RPC. `onBroadcast` fires with the hash before the receipt is
// awaited: from that moment the call may be consumed on chain whatever happens next.

import { numberToHex, type Hex } from "viem";

import type { Eip1193Provider } from "../../../data/auth/wallet";
import { CHAIN, type ChainId } from "../../model/chain";
import { requireChain } from "../../wallet/chain";
import { waitForReceipt, type ChainReader, type TxReceipt, type WaitOptions } from "./rpc";

/** Amoy enforces a ~25 gwei minimum priority fee that wallets often undershoot (upstream `amoyGasOverrides`). */
export function amoyGasOverrides(chainId: ChainId): { maxPriorityFeePerGas?: Hex; maxFeePerGas?: Hex } {
  if (chainId !== CHAIN.AMOY) return {};
  const tip = 30n * 10n ** 9n;
  return { maxPriorityFeePerGas: numberToHex(tip), maxFeePerGas: numberToHex(tip + 50n * 10n ** 9n) };
}

/** The wallet's first account, lowercase. */
export async function walletAccount(provider: Eip1193Provider): Promise<string> {
  let accounts: string[] = [];
  try {
    accounts = ((await provider.request({ method: "eth_requestAccounts" })) as string[] | undefined) ?? [];
  } catch {
    accounts = ((await provider.request({ method: "eth_accounts" })) as string[] | undefined) ?? [];
  }
  const first = accounts[0];
  if (!first) throw new Error("Could not find a connected wallet account");
  return first.toLowerCase();
}

/**
 * A mined transaction that reverted. Carries the receipt: a revert rolls the call back, so nothing it
 * spent was consumed (upstream `isRevertedTxError` reads the same fact off the ethers receipt).
 */
export class TxRevertedError extends Error {
  override name = "TxRevertedError";
  constructor(
    readonly receipt: TxReceipt,
    what = "The transaction",
  ) {
    super(`${what} reverted on-chain`);
  }
}

/** Broadcast, but no receipt inside the window: it may still mine. Never a reason to release or retry. */
export class TxPendingError extends Error {
  override name = "TxPendingError";
  constructor(
    readonly txHash: string,
    what = "The transaction",
  ) {
    super(`${what} was submitted but not confirmed in time`);
  }
}

export type SendOptions = {
  provider: Eip1193Provider;
  reader: ChainReader;
  chainId: ChainId;
  from: string;
  to: string;
  data: Hex;
  value?: bigint;
  /** Fired right before the transaction is handed to the wallet: from here it may be broadcast. */
  onHandoff?: () => void;
  /** Fired with the hash the moment the wallet hands it back: the point of no return. */
  onBroadcast?: (txHash: Hex) => void;
  wait?: WaitOptions;
  what?: string;
};

/** Submit from the wallet and wait for the receipt. Resolves only on success. */
export async function sendAndConfirm(opts: SendOptions): Promise<Hex> {
  await requireChain(opts.provider, opts.chainId);
  opts.onHandoff?.();
  const txHash = (await opts.provider.request({
    method: "eth_sendTransaction",
    params: [
      {
        from: opts.from,
        to: opts.to,
        data: opts.data,
        ...(opts.value ? { value: numberToHex(opts.value) } : {}),
        ...amoyGasOverrides(opts.chainId),
      },
    ],
  })) as Hex;
  opts.onBroadcast?.(txHash);
  const receipt = await waitForReceipt(opts.reader, opts.chainId, txHash, opts.wait);
  if (!receipt) throw new TxPendingError(txHash, opts.what);
  if (receipt.status === "reverted") throw new TxRevertedError(receipt, opts.what);
  return txHash;
}

/** Upstream `isRevertedTxError`: the one post-broadcast failure whose credits are provably untouched. */
export function isRevertedTxError(err: unknown): boolean {
  return err instanceof TxRevertedError || (err as { name?: string } | null)?.name === "TxRevertedError";
}

/** Upstream `revertedTxHash`. */
export function revertedTxHash(err: unknown): string | null {
  return isRevertedTxError(err) ? ((err as TxRevertedError).receipt?.transactionHash ?? null) : null;
}

export function isPendingTxError(err: unknown): boolean {
  return err instanceof TxPendingError || (err as { name?: string } | null)?.name === "TxPendingError";
}
