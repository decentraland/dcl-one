// from: marketplace@f3956826:webapp/src/modules/vendor/decentraland/contracts.ts (LAND, Estates, Names),
// decentraland-transactions@0556256:src/contracts/{rentals,dclControllerV2,dclRegistrar,manaToken}.ts
// Copyright 2020 Metaverse Holdings Ltd. Licensed under the Apache License, Version 2.0.
// The Ethereum contracts the LAND and NAME flows read and write, per network. Every one of them is a
// MANA-only surface except the NAME registrar controller, which is the direct-MANA registration route
// the payment rule offers a web3 wallet second, after credits.

import { evmAddress, type EvmAddress } from "../model/address";
import { CHAIN, type ChainId } from "../model/chain";

export type LandNetwork = {
  chainId: ChainId;
  /** The public read-only JSON-RPC path under the node (`/rpc/{network}`, the rpc.decentraland.org relay). */
  rpcNetwork: string;
  land: EvmAddress;
  estate: EvmAddress;
  rentals: EvmAddress;
  mana: EvmAddress;
  nameRegistrar: EvmAddress;
  nameController: EvmAddress;
};

export const LAND_NETWORKS: Record<number, LandNetwork> = {
  [CHAIN.ETHEREUM]: {
    chainId: CHAIN.ETHEREUM,
    rpcNetwork: "mainnet",
    land: evmAddress("0xf87e31492faf9a91b02ee0deaad50d51d56d5d4d"),
    estate: evmAddress("0x959e104e1a4db6317fa58f8295f586e1a978c297"),
    rentals: evmAddress("0x3a1469499d0be105d4f77045ca403a5f6dc2f3f5"),
    mana: evmAddress("0x0f5d2fb29fb7d3cfee444a200298f468908cc942"),
    nameRegistrar: evmAddress("0x2a187453064356c898cae034eaed119e1663acb8"),
    nameController: evmAddress("0xbe92b49aee993adea3a002adcda189a2b7dec56c"),
  },
  [CHAIN.SEPOLIA]: {
    chainId: CHAIN.SEPOLIA,
    rpcNetwork: "sepolia",
    land: evmAddress("0x42f4ba48791e2de32f5fbf553441c2672864bb33"),
    estate: evmAddress("0x369a7fbe718c870c79f99fb423882e8dd8b20486"),
    rentals: evmAddress("0xe70db6319e9cee3f604909bdade58d1f5c1cf702"),
    mana: evmAddress("0xfa04d2e2ba9aec166c93dfeeba7427b2303befa9"),
    nameRegistrar: evmAddress("0x7518456ae93eb98f3e64571b689c626616bb7f30"),
    nameController: evmAddress("0xd2046364317c21fa8d121d84185c39e6e910cf89"),
  },
};

/** The Rentals contract's EIP-712 domain (`Rentals`, version 1), as the signatures server verifies it. */
export const RENTALS_DOMAIN = { name: "Rentals", version: "1" } as const;

type ShopEnvWindow = Window & typeof globalThis & { __SHOP_ETHEREUM_CHAIN_ID__?: number };

/** The Ethereum network the Shop's LAND and NAME flows run on: mainnet unless the host says Sepolia. */
export function landChainId(): ChainId {
  const fromWindow = typeof window !== "undefined" ? (window as ShopEnvWindow).__SHOP_ETHEREUM_CHAIN_ID__ : undefined;
  const fromEnv = typeof import.meta !== "undefined" ? Number(import.meta.env?.VITE_SHOP_ETHEREUM_CHAIN_ID ?? NaN) : NaN;
  const chosen = fromWindow ?? (Number.isFinite(fromEnv) ? fromEnv : CHAIN.ETHEREUM);
  return LAND_NETWORKS[chosen] ? chosen : CHAIN.ETHEREUM;
}

export function landNetwork(chainId: ChainId = landChainId()): LandNetwork {
  const found = LAND_NETWORKS[chainId];
  if (!found) throw new RangeError(`LAND and NAMEs do not live on chain ${chainId}`);
  return found;
}

export type LandCategory = "parcel" | "estate";

/** Which LAND registry a contract is, on any configured network; null for anything else. */
export function landCategoryOf(contract: string): { category: LandCategory; network: LandNetwork } | null {
  const lower = contract.toLowerCase();
  for (const network of Object.values(LAND_NETWORKS)) {
    if (network.land === lower) return { category: "parcel", network };
    if (network.estate === lower) return { category: "estate", network };
  }
  return null;
}

export function isNameRegistrar(contract: string): boolean {
  const lower = contract.toLowerCase();
  return Object.values(LAND_NETWORKS).some((n) => n.nameRegistrar === lower);
}
