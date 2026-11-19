// The Ethereum network the Shop's LAND and NAME flows run on, in the browser. The networks themselves,
// and everything read from them, take the chain as an argument (@shop/land/contracts).

import { CHAIN, type ChainId } from "@shop/model/chain";
import { ethereumChainId } from "../checkout/chain/config";
import { LAND_NETWORKS } from "@shop/land/contracts";
import { pageOverride } from "../pageOverride";

/**
 * The Ethereum network the Shop's LAND and NAME flows run on: a test's or a local build's choice from the page
 * (../pageOverride), else the Shop's own
 * (`ethereumChainId`: VITE_SHOP_ETHEREUM_CHAIN_ID, or Sepolia when the Shop settles on Amoy, mainnet otherwise).
 */
export function landChainId(): ChainId {
  const chosen = pageOverride<number>("__SHOP_ETHEREUM_CHAIN_ID__") ?? ethereumChainId();
  return LAND_NETWORKS[chosen] ? chosen : CHAIN.ETHEREUM;
}
