// from: decentraland-transactions@3.3.0:src/contracts/{bid,bidV2,rentals,manaToken,collectionStore,creditsManager,offChainMarketplace,offChainMarketplaceV2,offChainMarketplaceV3,couponManagerV2,couponManagerV3,erc721CollectionV2,marketplaceV2}.ts
//       (the version shop@94584a5c's app/package-lock.json resolves), cross-checked with docs/shop/CONTRACTS.md "Addresses"
import { evmAddress, type EvmAddress } from "../../model/address";
import { CHAIN, type ChainId } from "../../model/chain";

export type MetaTxStyle = "functionData" | "functionSignature";

export type ContractInfo = {
  address: EvmAddress;
  chainId: ChainId;
  name: string;
  version: string;
  metaTx: MetaTxStyle | null;
  label: string;
};

export type OffChainMarketplace = ContractInfo & {
  couponManager: EvmAddress | null;
};

export type ChainContracts = {
  chainId: ChainId;
  mana: ContractInfo;
  creditsManager: ContractInfo | null;
  collectionStore: ContractInfo | null;
  offChainMarketplaces: OffChainMarketplace[];
  legacyMarketplace: ContractInfo | null;
  bids: ContractInfo | null;
  rentals: ContractInfo | null;
};

const c = (
  chainId: ChainId,
  label: string,
  address: string,
  name: string,
  version: string,
  metaTx: MetaTxStyle | null,
): ContractInfo => ({ chainId, label, address: evmAddress(address), name, version, metaTx });

const market = (chainId: ChainId, label: string, address: string, name: string, couponManager: string | null): OffChainMarketplace => ({
  ...c(chainId, label, address, name, "1.0.0", "functionData"),
  couponManager: couponManager ? evmAddress(couponManager) : null,
});

const POLYGON: ChainContracts = {
  chainId: CHAIN.POLYGON,
  mana: c(CHAIN.POLYGON, "MANAToken", "0xa1c57f48f0deb89f569dfbe6e2b7f46d33606fd4", "(PoS) Decentraland MANA", "1", "functionSignature"),
  creditsManager: c(CHAIN.POLYGON, "CreditsManager", "0x8b3a40ca1b6f5cafc99d112a4d02e897d1fd8cc5", "Decentraland Credits", "1.0.0", "functionData"),
  collectionStore: c(
    CHAIN.POLYGON,
    "CollectionStore",
    "0x214ffc0f0103735728dc66b61a22e4f163e275ae",
    "Decentraland Collection Store",
    "1",
    "functionSignature",
  ),
  offChainMarketplaces: [
    market(CHAIN.POLYGON, "OffChainMarketplaceV3", "0xe38ef22abe871513555cba89adfe45ab4f548ada", "DecentralandMarketplacePolygon", "0x655fdfa91d69ea49f4ce1a8f7f7e2622c8630813"),
    market(CHAIN.POLYGON, "OffChainMarketplaceV2", "0xa40b1d129b8906888720686f3a01921ddf37716f", "DecentralandMarketplacePolygon", "0x3fd3056ee72a2a85e9392fab3a450e7736536081"),
    market(CHAIN.POLYGON, "OffChainMarketplace", "0x540fb08edb56aae562864b390542c97f562825ba", "DecentralandMarketplacePolygon", null),
  ],
  legacyMarketplace: c(CHAIN.POLYGON, "MarketplaceV2", "0x480a0f4e360e8964e68858dd231c2922f1df45ef", "Decentraland Marketplace", "2", "functionSignature"),
  bids: c(CHAIN.POLYGON, "BidV2", "0xb96697fa4a3361ba35b774a42c58daccaad1b8e1", "Decentraland Bid", "2", "functionSignature"),
  rentals: null,
};

const AMOY: ChainContracts = {
  chainId: CHAIN.AMOY,
  mana: c(CHAIN.AMOY, "MANAToken", "0x7ad72b9f944ea9793cf4055d88f81138cc2c63a0", "Decentraland MANA(PoS)", "1", "functionSignature"),
  creditsManager: c(CHAIN.AMOY, "CreditsManager", "0x8052a560e6e6ac86eeb7e711a4497f639b322fb3", "Decentraland Credits", "1.0.0", "functionData"),
  collectionStore: c(
    CHAIN.AMOY,
    "CollectionStore",
    "0xe36abc9ec616c83caaa386541380829106149d68",
    "Decentraland Collection Store",
    "1",
    "functionSignature",
  ),
  offChainMarketplaces: [
    market(CHAIN.AMOY, "OffChainMarketplaceV3", "0x36fd1434a6c4b8ade80c9847c1d15033ce34488c", "DecentralandMarketplacePolygon", "0x6c956587d9fe70032781edcdc626310648575382"),
    market(CHAIN.AMOY, "OffChainMarketplaceV2", "0x1b67d0e31eeb6b52d8eeed71d3616c2f5b33b8e7", "DecentralandMarketplacePolygon", "0xa40b1d129b8906888720686f3a01921ddf37716f"),
    market(CHAIN.AMOY, "OffChainMarketplace", "0x6ab20ae56673ed65f520b7be332aeb61b3ed727d", "DecentralandMarketplacePolygon", null),
  ],
  legacyMarketplace: c(CHAIN.AMOY, "MarketplaceV2", "0x0c8ad1f6aadf89d2eb19f01a100a6143108fe2b0", "Decentraland Marketplace", "2", "functionSignature"),
  bids: c(CHAIN.AMOY, "BidV2", "0x4b66eab79cc03a96fb6275cfcdf23c0db431606d", "Decentraland Bid", "2", "functionSignature"),
  rentals: null,
};

const ETHEREUM: ChainContracts = {
  chainId: CHAIN.ETHEREUM,
  mana: c(CHAIN.ETHEREUM, "MANAToken", "0x0f5d2fb29fb7d3cfee444a200298f468908cc942", "MANAToken", "1", null),
  creditsManager: null,
  collectionStore: null,
  offChainMarketplaces: [
    market(CHAIN.ETHEREUM, "OffChainMarketplaceV3", "0x0f11d0d1671519683bd48abf3dbe779e300941cd", "DecentralandMarketplaceEthereum", "0xf9180eed9fcd5f8b3921c1b8caeb771c10faeb26"),
    market(CHAIN.ETHEREUM, "OffChainMarketplaceV2", "0x1b67d0e31eeb6b52d8eeed71d3616c2f5b33b8e7", "DecentralandMarketplaceEthereum", null),
    market(CHAIN.ETHEREUM, "OffChainMarketplace", "0x2d6b3508f9aca32d2550f92b2addba932e73c1ff", "DecentralandMarketplaceEthereum", null),
  ],
  legacyMarketplace: c(CHAIN.ETHEREUM, "Marketplace", "0x8e5660b4ab70168b5a6feea0e0315cb49c8cd539", "Decentraland Marketplace", "1", null),
  bids: c(CHAIN.ETHEREUM, "Bid", "0xe479dfd9664c693b2e2992300930b00bfde08233", "Decentraland Bid", "1", null),
  rentals: c(CHAIN.ETHEREUM, "Rentals", "0x3a1469499d0be105d4f77045ca403a5f6dc2f3f5", "Rentals", "1", null),
};

const SEPOLIA: ChainContracts = {
  chainId: CHAIN.SEPOLIA,
  mana: c(CHAIN.SEPOLIA, "MANAToken", "0xfa04d2e2ba9aec166c93dfeeba7427b2303befa9", "MANAToken", "1", null),
  creditsManager: null,
  collectionStore: null,
  offChainMarketplaces: [
    market(CHAIN.SEPOLIA, "OffChainMarketplaceV3", "0x257db44ac97789c16ab277eae87dcde0c246cc9f", "DecentralandMarketplaceEthereum", "0xed558211ae5ae57a6704423918cb9b8501051af0"),
    market(CHAIN.SEPOLIA, "OffChainMarketplaceV2", "0x1b67d0e31eeb6b52d8eeed71d3616c2f5b33b8e7", "DecentralandMarketplaceEthereum", null),
    market(CHAIN.SEPOLIA, "OffChainMarketplace", "0x54bdd3ffc1448bdab0251f05dd554eec803adb2f", "DecentralandMarketplaceEthereum", null),
  ],
  legacyMarketplace: null,
  bids: c(CHAIN.SEPOLIA, "Bid", "0x2c2835b95852fd975e087b3b25297322728792e2", "Decentraland Bid", "1", null),
  rentals: c(CHAIN.SEPOLIA, "Rentals", "0xe70db6319e9cee3f604909bdade58d1f5c1cf702", "Rentals", "1", null),
};

const BOOK: Record<number, ChainContracts> = {
  [CHAIN.POLYGON]: POLYGON,
  [CHAIN.AMOY]: AMOY,
  [CHAIN.ETHEREUM]: ETHEREUM,
  [CHAIN.SEPOLIA]: SEPOLIA,
};

export const COLLECTION_V2_DOMAIN = { name: "Decentraland Collection", version: "2" } as const;

export function chainContracts(chainId: ChainId): ChainContracts | null {
  return BOOK[chainId] ?? null;
}

export function requireContracts(chainId: ChainId): ChainContracts {
  const book = chainContracts(chainId);
  if (!book) throw new Error(`No Shop contracts are known on chain ${chainId}`);
  return book;
}

export function allContracts(): ContractInfo[] {
  return Object.values(BOOK).flatMap((b) =>
    [b.mana, b.creditsManager, b.collectionStore, b.legacyMarketplace, b.bids, b.rentals, ...b.offChainMarketplaces].filter(
      (x): x is ContractInfo => x !== null,
    ),
  );
}

export function contractAt(chainId: ChainId, address: string): ContractInfo | null {
  const lower = address.toLowerCase();
  return allContracts().find((x) => x.chainId === chainId && x.address === lower) ?? null;
}
