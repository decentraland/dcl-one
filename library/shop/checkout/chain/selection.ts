import { CHAIN, type ChainId } from "../../model/chain";

export type ShopChainSelection = {
  settlementChainId?: ChainId;
  ethereumChainId?: ChainId;
};

export function selectSettlementChain(selection: ShopChainSelection): ChainId {
  return selection.settlementChainId ?? CHAIN.POLYGON;
}

export function selectEthereumChain(selection: ShopChainSelection): ChainId {
  const fallback = selectSettlementChain(selection) === CHAIN.AMOY ? CHAIN.SEPOLIA : CHAIN.ETHEREUM;
  return selection.ethereumChainId ?? fallback;
}
