// The browser host's choice of the Ethereum network the Shop's LAND and NAME flows run on. The networks themselves,
// and everything read from them, take the chain as an argument (@shop/land/contracts).

import { CHAIN, type ChainId } from "@shop/model/chain";
import { ethereumChainId } from "../checkout/chain/config";
import { LAND_NETWORKS } from "@shop/land/contracts";

type ShopEnvWindow = Window & typeof globalThis & { __SHOP_ETHEREUM_CHAIN_ID__?: number };

/**
 * The Ethereum network the Shop's LAND and NAME flows run on: the host's choice, else the Shop's own
 * (`ethereumChainId`: VITE_SHOP_ETHEREUM_CHAIN_ID, or Sepolia when the Shop settles on Amoy, mainnet otherwise).
 */
export function landChainId(): ChainId {
  const fromWindow = typeof window !== "undefined" ? (window as ShopEnvWindow).__SHOP_ETHEREUM_CHAIN_ID__ : undefined;
  const chosen = fromWindow ?? ethereumChainId();
  return LAND_NETWORKS[chosen] ? chosen : CHAIN.ETHEREUM;
}
