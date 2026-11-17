// A signed off-chain trade as the Shop API serves it (`GET /v1/trades/{id}`) and the marketplace verifies it: its
// assets, checks and coupon. Read with the listing; encoding it for a purchase is checkout/purchase/encoding.ts.

import type { ShopCoupon } from "@catalyst-api/market/ShopCoupon";

/** The off-chain marketplace asset types (`src/shop/evm/trade.rs`, `ASSET_TYPE_*`). */
export const ASSET_TYPE = { ERC20: 1, USD_PEGGED_MANA: 2, ERC721: 3, COLLECTION_ITEM: 4 } as const;

export type TradeAssetWire = {
  assetType: number;
  contractAddress: string;
  extra?: string;
  amount?: string;
  tokenId?: string;
  itemId?: string;
  /** On-chain-shaped assets carry a generic value. */
  value?: string;
  beneficiary?: string;
};

export type TradeChecksWire = {
  uses: number;
  /** Stored in milliseconds, signed and checked on chain in seconds. */
  expiration: number;
  effective: number;
  salt: string;
  contractSignatureIndex: number;
  signerSignatureIndex: number;
  allowedRoot?: string;
  externalChecks?: { contractAddress: string; selector: string; value: string; required: boolean }[];
};

/** `GET /v1/trades/{id}`: a signed listing, exactly what the marketplace verifies. */
export type SignedTrade = {
  id: string;
  chainId: number;
  /** The marketplace version the trade was signed for. */
  contract: string;
  signer: string;
  signature: string;
  network?: string;
  type?: string;
  createdAt?: number;
  checks: TradeChecksWire;
  sent: TradeAssetWire[];
  received: TradeAssetWire[];
};

/** A creator's rate coupon with the Merkle proof for this listing's collection (the catalogue row). */
export type ListingCoupon = ShopCoupon;
