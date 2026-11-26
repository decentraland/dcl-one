// from: shop@94584a5c:app/src/config/index.ts (chainId, rpcUrl, ethereumChainId, ethereumRpcUrl, relayerUrl),
//       app/src/config/env/{prod,dev}.json, app/src/lib/gasless-config.ts
import { catalystBase } from "../../../data/catalyst/client";
import { selectEthereumChain, selectSettlementChain } from "@shop/checkout/chain/selection";
import { CHAIN, type ChainId } from "@shop/model/chain";
import { parseShopChainId, shopChainEnvironment } from "@web-core/shop-chain-env";
import { pageOverride } from "../../pageOverride";

export type ShopChainOverrides = {
  settlementChainId?: number;
  ethereumChainId?: number;
  rpc?: Partial<Record<number, string>>;
  relayerUrl?: string;
  gasless?: boolean;
};

const DEFAULT_RPC: Record<number, string> = {
  [CHAIN.POLYGON]: "https://rpc.decentraland.org/polygon",
  [CHAIN.ETHEREUM]: "https://rpc.decentraland.org/mainnet",
  [CHAIN.AMOY]: "https://rpc.decentraland.org/amoy",
  [CHAIN.SEPOLIA]: "https://rpc.decentraland.org/sepolia",
};

type ShopEnvKey =
  | "VITE_SHOP_RPC_POLYGON"
  | "VITE_SHOP_RPC_ETHEREUM"
  | "VITE_SHOP_RPC_AMOY"
  | "VITE_SHOP_RPC_SEPOLIA"
  | "VITE_SHOP_CREDITS_MANAGER"
  | "VITE_SHOP_CHAIN_ID"
  | "VITE_SHOP_RELAYER_URL"
  | "VITE_SHOP_GASLESS";

// One static read per key: Vite inlines `import.meta.env.X` as that value alone, but a computed
// `import.meta.env[key]` as the whole env object, every build-time `VITE_*` value, into the bundle.
function env(name: ShopEnvKey): string | undefined {
  if (typeof import.meta === "undefined") return undefined;
  const values: Record<ShopEnvKey, string | undefined> = {
    VITE_SHOP_RPC_POLYGON: import.meta.env?.VITE_SHOP_RPC_POLYGON,
    VITE_SHOP_RPC_ETHEREUM: import.meta.env?.VITE_SHOP_RPC_ETHEREUM,
    VITE_SHOP_RPC_AMOY: import.meta.env?.VITE_SHOP_RPC_AMOY,
    VITE_SHOP_RPC_SEPOLIA: import.meta.env?.VITE_SHOP_RPC_SEPOLIA,
    VITE_SHOP_CREDITS_MANAGER: import.meta.env?.VITE_SHOP_CREDITS_MANAGER,
    VITE_SHOP_CHAIN_ID: import.meta.env?.VITE_SHOP_CHAIN_ID,
    VITE_SHOP_RELAYER_URL: import.meta.env?.VITE_SHOP_RELAYER_URL,
    VITE_SHOP_GASLESS: import.meta.env?.VITE_SHOP_GASLESS,
  };
  const value = values[name];
  return value && value.trim() ? value.trim() : undefined;
}

/** A test's or a local build's override from the page; none in a production build (../../pageOverride). */
function fromWindow(): ShopChainOverrides {
  return pageOverride<ShopChainOverrides>("__SHOP_CHAIN__") ?? {};
}

function overrides(): ShopChainOverrides {
  return fromWindow();
}

export function settlementChainId(): ChainId {
  return selectSettlementChain(shopChainEnvironment());
}

export function ethereumChainId(): ChainId {
  return selectEthereumChain(shopChainEnvironment());
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

/**
 * The CreditsManager this build settles credits through on `chainId`, when it isn't that chain's registry
 * one: `VITE_SHOP_CREDITS_MANAGER`, the address the Shop server's CREDITS_MANAGER_CONTRACT names. It applies
 * only to the chain the build names in `VITE_SHOP_CHAIN_ID`, set explicitly: never to a chain picked at
 * runtime, nor to Polygon by default. Lowercase; null otherwise, or when it isn't an address.
 */
export function configuredCreditsManager(chainId: ChainId): string | null {
  const value = env("VITE_SHOP_CREDITS_MANAGER");
  if (!value || !/^0x[0-9a-fA-F]{40}$/.test(value)) return null;
  return parseShopChainId(env("VITE_SHOP_CHAIN_ID")) === chainId ? value.toLowerCase() : null;
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
