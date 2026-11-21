// from: shop@94584a5c:app/src/lib/authorizations.ts (readProvider), app/src/lib/buy-gasless.ts (readProvider,
//       waitForSettlement), app/src/lib/tx-confirm.ts
// Read-only chain access over each chain's own RPC. Upstream built an ethers JsonRpcProvider per call;
// here it is a small JSON-RPC client behind an interface, so every rail can be driven offline in tests
// and stories by installing a fake reader. The wallet is never asked to read: a wallet on another
// network would answer about the wrong chain.

import type { Hex } from "viem";

import type { ChainId } from "@shop/model/chain";
import { rpcUrl } from "./config";

export type TxReceipt = {
  transactionHash: Hex;
  /** "reverted" is a mined status-0 receipt: final, and nothing the call did happened. */
  status: "success" | "reverted";
  blockNumber: bigint;
};

/** Each read takes an optional `signal`: a reader that honours it lets the read go once it is aborted (a deadline passed). */
export type ChainReader = {
  call(chainId: ChainId, to: string, data: Hex, signal?: AbortSignal): Promise<Hex>;
  receipt(chainId: ChainId, txHash: string, signal?: AbortSignal): Promise<TxReceipt | null>;
  balance(chainId: ChainId, address: string, signal?: AbortSignal): Promise<bigint>;
  code(chainId: ChainId, address: string, signal?: AbortSignal): Promise<Hex>;
};

export class RpcError extends Error {
  override name = "RpcError";
  constructor(
    message: string,
    readonly code: number | null = null,
    readonly data: unknown = null,
  ) {
    super(message);
  }
}

type RpcResponse = { result?: unknown; error?: { code?: number; message?: string; data?: unknown } };

let nextId = 1;

export function createRpcReader(opts: { url?: (chainId: ChainId) => string | null; fetchImpl?: typeof fetch } = {}): ChainReader {
  const urlFor = opts.url ?? rpcUrl;
  const fetchImpl = opts.fetchImpl ?? ((...a: Parameters<typeof fetch>) => fetch(...a));

  async function rpc<T>(chainId: ChainId, method: string, params: unknown[], signal?: AbortSignal): Promise<T> {
    const url = urlFor(chainId);
    if (!url) throw new RpcError(`No RPC is configured for chain ${chainId}`);
    const res = await fetchImpl(url, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ jsonrpc: "2.0", id: nextId++, method, params }),
      ...(signal ? { signal } : {}),
    });
    if (!res.ok) {
      // An error's body is let go, never left open: a reader that polls would leak one stream per failed read.
      void res.body?.cancel().catch(() => undefined);
      throw new RpcError(`RPC ${method} answered HTTP ${res.status}`);
    }
    const body = (await res.json()) as RpcResponse;
    if (body.error) throw new RpcError(body.error.message ?? `RPC ${method} failed`, body.error.code ?? null, body.error.data ?? null);
    return body.result as T;
  }

  return {
    call: (chainId, to, data, signal) => rpc<Hex>(chainId, "eth_call", [{ to, data }, "latest"], signal),
    async receipt(chainId, txHash, signal) {
      const raw = await rpc<{ transactionHash: Hex; status: Hex; blockNumber: Hex } | null>(chainId, "eth_getTransactionReceipt", [txHash], signal);
      if (!raw) return null;
      return {
        transactionHash: raw.transactionHash,
        status: BigInt(raw.status) === 1n ? "success" : "reverted",
        blockNumber: BigInt(raw.blockNumber),
      };
    },
    balance: async (chainId, address, signal) => BigInt(await rpc<Hex>(chainId, "eth_getBalance", [address, "latest"], signal)),
    code: (chainId, address, signal) => rpc<Hex>(chainId, "eth_getCode", [address, "latest"], signal),
  };
}

let shared: ChainReader | null = null;

/** The document's reader: each chain's configured read-only RPC. */
export function shopChainReader(): ChainReader {
  shared ??= createRpcReader();
  return shared;
}

/** Tests, stories and hosts with their own RPC install a reader here. */
export function setShopChainReader(reader: ChainReader | null): void {
  shared = reader;
}

export type WaitOptions = { timeoutMs?: number; pollMs?: number; signal?: AbortSignal; now?: () => number; sleep?: (ms: number) => Promise<void> };

export const defaultSleep = (ms: number, signal?: AbortSignal) =>
  new Promise<void>((resolve, reject) => {
    const timer = setTimeout(resolve, ms);
    signal?.addEventListener(
      "abort",
      () => {
        clearTimeout(timer);
        reject(new DOMException("Aborted", "AbortError"));
      },
      { once: true },
    );
  });

/**
 * Polls for a receipt until one exists or the window closes. Null means none inside the window: the
 * transaction may still mine, which is never the same answer as a revert. Read errors are not answers
 * either; polling continues through them. Without a set `pollMs`, it polls every 500 ms for the first
 * 15 s, where a relayed transaction usually lands (Polygon makes a block about every 2 s), then every 2 s.
 */
export async function waitForReceipt(
  reader: ChainReader,
  chainId: ChainId,
  txHash: string,
  opts: WaitOptions = {},
): Promise<TxReceipt | null> {
  const timeoutMs = opts.timeoutMs ?? 120_000;
  const pollMs = (elapsedMs: number) => Math.max(opts.pollMs ?? (elapsedMs < 15_000 ? 500 : 2_000), 10);
  const now = opts.now ?? Date.now;
  const sleep = opts.sleep ?? ((ms: number) => defaultSleep(ms, opts.signal));
  const started = now();
  for (;;) {
    if (opts.signal?.aborted) throw new DOMException("Aborted", "AbortError");
    try {
      const receipt = await reader.receipt(chainId, txHash, opts.signal);
      if (receipt) return receipt;
    } catch (e) {
      if ((e as { name?: string } | null)?.name === "AbortError") throw e;
    }
    const elapsedMs = now() - started;
    if (elapsedMs >= timeoutMs) return null;
    await sleep(pollMs(elapsedMs));
  }
}
