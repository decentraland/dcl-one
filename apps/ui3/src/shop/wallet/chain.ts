// from: shop@94584a5c:app/src/lib/network.ts, app/src/hooks/useWalletChain.ts
// The wallet's network: reading it, and changing it only when the buyer asked for that. Built on
// ui3's EIP-1193 seam (data/auth/wallet) instead of an ethers Web3Provider. `requestChainSwitch` must
// run inside the buyer's own click: wallets refuse `wallet_*` requests they cannot attribute to a
// gesture (-32006 / 4100), and a declined switch (4001) is an answer, not an error to retry around.

import type { Eip1193Provider } from "../../data/auth/wallet";
import { CHAIN, chainLabel } from "../model/chain";

export const USER_REJECTED = 4001;
const UNKNOWN_CHAIN = 4902;

/** Amoy is not in most wallets by default, so switching to it can need an add first (EIP-3085). */
export const AMOY_ADD_PARAMS = {
  chainId: "0x13882",
  chainName: "Polygon Amoy",
  nativeCurrency: { name: "POL", symbol: "POL", decimals: 18 },
  rpcUrls: ["https://rpc-amoy.polygon.technology"],
  blockExplorerUrls: ["https://amoy.polygonscan.com"],
};

export class WrongNetworkError extends Error {
  override name = "WrongNetworkError";
  constructor(
    readonly current: number,
    readonly required: number,
  ) {
    super(`Wallet is on ${chainLabel(current)}, but this runs on ${chainLabel(required)}`);
  }
}

export function isWrongNetworkError(e: unknown): e is WrongNetworkError {
  return e instanceof WrongNetworkError || (e as { name?: string } | null)?.name === "WrongNetworkError";
}

export function isUserRejection(e: unknown): boolean {
  return (e as { code?: unknown } | null)?.code === USER_REJECTED;
}

/** The chain the wallet is on right now (`eth_chainId`, never a cached network). Throws on a bad answer. */
export async function readChainId(provider: Eip1193Provider): Promise<number> {
  const raw = (await provider.request({ method: "eth_chainId" })) as string | number;
  const parsed = typeof raw === "string" ? Number.parseInt(raw, 16) : Number(raw);
  if (!Number.isSafeInteger(parsed) || parsed <= 0) throw new Error(`Wallet answered eth_chainId with an unusable value: ${String(raw)}`);
  return parsed;
}

/** Refuses to continue unless the wallet is already on `chainId`. Never prompts, never switches. */
export async function requireChain(provider: Eip1193Provider, chainId: number): Promise<void> {
  const current = await readChainId(provider);
  if (current !== chainId) throw new WrongNetworkError(current, chainId);
}

/** Asks the wallet to change networks. Call ONLY from the buyer's click. */
export async function requestChainSwitch(provider: Eip1193Provider, chainId: number): Promise<void> {
  try {
    await provider.request({ method: "wallet_switchEthereumChain", params: [{ chainId: `0x${chainId.toString(16)}` }] });
  } catch (e) {
    if ((e as { code?: number }).code === UNKNOWN_CHAIN && chainId === CHAIN.AMOY) {
      await provider.request({ method: "wallet_addEthereumChain", params: [AMOY_ADD_PARAMS] });
      return;
    }
    throw e;
  }
}

export type WalletEvents = {
  onAccountsChanged?(account: string | null): void;
  onChainChanged?(): void;
};

/** Follows the wallet's own events; returns the unsubscribe. */
export function subscribeWallet(provider: Eip1193Provider | null, events: WalletEvents): () => void {
  if (!provider?.on || !provider.removeListener) return () => {};
  const accounts = (...args: unknown[]) => {
    const list = (args[0] as string[] | undefined) ?? [];
    events.onAccountsChanged?.(list[0]?.toLowerCase() ?? null);
  };
  const chain = () => events.onChainChanged?.();
  provider.on("accountsChanged", accounts);
  provider.on("chainChanged", chain);
  return () => {
    provider.removeListener?.("accountsChanged", accounts);
    provider.removeListener?.("chainChanged", chain);
  };
}
