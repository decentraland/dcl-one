// from: shop@94584a5c:app/src/lib/authorizations.ts (readProvider)
// Reads go to the TARGET chain's RPC, never the wallet's current network: a wallet on Ethereum must still
// be able to see a Polygon approval, and the wallet's RPC is also the flakiest one in the path (upstream's
// -32002 "too many errors"). One JSON-RPC URL per chain, overridable by the host
// (`window.__SHOP_RPC__ = { 137: "https://…" }` or `setShopRpc`), with the public endpoints the contract
// boundary reads used (tests/fixtures/shop/chain-reads.json) as defaults.

import { decodeFunctionResult, encodeFunctionData, type Abi, type ContractFunctionName } from "viem";

import { CHAIN, type ChainId } from "../../model/chain";

const DEFAULT_RPC: Partial<Record<ChainId, string>> = {
  [CHAIN.ETHEREUM]: "https://ethereum-rpc.publicnode.com",
  [CHAIN.SEPOLIA]: "https://ethereum-sepolia-rpc.publicnode.com",
  [CHAIN.POLYGON]: "https://polygon-bor-rpc.publicnode.com",
  [CHAIN.AMOY]: "https://rpc-amoy.polygon.technology",
};

type RpcWindow = Window & { __SHOP_RPC__?: Partial<Record<number, string>> };

const overrides = new Map<ChainId, string>();

export type RpcTransport = (chainId: ChainId, method: string, params: unknown[]) => Promise<unknown>;

let transport: RpcTransport | null = null;

/** Point a chain's reads at another RPC (a host's own node, a test double). */
export function setShopRpc(chainId: ChainId, url: string | null): void {
  if (url) overrides.set(chainId, url);
  else overrides.delete(chainId);
}

/** Replace the JSON-RPC transport entirely (tests, the Explorer's engine bridge). Returns the restore. */
export function setRpcTransport(next: RpcTransport | null): () => void {
  const previous = transport;
  transport = next;
  return () => {
    transport = previous;
  };
}

export function rpcUrl(chainId: ChainId): string {
  const fromWindow = typeof window !== "undefined" ? (window as RpcWindow).__SHOP_RPC__?.[chainId] : undefined;
  const url = overrides.get(chainId) ?? fromWindow ?? DEFAULT_RPC[chainId];
  if (!url) throw new Error(`No RPC is configured for chain ${chainId}`);
  return url;
}

export class RpcError extends Error {
  override name = "RpcError";
  constructor(
    message: string,
    readonly code: number | null,
  ) {
    super(message);
  }
}

let nextId = 1;

export async function rpc<T = unknown>(chainId: ChainId, method: string, params: unknown[] = []): Promise<T> {
  if (transport) return (await transport(chainId, method, params)) as T;
  const res = await fetch(rpcUrl(chainId), {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ jsonrpc: "2.0", id: nextId++, method, params }),
  });
  if (!res.ok) throw new RpcError(`RPC ${method} answered ${res.status}`, res.status);
  const body = (await res.json()) as { result?: T; error?: { message?: string; code?: number } };
  if (body.error) throw new RpcError(body.error.message ?? `RPC ${method} failed`, body.error.code ?? null);
  return body.result as T;
}

/** A typed `eth_call` against `address` on `chainId` at the latest block. */
export async function readContract<const abi extends Abi, name extends ContractFunctionName<abi, "view" | "pure">>(opts: {
  chainId: ChainId;
  address: string;
  abi: abi;
  functionName: name;
  args?: readonly unknown[];
}): Promise<unknown> {
  const data = encodeFunctionData({ abi: opts.abi, functionName: opts.functionName, args: opts.args } as never);
  const raw = await rpc<`0x${string}`>(opts.chainId, "eth_call", [{ to: opts.address, data }, "latest"]);
  return decodeFunctionResult({ abi: opts.abi, functionName: opts.functionName, data: raw } as never);
}
