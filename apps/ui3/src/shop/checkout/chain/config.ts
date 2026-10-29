// from: shop@94584a5c:app/src/config/index.ts (chainId, rpcUrl, ethereumChainId, ethereumRpcUrl, relayerUrl),
//       app/src/config/env/{prod,dev}.json, app/src/lib/gasless-config.ts
import { catalystBase } from "../../../data/catalyst/client";
import { CHAIN, type ChainId } from "../../model/chain";

export type ShopChainOverrides = {
  settlementChainId?: number;
  ethereumChainId?: number;
  rpc?: Partial<Record<number, string>>;
  relayerUrl?: string;
  gasless?: boolean;
};

type ShopChainWindow = Window & typeof globalThis & { __SHOP_CHAIN__?: ShopChainOverrides };

const DEFAULT_RPC: Record<number, string> = {
  [CHAIN.POLYGON]: "https://rpc.decentraland.org/polygon",
  [CHAIN.ETHEREUM]: "https://rpc.decentraland.org/mainnet",
  [CHAIN.AMOY]: "https://rpc.decentraland.org/amoy",
  [CHAIN.SEPOLIA]: "https://rpc.decentraland.org/sepolia",
};

function env(name: string): string | undefined {
  const value = typeof import.meta !== "undefined" ? (import.meta.env?.[name] as string | undefined) : undefined;
  return value && value.trim() ? value.trim() : undefined;
}

function fromWindow(): ShopChainOverrides {
  return typeof window !== "undefined" ? ((window as ShopChainWindow).__SHOP_CHAIN__ ?? {}) : {};
}

function overrides(): ShopChainOverrides {
  return fromWindow();
}

function numberFrom(value: string | undefined): number | undefined {
  const n = value === undefined ? NaN : Number(value);
  return Number.isSafeInteger(n) && n > 0 ? n : undefined;
}

export function settlementChainId(): ChainId {
  return overrides().settlementChainId ?? numberFrom(env("VITE_SHOP_CHAIN_ID")) ?? CHAIN.POLYGON;
}

export function ethereumChainId(): ChainId {
  const settle = settlementChainId();
  const fallback = settle === CHAIN.AMOY ? CHAIN.SEPOLIA : CHAIN.ETHEREUM;
  return overrides().ethereumChainId ?? numberFrom(env("VITE_SHOP_ETHEREUM_CHAIN_ID")) ?? fallback;
}

export function rpcUrl(chainId: ChainId): string | null {
  const o = overrides().rpc?.[chainId];
  if (o) return o;
  const named =
    chainId === CHAIN.POLYGON
      ? env("VITE_SHOP_RPC_POLYGON")
      : chainId === CHAIN.ETHEREUM
        ? env("VITE_SHOP_RPC_ETHEREUM")
        : chainId === CHAIN.AMOY
          ? env("VITE_SHOP_RPC_AMOY")
          : chainId === CHAIN.SEPOLIA
            ? env("VITE_SHOP_RPC_SEPOLIA")
            : undefined;
  return named ?? DEFAULT_RPC[chainId] ?? null;
}

export function relayerUrl(): string {
  return (overrides().relayerUrl ?? env("VITE_SHOP_RELAYER_URL") ?? `${catalystBase()}/transactions-api/v1`).replace(/\/$/, "");
}

export function gaslessEnabled(): boolean {
  const o = overrides().gasless;
  if (typeof o === "boolean") return o;
  const flag = (env("VITE_SHOP_GASLESS") ?? "").toLowerCase();
  return flag !== "0" && flag !== "false";
}

export function explorerAddressUrl(chainId: ChainId, address: string): string | null {
  const base =
    chainId === CHAIN.POLYGON
      ? "https://polygonscan.com/address/"
      : chainId === CHAIN.AMOY
        ? "https://amoy.polygonscan.com/address/"
        : chainId === CHAIN.ETHEREUM
          ? "https://etherscan.io/address/"
          : chainId === CHAIN.SEPOLIA
            ? "https://sepolia.etherscan.io/address/"
            : null;
  return base && /^0x[0-9a-fA-F]{40}$/.test(address) ? `${base}${address.toLowerCase()}` : null;
}

export function explorerTxUrl(chainId: ChainId, txHash: string): string | null {
  const base =
    chainId === CHAIN.POLYGON
      ? "https://polygonscan.com/tx/"
      : chainId === CHAIN.AMOY
        ? "https://amoy.polygonscan.com/tx/"
        : chainId === CHAIN.ETHEREUM
          ? "https://etherscan.io/tx/"
          : chainId === CHAIN.SEPOLIA
            ? "https://sepolia.etherscan.io/tx/"
            : null;
  return base && /^0x[0-9a-fA-F]{64}$/.test(txHash) ? `${base}${txHash}` : null;
}
