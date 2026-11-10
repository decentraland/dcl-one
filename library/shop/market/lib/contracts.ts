// from: shop@94584a5c:app/src/lib/marketplace.ts; marketplace@f3956826:webapp/src/utils/trades.ts
// Copyright 2020 Metaverse Holdings Ltd. (marketplace), Apache-2.0.
// The contracts the seller and marketplace workflows sign against or send to. Upstream reads them from
// decentraland-transactions; the addresses and EIP-712 domain names here are that package at 3.3.0, the
// version the pinned Shop lockfile resolves (app/package-lock.json). New listings and bids go to the
// newest off-chain marketplace deployed on a chain (V3, then V2); a trade keeps settling on the version
// it names. The ABIs are the minimal fragments these flows call, checked against the pinned contract
// selectors in tests/fixtures/shop/offchain-marketplace-contract@87350c63e.

import { parseAbi, type Abi } from "viem";

import { evmAddress, type EvmAddress } from "../../model/address";
import { CHAIN, type ChainId } from "../../model/chain";

export type ContractData = { address: EvmAddress; name: string; version: string; chainId: ChainId };

const c = (chainId: ChainId, address: string, name: string, version: string): ContractData => ({
  chainId,
  address: evmAddress(address),
  name,
  version,
});

const MARKETPLACE_POLYGON = "DecentralandMarketplacePolygon";
const MARKETPLACE_ETHEREUM = "DecentralandMarketplaceEthereum";

/** decentraland-transactions OffChainMarketplaceV3, V2 and V1, per chain. */
const OFF_CHAIN_MARKETPLACES: Record<"v3" | "v2" | "v1", Partial<Record<ChainId, ContractData>>> = {
  v3: {
    [CHAIN.ETHEREUM]: c(CHAIN.ETHEREUM, "0x0f11d0d1671519683bd48abf3dbe779e300941cd", MARKETPLACE_ETHEREUM, "1.0.0"),
    [CHAIN.SEPOLIA]: c(CHAIN.SEPOLIA, "0x257db44ac97789c16ab277eae87dcde0c246cc9f", MARKETPLACE_ETHEREUM, "1.0.0"),
    [CHAIN.POLYGON]: c(CHAIN.POLYGON, "0xe38ef22abe871513555cba89adfe45ab4f548ada", MARKETPLACE_POLYGON, "1.0.0"),
    [CHAIN.AMOY]: c(CHAIN.AMOY, "0x36fd1434a6c4b8ade80c9847c1d15033ce34488c", MARKETPLACE_POLYGON, "1.0.0"),
  },
  v2: {
    [CHAIN.ETHEREUM]: c(CHAIN.ETHEREUM, "0x1b67d0e31eeb6b52d8eeed71d3616c2f5b33b8e7", MARKETPLACE_ETHEREUM, "1.0.0"),
    [CHAIN.SEPOLIA]: c(CHAIN.SEPOLIA, "0x1b67d0e31eeb6b52d8eeed71d3616c2f5b33b8e7", MARKETPLACE_ETHEREUM, "1.0.0"),
    [CHAIN.POLYGON]: c(CHAIN.POLYGON, "0xa40b1d129b8906888720686f3a01921ddf37716f", MARKETPLACE_POLYGON, "1.0.0"),
    [CHAIN.AMOY]: c(CHAIN.AMOY, "0x1b67d0e31eeb6b52d8eeed71d3616c2f5b33b8e7", MARKETPLACE_POLYGON, "1.0.0"),
  },
  v1: {
    [CHAIN.ETHEREUM]: c(CHAIN.ETHEREUM, "0x2d6b3508f9aca32d2550f92b2addba932e73c1ff", MARKETPLACE_ETHEREUM, "1.0.0"),
    [CHAIN.POLYGON]: c(CHAIN.POLYGON, "0x540fb08edb56aae562864b390542c97f562825ba", MARKETPLACE_POLYGON, "1.0.0"),
    [CHAIN.AMOY]: c(CHAIN.AMOY, "0x6ab20ae56673ed65f520b7be332aeb61b3ed727d", MARKETPLACE_POLYGON, "1.0.0"),
  },
};

/** Upstream `getLatestOffChainMarketplaceContract`: where a NEW listing or bid is signed. */
export function latestOffChainMarketplace(chainId: ChainId): ContractData {
  const found = OFF_CHAIN_MARKETPLACES.v3[chainId] ?? OFF_CHAIN_MARKETPLACES.v2[chainId];
  if (!found) throw new Error(`No off-chain marketplace contract exists on chain ${chainId}`);
  return found;
}

/** Upstream `getMarketplaceForTrade`: the deployed version a trade names on its own chain, or null. */
export function marketplaceForTrade(trade: { contract: string; chainId: number }): ContractData | null {
  const wanted = trade.contract.toLowerCase();
  for (const version of ["v3", "v2", "v1"] as const) {
    const found = OFF_CHAIN_MARKETPLACES[version][trade.chainId];
    if (found && found.address === wanted) return found;
  }
  return null;
}

/** Every off-chain marketplace version on a chain, newest first (grants on older ones stay live). */
export function deployedOffChainMarketplaces(chainId: ChainId): ContractData[] {
  return (["v3", "v2", "v1"] as const).flatMap((v) => OFF_CHAIN_MARKETPLACES[v][chainId] ?? []);
}

export const MANA_TOKEN: Partial<Record<ChainId, ContractData>> = {
  [CHAIN.ETHEREUM]: c(CHAIN.ETHEREUM, "0x0f5d2fb29fb7d3cfee444a200298f468908cc942", "MANAToken", "1"),
  [CHAIN.SEPOLIA]: c(CHAIN.SEPOLIA, "0xfa04d2e2ba9aec166c93dfeeba7427b2303befa9", "MANAToken", "1"),
  [CHAIN.POLYGON]: c(CHAIN.POLYGON, "0xa1c57f48f0deb89f569dfbe6e2b7f46d33606fd4", "(PoS) Decentraland MANA", "1"),
  [CHAIN.AMOY]: c(CHAIN.AMOY, "0x7ad72b9f944ea9793cf4055d88f81138cc2c63a0", "Decentraland MANA(PoS)", "1"),
};

export function manaToken(chainId: ChainId): ContractData {
  const found = MANA_TOKEN[chainId];
  if (!found) throw new Error(`MANA is not deployed on chain ${chainId}`);
  return found;
}

/** The legacy (pre-trades) marketplaces: `executeOrder`/`cancelOrder` on existing on-chain orders. */
export const LEGACY_MARKETPLACE: Partial<Record<ChainId, ContractData>> = {
  [CHAIN.ETHEREUM]: c(CHAIN.ETHEREUM, "0x8e5660b4ab70168b5a6feea0e0315cb49c8cd539", "Decentraland Marketplace", "1"),
  [CHAIN.SEPOLIA]: c(CHAIN.SEPOLIA, "0xccf0c17da6cd68041b1bf0f7e015767242077d8c", "Decentraland Marketplace", "1"),
  [CHAIN.POLYGON]: c(CHAIN.POLYGON, "0x480a0f4e360e8964e68858dd231c2922f1df45ef", "Decentraland Marketplace", "2"),
  [CHAIN.AMOY]: c(CHAIN.AMOY, "0x0c8ad1f6aadf89d2eb19f01a100a6143108fe2b0", "Decentraland Marketplace", "2"),
};

/** The legacy on-chain Bid contracts (a bid without a trade id). */
export const LEGACY_BID: Partial<Record<ChainId, ContractData>> = {
  [CHAIN.ETHEREUM]: c(CHAIN.ETHEREUM, "0xe479dfd9664c693b2e2992300930b00bfde08233", "Decentraland Bid", "1"),
  [CHAIN.SEPOLIA]: c(CHAIN.SEPOLIA, "0x2c2835b95852fd975e087b3b25297322728792e2", "Decentraland Bid", "1"),
  [CHAIN.POLYGON]: c(CHAIN.POLYGON, "0xb96697fa4a3361ba35b774a42c58daccaad1b8e1", "Decentraland Bid", "2"),
  [CHAIN.AMOY]: c(CHAIN.AMOY, "0x4b66eab79cc03a96fb6275cfcdf23c0db431606d", "Decentraland Bid", "2"),
};

/** CouponManager V3 (the one OffChainMarketplaceV3 redeems through) and V2, per chain. */
export const COUPON_MANAGERS: Record<"v3" | "v2", Partial<Record<ChainId, ContractData>>> = {
  v3: {
    [CHAIN.ETHEREUM]: c(CHAIN.ETHEREUM, "0xf9180eed9fcd5f8b3921c1b8caeb771c10faeb26", "CouponManager", "1.0.0"),
    [CHAIN.SEPOLIA]: c(CHAIN.SEPOLIA, "0xed558211ae5ae57a6704423918cb9b8501051af0", "CouponManager", "1.0.0"),
    [CHAIN.POLYGON]: c(CHAIN.POLYGON, "0x655fdfa91d69ea49f4ce1a8f7f7e2622c8630813", "CouponManager", "1.0.0"),
    [CHAIN.AMOY]: c(CHAIN.AMOY, "0x6c956587d9fe70032781edcdc626310648575382", "CouponManager", "1.0.0"),
  },
  v2: {
    [CHAIN.POLYGON]: c(CHAIN.POLYGON, "0x3fd3056ee72a2a85e9392fab3a450e7736536081", "CouponManager", "1.0.0"),
    [CHAIN.AMOY]: c(CHAIN.AMOY, "0xa40b1d129b8906888720686f3a01921ddf37716f", "CouponManager", "1.0.0"),
  },
};

/** Collections only live on Polygon, so the discount coupon has no Ethereum deployment. */
export const COLLECTION_DISCOUNT_COUPON: Partial<Record<ChainId, ContractData>> = {
  [CHAIN.POLYGON]: c(CHAIN.POLYGON, "0xc914507fe297b2dddd1232ac3a8903f1c125e794", "CollectionDiscountCoupon", "1.0.0"),
  [CHAIN.AMOY]: c(CHAIN.AMOY, "0x4ee8f6b87f4917a3bbc7c8bb3a06db8555f83db9", "CollectionDiscountCoupon", "1.0.0"),
};

/** ERC721CollectionV2's fixed native meta-transaction domain; only the address differs per collection. */
export function collectionV2(chainId: ChainId, address: string): ContractData {
  return c(chainId, address, "Decentraland Collection", "2");
}

const CHECKS_COMPONENTS = [
  { name: "uses", type: "uint256" },
  { name: "expiration", type: "uint256" },
  { name: "effective", type: "uint256" },
  { name: "salt", type: "bytes32" },
  { name: "contractSignatureIndex", type: "uint256" },
  { name: "signerSignatureIndex", type: "uint256" },
  { name: "allowedRoot", type: "bytes32" },
  { name: "allowedProof", type: "bytes32[]" },
  {
    name: "externalChecks",
    type: "tuple[]",
    components: [
      { name: "contractAddress", type: "address" },
      { name: "selector", type: "bytes4" },
      { name: "value", type: "bytes" },
      { name: "required", type: "bool" },
    ],
  },
] as const;

const ASSET_COMPONENTS = [
  { name: "assetType", type: "uint256" },
  { name: "contractAddress", type: "address" },
  { name: "value", type: "uint256" },
  { name: "beneficiary", type: "address" },
  { name: "extra", type: "bytes" },
] as const;

/** `IMarketplace.Trade` (the pinned structs.json layout). */
const TRADE_COMPONENTS = [
  { name: "signer", type: "address" },
  { name: "signature", type: "bytes" },
  { name: "checks", type: "tuple", components: CHECKS_COMPONENTS },
  { name: "sent", type: "tuple[]", components: ASSET_COMPONENTS },
  { name: "received", type: "tuple[]", components: ASSET_COMPONENTS },
] as const;

/** `CouponTypes.Coupon`. */
const COUPON_COMPONENTS = [
  { name: "signature", type: "bytes" },
  { name: "checks", type: "tuple", components: CHECKS_COMPONENTS },
  { name: "couponAddress", type: "address" },
  { name: "data", type: "bytes" },
  { name: "callerData", type: "bytes" },
] as const;

const tradesFn = (name: "accept" | "cancelSignature") =>
  ({
    type: "function",
    name,
    stateMutability: "nonpayable",
    inputs: [{ name: "_trades", type: "tuple[]", components: TRADE_COMPONENTS }],
    outputs: [],
  }) as const;

export const MARKETPLACE_ABI = [
  tradesFn("accept"),
  tradesFn("cancelSignature"),
  ...parseAbi([
    "function contractSignatureIndex() view returns (uint256)",
    "function signerSignatureIndex(address) view returns (uint256)",
    "function getNonce(address) view returns (uint256)",
    "function executeMetaTransaction(address _userAddress, bytes _functionData, bytes _signature) payable returns (bytes)",
  ]),
] as const satisfies Abi;

export const COUPON_MANAGER_ABI = [
  {
    type: "function",
    name: "cancelSignature",
    stateMutability: "nonpayable",
    inputs: [{ name: "_coupons", type: "tuple[]", components: COUPON_COMPONENTS }],
    outputs: [],
  },
  ...parseAbi(["function contractSignatureIndex() view returns (uint256)", "function signerSignatureIndex(address) view returns (uint256)"]),
] as const satisfies Abi;

export const ERC721_ABI = parseAbi([
  "function transferFrom(address from, address to, uint256 tokenId)",
  "function safeTransferFrom(address from, address to, uint256 tokenId)",
  "function setApprovalForAll(address operator, bool approved)",
  "function isApprovedForAll(address owner, address operator) view returns (bool)",
  "function ownerOf(uint256 tokenId) view returns (address)",
]);

export const COLLECTION_V2_ABI = parseAbi([
  "function issueTokens(address[] _beneficiaries, uint256[] _itemIds)",
  "function setMinters(address[] _minters, bool[] _values)",
  "function globalMinters(address) view returns (bool)",
  "function creator() view returns (address)",
  "function getNonce(address user) view returns (uint256 nonce)",
  "function executeMetaTransaction(address userAddress, bytes functionSignature, bytes32 sigR, bytes32 sigS, uint8 sigV) payable returns (bytes)",
]);

export const ERC20_ABI = parseAbi([
  "function approve(address spender, uint256 amount) returns (bool)",
  "function allowance(address owner, address spender) view returns (uint256)",
  "function balanceOf(address owner) view returns (uint256)",
]);

export const LEGACY_MARKETPLACE_ABI = parseAbi([
  "function executeOrder(address nftAddress, uint256 assetId, uint256 price)",
  "function safeExecuteOrder(address nftAddress, uint256 assetId, uint256 price, bytes fingerprint)",
  "function cancelOrder(address nftAddress, uint256 assetId)",
]);

export const LEGACY_BID_ABI = parseAbi(["function cancelBid(address _tokenAddress, uint256 _tokenId)"]);

/** ERC721 `safeTransferFrom(from, bidContract, tokenId, bidId)`: how the owner accepts a legacy bid. */
export const LEGACY_ACCEPT_BID_ABI = parseAbi(["function safeTransferFrom(address from, address to, uint256 tokenId, bytes data)"]);
