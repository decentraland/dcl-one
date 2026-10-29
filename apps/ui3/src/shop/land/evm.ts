// from: marketplace@f3956826:webapp/src/modules/rental/contract.ts, webapp/src/modules/rental/utils.ts (the
// nonce reads), catalyrst-v2 sites/src/data/lib/catalyst/marketplace/name-claim.ts (transaction + receipt)
// Copyright 2020 Metaverse Holdings Ltd. Licensed under the Apache License, Version 2.0.
// Contract reads and writes for the LAND and NAME flows on ui3's EIP-1193 seam, with viem for the ABI
// (no ethers Web3Provider, no decentraland-dapps sendTransaction). Reads that need no wallet go to the
// node's read-only JSON-RPC relay (`/rpc/{network}`); writes always go through the buyer's wallet,
// only on the chain the caller names, and never switch it (the buyer does, from a click).

import { decodeFunctionResult, encodeFunctionData, erc20Abi, parseAbi, type Abi, type Hex } from "viem";

import type { Eip1193Provider } from "../../data/auth/wallet";
import { catalystBase } from "../../data/catalyst/client";
import { requireChain } from "../wallet/chain";

export const erc721Abi = parseAbi([
  "function ownerOf(uint256 tokenId) view returns (address)",
  "function isApprovedForAll(address owner, address operator) view returns (bool)",
  "function setApprovalForAll(address operator, bool approved)",
  "function transferFrom(address from, address to, uint256 tokenId)",
]);

export { erc20Abi };

/** Something that answers `eth_call`: the node's relay, or a connected wallet. */
export type ChainReader = { chainId: number; call(to: string, data: Hex, signal?: AbortSignal): Promise<Hex> };

type RpcResponse = { result?: unknown; error?: { code?: number; message?: string } };

let rpcId = 0;

/** The relay's network names (src/ops/rpc: `POST /rpc/{network}`). */
const RPC_NETWORK: Record<number, string> = { 1: "mainnet", 11155111: "sepolia", 137: "polygon", 80002: "amoy" };

/** Reads through the node's read-only JSON-RPC relay for `chainId` (rpc.decentraland.org's contract). */
export function rpcReader(chainId: number, opts: { base?: string; fetchImpl?: typeof fetch } = {}): ChainReader {
  const network = RPC_NETWORK[chainId];
  if (!network) throw new RangeError(`No read relay for chain ${chainId}`);
  const url = `${catalystBase(opts.base)}/rpc/${network}`;
  const doFetch = opts.fetchImpl ?? ((...a: Parameters<typeof fetch>) => fetch(...a));
  return {
    chainId,
    async call(to, data, signal) {
      const res = await doFetch(url, {
        method: "POST",
        headers: { "content-type": "application/json", accept: "application/json" },
        body: JSON.stringify({ jsonrpc: "2.0", id: ++rpcId, method: "eth_call", params: [{ to, data }, "latest"] }),
        signal,
      });
      if (!res.ok) throw new Error(`The chain reader answered ${res.status}`);
      const body = (await res.json()) as RpcResponse;
      if (body.error) throw new Error(body.error.message ?? "The chain read failed");
      if (typeof body.result !== "string" || !body.result.startsWith("0x")) throw new Error("The chain read returned no data");
      return body.result as Hex;
    },
  };
}

/** Reads through the buyer's own wallet, which must already be on `chainId`. */
export function walletReader(provider: Eip1193Provider, chainId: number): ChainReader {
  return {
    chainId,
    async call(to, data) {
      await requireChain(provider, chainId);
      const result = await provider.request({ method: "eth_call", params: [{ to, data }, "latest"] });
      if (typeof result !== "string") throw new Error("The wallet returned no data for a contract read");
      return result as Hex;
    },
  };
}

/** One `view` call, ABI-encoded and decoded by viem; `T` is what the function returns. */
export async function readContract<T>(
  reader: ChainReader,
  address: string,
  abi: Abi,
  functionName: string,
  args: readonly unknown[],
  signal?: AbortSignal,
): Promise<T> {
  const data = encodeFunctionData({ abi, functionName, args } as never);
  const raw = await reader.call(address, data, signal);
  return decodeFunctionResult({ abi, functionName, data: raw } as never) as T;
}

export class TxRevertedError extends Error {
  override name = "TxRevertedError";
  constructor(readonly txHash: string) {
    super("The transaction was reverted by the network.");
  }
}

/** The transaction was handed to the network but has not confirmed within the wait: it may still land. */
export class TxPendingError extends Error {
  override name = "TxPendingError";
  constructor(readonly txHash: string) {
    super("The transaction is still pending.");
  }
}

export type TxRequest = { chainId: number; from: string; to: string; data: Hex; value?: bigint };

/**
 * Sends one transaction from the buyer's wallet. Refuses (WrongNetworkError) instead of switching when
 * the wallet is on another chain, and checks the connected account is still the one the flow started
 * with, so an account switch mid-flow cannot send from someone else.
 */
export async function sendTransaction(provider: Eip1193Provider, tx: TxRequest): Promise<string> {
  await requireChain(provider, tx.chainId);
  const accounts = await provider.request({ method: "eth_accounts" });
  const current = Array.isArray(accounts) ? String(accounts[0] ?? "").toLowerCase() : "";
  if (current !== tx.from.toLowerCase()) throw new AccountChangedError(tx.from, current || null);
  const params: Record<string, string> = { from: tx.from, to: tx.to, data: tx.data };
  if (tx.value && tx.value > 0n) params.value = `0x${tx.value.toString(16)}`;
  const hash = await provider.request({ method: "eth_sendTransaction", params: [params] });
  if (typeof hash !== "string" || !/^0x[0-9a-fA-F]{64}$/.test(hash)) throw new Error("The wallet returned no transaction hash.");
  return hash;
}

export class AccountChangedError extends Error {
  override name = "AccountChangedError";
  constructor(
    readonly expected: string,
    readonly current: string | null,
  ) {
    super("Your wallet is on a different account than the one this purchase started with.");
  }
}

export function encodeCall(abi: Abi, functionName: string, args: readonly unknown[]): Hex {
  return encodeFunctionData({ abi, functionName, args } as never);
}

type Receipt = { status?: string | number } | null;

/**
 * Waits for `hash` to mine. Resolves on success, throws TxRevertedError on a revert and TxPendingError
 * when the wait runs out: the transaction may still land, so the caller must not treat it as failed.
 */
export async function waitForReceipt(
  provider: Eip1193Provider,
  hash: string,
  opts: { pollMs?: number; timeoutMs?: number; signal?: AbortSignal; now?: () => number } = {},
): Promise<void> {
  const now = opts.now ?? Date.now;
  const deadline = now() + (opts.timeoutMs ?? 300_000);
  for (;;) {
    opts.signal?.throwIfAborted();
    let receipt: Receipt = null;
    try {
      receipt = (await provider.request({ method: "eth_getTransactionReceipt", params: [hash] })) as Receipt;
    } catch {
      receipt = null;
    }
    if (receipt) {
      const status = typeof receipt.status === "string" ? Number.parseInt(receipt.status, 16) : receipt.status;
      if (status === 1) return;
      throw new TxRevertedError(hash);
    }
    if (now() >= deadline) throw new TxPendingError(hash);
    await new Promise((resolve) => setTimeout(resolve, opts.pollMs ?? 3000));
  }
}

export type ApprovalStage = "checking" | "awaiting-approval" | "approving";

/**
 * Makes sure `spender` may pull `required` of `token` from `owner`: skips a sufficient allowance, else
 * asks the wallet for one approval of exactly `required` and waits for it.
 */
export async function ensureAllowance(
  provider: Eip1193Provider,
  opts: {
    chainId: number;
    token: string;
    owner: string;
    spender: string;
    required: bigint;
    reader?: ChainReader;
    onStage?(stage: ApprovalStage): void;
    wait?: Parameters<typeof waitForReceipt>[2];
  },
): Promise<string | null> {
  const reader = opts.reader ?? walletReader(provider, opts.chainId);
  opts.onStage?.("checking");
  const allowance = await readContract<bigint>(reader, opts.token, erc20Abi, "allowance", [opts.owner, opts.spender]);
  if (allowance >= opts.required) return null;
  opts.onStage?.("awaiting-approval");
  const hash = await sendTransaction(provider, {
    chainId: opts.chainId,
    from: opts.owner,
    to: opts.token,
    data: encodeCall(erc20Abi, "approve", [opts.spender, opts.required]),
  });
  opts.onStage?.("approving");
  await waitForReceipt(provider, hash, opts.wait);
  return hash;
}

/** Makes sure `operator` may move every token of `registry` the owner holds (ERC-721 setApprovalForAll). */
export async function ensureOperatorApproval(
  provider: Eip1193Provider,
  opts: {
    chainId: number;
    registry: string;
    owner: string;
    operator: string;
    reader?: ChainReader;
    onStage?(stage: ApprovalStage): void;
    wait?: Parameters<typeof waitForReceipt>[2];
  },
): Promise<string | null> {
  const reader = opts.reader ?? walletReader(provider, opts.chainId);
  opts.onStage?.("checking");
  const approved = await readContract<boolean>(reader, opts.registry, erc721Abi, "isApprovedForAll", [opts.owner, opts.operator]);
  if (approved) return null;
  opts.onStage?.("awaiting-approval");
  const hash = await sendTransaction(provider, {
    chainId: opts.chainId,
    from: opts.owner,
    to: opts.registry,
    data: encodeCall(erc721Abi, "setApprovalForAll", [opts.operator, true]),
  });
  opts.onStage?.("approving");
  await waitForReceipt(provider, hash, opts.wait);
  return hash;
}

function explorerHost(chainId: number): string {
  return (
    chainId === 1
      ? "https://etherscan.io"
      : chainId === 11155111
        ? "https://sepolia.etherscan.io"
        : chainId === 137
          ? "https://polygonscan.com"
          : chainId === 80002
            ? "https://amoy.polygonscan.com"
            : "https://etherscan.io"
  );
}

/** A block explorer link for a transaction. */
export function txHref(chainId: number, hash: string): string {
  return `${explorerHost(chainId)}/tx/${hash}`;
}

/** A block explorer link for a contract or account. */
export function addressHref(chainId: number, address: string): string {
  return `${explorerHost(chainId)}/address/${address}`;
}
