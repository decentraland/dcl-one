// GENERATED from library/catalyst-api/market by sites/scripts/gen-zod-schemas.mts. Do not edit.
import { z } from "zod";

import type { Account } from "@catalyst-api/market/Account";
import type { AssetsHttpResponse } from "@catalyst-api/market/AssetsHttpResponse";
import type { Bid } from "@catalyst-api/market/Bid";
import type { BidsEnvelope } from "@catalyst-api/market/BidsEnvelope";
import type { BidsPage } from "@catalyst-api/market/BidsPage";
import type { CatalogItem } from "@catalyst-api/market/CatalogItem";
import type { Collection } from "@catalyst-api/market/Collection";
import type { Contract } from "@catalyst-api/market/Contract";
import type { CreatorSearchHit } from "@catalyst-api/market/CreatorSearchHit";
import type { CreditCatalogItem } from "@catalyst-api/market/CreditCatalogItem";
import type { DataOnly } from "@catalyst-api/market/DataOnly";
import type { DataTotal } from "@catalyst-api/market/DataTotal";
import type { DataTotalString } from "@catalyst-api/market/DataTotalString";
import type { EmoteData } from "@catalyst-api/market/EmoteData";
import type { EnsData } from "@catalyst-api/market/EnsData";
import type { EstateData } from "@catalyst-api/market/EstateData";
import type { EstateParcel } from "@catalyst-api/market/EstateParcel";
import type { FavoriteList } from "@catalyst-api/market/FavoriteList";
import type { GroupedWearable } from "@catalyst-api/market/GroupedWearable";
import type { ImportableListing } from "@catalyst-api/market/ImportableListing";
import type { ImportableResponseBody } from "@catalyst-api/market/ImportableResponseBody";
import type { IndividualData } from "@catalyst-api/market/IndividualData";
import type { Item } from "@catalyst-api/market/Item";
import type { LegacyListing } from "@catalyst-api/market/LegacyListing";
import type { ListPickItem } from "@catalyst-api/market/ListPickItem";
import type { ListsEnvelope } from "@catalyst-api/market/ListsEnvelope";
import type { ListsPage } from "@catalyst-api/market/ListsPage";
import type { NameOnly } from "@catalyst-api/market/NameOnly";
import type { Network } from "@catalyst-api/market/Network";
import type { Nft } from "@catalyst-api/market/Nft";
import type { NftCategory } from "@catalyst-api/market/NftCategory";
import type { NftData } from "@catalyst-api/market/NftData";
import type { NftResult } from "@catalyst-api/market/NftResult";
import type { NftsResponseBody } from "@catalyst-api/market/NftsResponseBody";
import type { Order } from "@catalyst-api/market/Order";
import type { Owner } from "@catalyst-api/market/Owner";
import type { PaginatedAssetsBody } from "@catalyst-api/market/PaginatedAssetsBody";
import type { ParcelData } from "@catalyst-api/market/ParcelData";
import type { ParcelEstate } from "@catalyst-api/market/ParcelEstate";
import type { PicksCount } from "@catalyst-api/market/PicksCount";
import type { PicksEnvelope } from "@catalyst-api/market/PicksEnvelope";
import type { PicksPage } from "@catalyst-api/market/PicksPage";
import type { PicksStatsEnvelope } from "@catalyst-api/market/PicksStatsEnvelope";
import type { PickStats } from "@catalyst-api/market/PickStats";
import type { PickUnpickEnvelope } from "@catalyst-api/market/PickUnpickEnvelope";
import type { PickUnpickResult } from "@catalyst-api/market/PickUnpickResult";
import type { ProfileName } from "@catalyst-api/market/ProfileName";
import type { ProfileWearable } from "@catalyst-api/market/ProfileWearable";
import type { RelatedResponseBody } from "@catalyst-api/market/RelatedResponseBody";
import type { RentalListing } from "@catalyst-api/market/RentalListing";
import type { RentalListingPeriod } from "@catalyst-api/market/RentalListingPeriod";
import type { Sale } from "@catalyst-api/market/Sale";
import type { SalesSummary } from "@catalyst-api/market/SalesSummary";
import type { SalesSummaryCollection } from "@catalyst-api/market/SalesSummaryCollection";
import type { SalesSummaryItem } from "@catalyst-api/market/SalesSummaryItem";
import type { SalesSummaryRoyalties } from "@catalyst-api/market/SalesSummaryRoyalties";
import type { ShopCoupon } from "@catalyst-api/market/ShopCoupon";
import type { ShopCouponRow } from "@catalyst-api/market/ShopCouponRow";
import type { ShopListing } from "@catalyst-api/market/ShopListing";
import type { ShopSaleFields } from "@catalyst-api/market/ShopSaleFields";
import type { SuggestedCollection } from "@catalyst-api/market/SuggestedCollection";
import type { SuggestedItem } from "@catalyst-api/market/SuggestedItem";
import type { SuggestionReason } from "@catalyst-api/market/SuggestionReason";
import type { SuggestionReasonKind } from "@catalyst-api/market/SuggestionReasonKind";
import type { SuggestionsResponse } from "@catalyst-api/market/SuggestionsResponse";
import type { TopCreator } from "@catalyst-api/market/TopCreator";
import type { TopCreatorsResponseBody } from "@catalyst-api/market/TopCreatorsResponseBody";
import type { TrendingItem } from "@catalyst-api/market/TrendingItem";
import type { TrendingResponseBody } from "@catalyst-api/market/TrendingResponseBody";
import type { UnifiedItem } from "@catalyst-api/market/UnifiedItem";
import type { UnifiedListing } from "@catalyst-api/market/UnifiedListing";
import type { UrnToken } from "@catalyst-api/market/UrnToken";
import type { WearableData } from "@catalyst-api/market/WearableData";

export const AccountSchema = z.object({
  id: z.string(),
  address: z.string(),
  sales: z.number(),
  purchases: z.number(),
  spent: z.string(),
  earned: z.string(),
  royalties: z.string(),
  collections: z.number(),
});

export const PaginatedAssetsBodySchema = <T extends z.ZodType>(t: T) =>
  z.object({
    elements: z.array(t),
    page: z.number(),
    pages: z.number(),
    limit: z.number(),
    total: z.number(),
    totalItems: z.number().optional(),
  });

export const AssetsHttpResponseSchema = <T extends z.ZodType>(t: T) =>
  z.object({
    ok: z.boolean(),
    data: PaginatedAssetsBodySchema(t),
  });

export const NetworkSchema = z.enum(["ETHEREUM", "MATIC"]);

export const BidSchema = z.object({
  id: z.string(),
  bidder: z.string(),
  price: z.string(),
  createdAt: z.number(),
  updatedAt: z.number(),
  fingerprint: z.string(),
  status: z.string(),
  seller: z.string(),
  network: NetworkSchema,
  chainId: z.number(),
  contractAddress: z.string(),
  expiresAt: z.number(),
  tokenId: z.string().optional(),
  itemId: z.string().optional(),
  tradeId: z.string().optional(),
  tradeContractAddress: z.string().optional(),
  bidAddress: z.string().optional(),
  blockchainId: z.string().optional(),
  blockNumber: z.string().optional(),
});

export const BidsPageSchema = z.object({
  results: z.array(BidSchema),
  total: z.number(),
  page: z.number(),
  pages: z.number(),
  limit: z.number(),
});

export const BidsEnvelopeSchema = z.object({
  ok: z.boolean(),
  data: BidsPageSchema,
});

export const PickStatsSchema = z.object({
  count: z.number(),
  itemId: z.string(),
  pickedByUser: z.boolean().optional(),
});

export const CatalogItemSchema = z.object({
  id: z.string(),
  beneficiary: z.string().nullable(),
  itemId: z.string(),
  name: z.string(),
  thumbnail: z.string(),
  url: z.string(),
  urn: z.string(),
  category: z.string(),
  contractAddress: z.string(),
  rarity: z.string(),
  available: z.number(),
  isOnSale: z.boolean(),
  tradeId: z.string().optional(),
  creator: z.string(),
  data: z.record(z.string(), z.unknown()),
  network: NetworkSchema,
  chainId: z.number(),
  price: z.string(),
  createdAt: z.number(),
  updatedAt: z.number(),
  reviewedAt: z.number(),
  firstListedAt: z.number().nullable(),
  soldAt: z.number(),
  minPrice: z.string().optional(),
  maxListingPrice: z.string().nullable(),
  minListingPrice: z.string().nullable(),
  listings: z.number().nullable(),
  owners: z.number().nullable(),
  picks: PickStatsSchema.nullable(),
});

export const CollectionSchema = z.object({
  urn: z.string(),
  creator: z.string(),
  name: z.string(),
  contractAddress: z.string(),
  createdAt: z.number(),
  updatedAt: z.number(),
  reviewedAt: z.number(),
  isOnSale: z.boolean(),
  size: z.number(),
  network: NetworkSchema,
  chainId: z.number(),
  firstListedAt: z.number().nullable(),
});

export const NftCategorySchema = z.enum(["parcel", "estate", "wearable", "ens", "emote"]);

export const ContractSchema = z.object({
  name: z.string(),
  address: z.string(),
  category: NftCategorySchema,
  network: NetworkSchema,
  chainId: z.number(),
});

export const CreatorSearchHitSchema = z.object({
  address: z.string(),
  name: z.string(),
  face: z.string().nullable(),
  items: z.number(),
  collections: z.number(),
});

export const PicksCountSchema = z.object({
  count: z.number(),
});

export const CreditCatalogItemSchema = z.object({
  priceCredits: z.number(),
  id: z.string(),
  name: z.string(),
  thumbnail: z.string(),
  url: z.string(),
  category: NftCategorySchema,
  contractAddress: z.string(),
  itemId: z.string(),
  rarity: z.string(),
  price: z.string(),
  available: z.number(),
  isOnSale: z.boolean(),
  creator: z.string(),
  beneficiary: z.string().nullable(),
  createdAt: z.number(),
  updatedAt: z.number(),
  reviewedAt: z.number(),
  soldAt: z.number(),
  data: z.record(z.string(), z.unknown()),
  network: NetworkSchema,
  chainId: z.number(),
  urn: z.string(),
  firstListedAt: z.number().optional(),
  picks: PicksCountSchema,
  utility: z.string().nullable(),
  tradeId: z.string().nullable(),
  tradeExpiresAt: z.number().optional(),
  tradeContractAddress: z.string().nullable(),
});

export const DataOnlySchema = <T extends z.ZodType>(t: T) =>
  z.object({
    data: t,
  });

export const DataTotalSchema = <T extends z.ZodType>(t: T) =>
  z.object({
    data: z.array(t),
    total: z.number(),
  });

export const DataTotalStringSchema = <T extends z.ZodType>(t: T) =>
  z.object({
    data: z.array(t),
    total: z.string(),
  });

export const EmoteDataSchema = z.object({
  bodyShapes: z.array(z.string()),
  category: z.string(),
  description: z.string(),
  rarity: z.string(),
  loop: z.boolean(),
  hasSound: z.boolean(),
  hasGeometry: z.boolean(),
  outcomeType: z.string().nullable(),
});

export const EnsDataSchema = z.object({
  subdomain: z.string(),
});

export const EstateParcelSchema = z.object({
  x: z.number(),
  y: z.number(),
});

export const EstateDataSchema = z.object({
  size: z.number(),
  description: z.string().nullable(),
  parcels: z.array(EstateParcelSchema),
});

export const FavoriteListSchema = z.object({
  id: z.string(),
  name: z.string(),
  description: z.string().optional(),
  userAddress: z.string(),
  createdAt: z.number(),
  updatedAt: z.number().optional(),
  isPrivate: z.boolean(),
  permission: z.string().optional(),
  isDefaultList: z.boolean(),
  itemsCount: z.number(),
  previewOfItemIds: z.array(z.string()),
  isItemInList: z.boolean().optional(),
});

export const IndividualDataSchema = z.object({
  id: z.string(),
  tokenId: z.string(),
  transferredAt: z.string(),
  price: z.string(),
});

export const GroupedWearableSchema = z.object({
  urn: z.string(),
  amount: z.string(),
  individualData: z.array(IndividualDataSchema),
  name: z.string(),
  rarity: z.string(),
  minTransferredAt: z.string(),
  maxTransferredAt: z.string(),
  category: z.string(),
  itemType: z.string().optional(),
  status: z.string().optional(),
  unlockAt: z.number().optional(),
});

export const ImportableListingSchema = z.object({
  oldTradeId: z.string(),
  listingType: z.string(),
  contractAddress: z.string(),
  itemId: z.string().nullable(),
  tokenId: z.string().nullable(),
  name: z.string(),
  thumbnail: z.string(),
  rarity: z.string(),
  category: z.string(),
  wearableCategory: z.string().nullable(),
  manaWei: z.string(),
  available: z.number(),
  network: NetworkSchema,
  chainId: z.number(),
});

export const ImportableResponseBodySchema = z.object({
  data: z.array(ImportableListingSchema),
});

export const ItemSchema = z.object({
  id: z.string(),
  name: z.string(),
  thumbnail: z.string(),
  url: z.string(),
  category: NftCategorySchema,
  contractAddress: z.string(),
  itemId: z.string(),
  rarity: z.string(),
  price: z.string(),
  available: z.number(),
  isOnSale: z.boolean(),
  creator: z.string(),
  beneficiary: z.string().nullable(),
  createdAt: z.number(),
  updatedAt: z.number(),
  reviewedAt: z.number(),
  soldAt: z.number(),
  data: z.record(z.string(), z.unknown()),
  network: NetworkSchema,
  chainId: z.number(),
  urn: z.string(),
  firstListedAt: z.number().optional(),
  picks: PicksCountSchema,
  utility: z.string().nullable(),
  tradeId: z.string().nullable(),
  tradeExpiresAt: z.number().optional(),
  tradeContractAddress: z.string().nullable(),
});

export const LegacyListingSchema = z.object({
  tradeId: z.string(),
  listingType: z.string(),
  contractAddress: z.string(),
  itemId: z.string().nullable(),
  name: z.string(),
  thumbnail: z.string(),
  rarity: z.string(),
  category: z.string(),
  wearableCategory: z.string().nullable(),
  gender: z.enum(["male", "female", "unisex"]).nullable(),
  creator: z.string(),
  manaWei: z.string(),
  available: z.number(),
  network: NetworkSchema,
  chainId: z.number(),
  createdAt: z.number(),
});

export const ListPickItemSchema = z.object({
  itemId: z.string(),
  createdAt: z.number(),
});

export const ListsPageSchema = z.object({
  results: z.array(FavoriteListSchema),
  total: z.number(),
  page: z.number(),
  pages: z.number(),
  limit: z.number(),
});

export const ListsEnvelopeSchema = z.object({
  ok: z.boolean(),
  data: ListsPageSchema,
});

export const NameOnlySchema = z.object({
  name: z.string(),
});

export const ParcelEstateSchema = z.object({
  name: z.string(),
  tokenId: z.string(),
});

export const ParcelDataSchema = z.object({
  x: z.string(),
  y: z.string(),
  description: z.string().nullable(),
  estate: ParcelEstateSchema.nullable(),
});

export const WearableDataSchema = z.object({
  bodyShapes: z.array(z.string()),
  category: z.string(),
  description: z.string(),
  rarity: z.string(),
  isSmart: z.boolean(),
});

export const NftDataSchema = z.union([z.object({
  wearable: WearableDataSchema,
}), z.object({
  emote: EmoteDataSchema,
}), z.object({
  parcel: ParcelDataSchema,
}), z.object({
  estate: EstateDataSchema,
}), z.object({
  ens: EnsDataSchema,
})]);

export const NftSchema = z.object({
  activeOrderId: z.string().nullable(),
  category: z.string(),
  chainId: z.number(),
  contractAddress: z.string(),
  createdAt: z.number(),
  data: NftDataSchema,
  id: z.string(),
  image: z.string(),
  issuedId: z.string().nullable(),
  itemId: z.string().nullable(),
  name: z.string(),
  network: NetworkSchema,
  openRentalId: z.string().nullable(),
  owner: z.string(),
  tokenId: z.string(),
  soldAt: z.number(),
  updatedAt: z.number(),
  url: z.string(),
  urn: z.string().optional(),
});

export const OrderSchema = z.object({
  id: z.string(),
  marketplaceAddress: z.string(),
  contractAddress: z.string(),
  tokenId: z.string().nullable(),
  owner: z.string(),
  buyer: z.string().nullable(),
  price: z.string(),
  status: z.string(),
  expiresAt: z.number(),
  createdAt: z.number(),
  updatedAt: z.number(),
  network: NetworkSchema,
  chainId: z.number(),
  issuedId: z.string().nullable(),
  tradeId: z.string().nullable(),
});

export const RentalListingPeriodSchema = z.object({
  minDays: z.number(),
  maxDays: z.number(),
  pricePerDay: z.string(),
});

export const RentalListingSchema = z.object({
  id: z.string(),
  nftId: z.string(),
  category: z.string(),
  searchText: z.string(),
  network: z.string(),
  chainId: z.number(),
  expiration: z.number(),
  signature: z.string(),
  nonces: z.array(z.string()),
  tokenId: z.string(),
  contractAddress: z.string(),
  rentalContractAddress: z.string(),
  lessor: z.string().nullable(),
  tenant: z.string().nullable(),
  status: z.string(),
  createdAt: z.number(),
  updatedAt: z.number(),
  startedAt: z.number().nullable(),
  periods: z.array(RentalListingPeriodSchema),
  target: z.string(),
  rentedDays: z.number().nullable(),
});

export const NftResultSchema = z.object({
  nft: NftSchema,
  order: OrderSchema.nullable(),
  rental: RentalListingSchema.nullable(),
});

export const NftsResponseBodySchema = z.object({
  data: z.array(NftResultSchema),
  total: z.number(),
});

export const OwnerSchema = z.object({
  issuedId: z.string(),
  ownerId: z.string(),
  tokenId: z.string(),
});

export const PickUnpickResultSchema = z.object({
  pickedByUser: z.boolean(),
});

export const PickUnpickEnvelopeSchema = z.object({
  ok: z.boolean(),
  data: PickUnpickResultSchema,
});

export const PicksPageSchema = z.object({
  results: z.array(ListPickItemSchema),
  total: z.number(),
  page: z.number(),
  pages: z.number(),
  limit: z.number(),
});

export const PicksEnvelopeSchema = z.object({
  ok: z.boolean(),
  data: PicksPageSchema,
});

export const PicksStatsEnvelopeSchema = z.object({
  ok: z.boolean(),
  data: z.array(PickStatsSchema),
});

export const ProfileNameSchema = z.object({
  name: z.string(),
  contractAddress: z.string(),
  tokenId: z.string(),
  price: z.string().optional(),
});

export const ProfileWearableSchema = z.object({
  urn: z.string(),
  id: z.string(),
  tokenId: z.string(),
  category: z.string(),
  transferredAt: z.string().nullable(),
  name: z.string(),
  rarity: z.string(),
  price: z.string().optional(),
  status: z.string().optional(),
  unlockAt: z.number().optional(),
});

export const ShopCouponSchema = z.object({
  proof: z.array(z.string()),
  id: z.string(),
  signer: z.string(),
  couponManager: z.string(),
  couponAddress: z.string(),
  checks: z.object({
    uses: z.number(),
    expiration: z.number(),
    effective: z.number(),
    salt: z.string(),
    contractSignatureIndex: z.number(),
    signerSignatureIndex: z.number(),
    allowedRoot: z.string(),
    externalChecks: z.array(z.object({
      contractAddress: z.string(),
      selector: z.string(),
      value: z.string(),
      required: z.boolean(),
    })),
  }),
  discountType: z.number(),
  discount: z.number(),
  root: z.string(),
  collections: z.array(z.string()),
  signature: z.string(),
  used: z.number(),
});

export const UnifiedItemSchema = z.object({
  source: z.enum(["native", "legacy"]),
  acquisition: z.enum(["trade", "store"]),
  tradeId: z.string().nullable(),
  listingType: z.string(),
  contractAddress: z.string(),
  itemId: z.string().nullable(),
  tokenId: z.string().nullable(),
  name: z.string(),
  thumbnail: z.string(),
  rarity: z.string(),
  category: z.string(),
  wearableCategory: z.string().nullable(),
  emoteLoop: z.boolean().nullable(),
  gender: z.enum(["male", "female", "unisex"]).nullable(),
  creator: z.string(),
  seller: z.string().nullable(),
  issuedId: z.string().nullable(),
  priceCredits: z.number(),
  manaWei: z.string().nullable(),
  listingCount: z.number(),
  available: z.number(),
  network: NetworkSchema,
  chainId: z.number(),
  createdAt: z.number(),
  compareAtCredits: z.number().nullable(),
  saleEndsAt: z.number().nullable(),
  saleUnitsLeft: z.number().nullable(),
  coupon: ShopCouponSchema.nullable(),
});

export const RelatedResponseBodySchema = z.object({
  data: z.array(UnifiedItemSchema),
});

export const SaleSchema = z.object({
  id: z.string(),
  itemId: z.string().nullable(),
  contractAddress: z.string(),
  buyer: z.string(),
  chainId: z.number(),
  network: NetworkSchema,
  price: z.string(),
  seller: z.string(),
  timestamp: z.number(),
  tokenId: z.string().nullable(),
  txHash: z.string(),
  type: z.string(),
});

export const SalesSummaryCollectionSchema = z.object({
  contractAddress: z.string(),
  sold: z.number(),
  earnedWei: z.string(),
});

export const SalesSummaryItemSchema = z.object({
  contractAddress: z.string(),
  itemId: z.string(),
  soldLifetime: z.number(),
});

export const SalesSummaryRoyaltiesSchema = z.object({
  resales: z.number(),
  volumeWei: z.string(),
});

export const SalesSummarySchema = z.object({
  total: z.number(),
  mints: z.number(),
  resales: z.number(),
  earnedWei: z.string(),
  byCollection: z.array(SalesSummaryCollectionSchema),
  byItem: z.array(SalesSummaryItemSchema),
  royalties: SalesSummaryRoyaltiesSchema,
});

export const ShopCouponRowSchema = z.object({
  id: z.string(),
  signer: z.string(),
  couponManager: z.string(),
  couponAddress: z.string(),
  checks: z.object({
    uses: z.number(),
    expiration: z.number(),
    effective: z.number(),
    salt: z.string(),
    contractSignatureIndex: z.number(),
    signerSignatureIndex: z.number(),
    allowedRoot: z.string(),
    externalChecks: z.array(z.object({
      contractAddress: z.string(),
      selector: z.string(),
      value: z.string(),
      required: z.boolean(),
    })),
  }),
  discountType: z.number(),
  discount: z.number(),
  root: z.string(),
  collections: z.array(z.string()),
  signature: z.string(),
  used: z.number(),
});

export const ShopListingSchema = z.object({
  tradeId: z.string(),
  listingType: z.string(),
  contractAddress: z.string(),
  itemId: z.string().nullable(),
  tokenId: z.string().nullable(),
  name: z.string(),
  thumbnail: z.string(),
  rarity: z.string(),
  category: z.string(),
  wearableCategory: z.string().nullable(),
  gender: z.enum(["male", "female", "unisex"]).nullable(),
  creator: z.string(),
  seller: z.string().nullable(),
  issuedId: z.string().nullable(),
  priceCredits: z.number(),
  available: z.number(),
  network: NetworkSchema,
  chainId: z.number(),
  createdAt: z.number(),
  compareAtCredits: z.number().nullable(),
  saleEndsAt: z.number().nullable(),
  saleUnitsLeft: z.number().nullable(),
  coupon: ShopCouponSchema.nullable(),
});

export const ShopSaleFieldsSchema = z.object({
  compareAtCredits: z.number().nullable(),
  saleEndsAt: z.number().nullable(),
  saleUnitsLeft: z.number().nullable(),
  coupon: ShopCouponSchema.nullable(),
});

export const SuggestedCollectionSchema = z.object({
  contractAddress: z.string(),
  name: z.string(),
  creator: z.string(),
  creatorName: z.string().nullable(),
  items: z.number(),
  sales: z.number(),
});

export const SuggestionReasonKindSchema = z.enum(["co_owned", "creator_affinity", "favorite_similar", "equipped_similar", "seed_similar", "trending"]);

export const SuggestionReasonSchema = z.object({
  kind: SuggestionReasonKindSchema,
  itemId: z.string().optional(),
  creator: z.string().optional(),
});

export const SuggestedItemSchema = z.object({
  reason: SuggestionReasonSchema,
  score: z.number(),
  source: z.enum(["native", "legacy"]),
  acquisition: z.enum(["trade", "store"]),
  tradeId: z.string().nullable(),
  listingType: z.string(),
  contractAddress: z.string(),
  itemId: z.string().nullable(),
  tokenId: z.string().nullable(),
  name: z.string(),
  thumbnail: z.string(),
  rarity: z.string(),
  category: z.string(),
  wearableCategory: z.string().nullable(),
  emoteLoop: z.boolean().nullable(),
  gender: z.enum(["male", "female", "unisex"]).nullable(),
  creator: z.string(),
  seller: z.string().nullable(),
  issuedId: z.string().nullable(),
  priceCredits: z.number(),
  manaWei: z.string().nullable(),
  listingCount: z.number(),
  available: z.number(),
  network: NetworkSchema,
  chainId: z.number(),
  createdAt: z.number(),
  compareAtCredits: z.number().nullable(),
  saleEndsAt: z.number().nullable(),
  saleUnitsLeft: z.number().nullable(),
  coupon: ShopCouponSchema.nullable(),
});

export const SuggestionsResponseSchema = z.object({
  data: z.array(SuggestedItemSchema),
  personalized: z.boolean(),
  algorithm: z.string(),
});

export const TopCreatorSchema = z.object({
  id: z.string(),
  volumeWei: z.string(),
  sales: z.number(),
  totalSales: z.number(),
  collections: z.number(),
  items: z.number(),
});

export const TopCreatorsResponseBodySchema = z.object({
  data: z.array(TopCreatorSchema),
});

export const TrendingItemSchema = z.object({
  trendingSales: z.number(),
  source: z.enum(["native", "legacy"]),
  acquisition: z.enum(["trade", "store"]),
  tradeId: z.string().nullable(),
  listingType: z.string(),
  contractAddress: z.string(),
  itemId: z.string().nullable(),
  tokenId: z.string().nullable(),
  name: z.string(),
  thumbnail: z.string(),
  rarity: z.string(),
  category: z.string(),
  wearableCategory: z.string().nullable(),
  emoteLoop: z.boolean().nullable(),
  gender: z.enum(["male", "female", "unisex"]).nullable(),
  creator: z.string(),
  seller: z.string().nullable(),
  issuedId: z.string().nullable(),
  priceCredits: z.number(),
  manaWei: z.string().nullable(),
  listingCount: z.number(),
  available: z.number(),
  network: NetworkSchema,
  chainId: z.number(),
  createdAt: z.number(),
  compareAtCredits: z.number().nullable(),
  saleEndsAt: z.number().nullable(),
  saleUnitsLeft: z.number().nullable(),
  coupon: ShopCouponSchema.nullable(),
});

export const TrendingResponseBodySchema = z.object({
  data: z.array(TrendingItemSchema),
});

export const UnifiedListingSchema = z.object({
  source: z.enum(["native", "legacy"]),
  acquisition: z.enum(["trade", "store"]),
  tradeId: z.string().nullable(),
  listingType: z.string(),
  contractAddress: z.string(),
  itemId: z.string().nullable(),
  tokenId: z.string().nullable(),
  name: z.string(),
  thumbnail: z.string(),
  rarity: z.string(),
  category: z.string(),
  wearableCategory: z.string().nullable(),
  emoteLoop: z.boolean().nullable(),
  gender: z.enum(["male", "female", "unisex"]).nullable(),
  creator: z.string(),
  seller: z.string().nullable(),
  issuedId: z.string().nullable(),
  priceCredits: z.number(),
  manaWei: z.string().nullable(),
  available: z.number(),
  network: NetworkSchema,
  chainId: z.number(),
  createdAt: z.number(),
  compareAtCredits: z.number().nullable(),
  saleEndsAt: z.number().nullable(),
  saleUnitsLeft: z.number().nullable(),
  coupon: ShopCouponSchema.nullable(),
});

export const UrnTokenSchema = z.object({
  urn: z.string(),
  tokenId: z.string(),
});

type AssignableTo<Sub, Sup> = Sub extends Sup ? true : false;
type Mutual<A, B> = AssignableTo<A, B> extends true ? AssignableTo<B, A> : false;
type Assert<T extends true> = T;

export type _AssertAccount = Assert<Mutual<Account, z.infer<typeof AccountSchema>>>;
export type _AssertAssetsHttpResponse = Assert<Mutual<AssetsHttpResponse<unknown>, z.infer<ReturnType<typeof AssetsHttpResponseSchema<z.ZodUnknown>>>>>;
export type _AssertBid = Assert<Mutual<Bid, z.infer<typeof BidSchema>>>;
export type _AssertBidsEnvelope = Assert<Mutual<BidsEnvelope, z.infer<typeof BidsEnvelopeSchema>>>;
export type _AssertBidsPage = Assert<Mutual<BidsPage, z.infer<typeof BidsPageSchema>>>;
export type _AssertCatalogItem = Assert<Mutual<CatalogItem, z.infer<typeof CatalogItemSchema>>>;
export type _AssertCollection = Assert<Mutual<Collection, z.infer<typeof CollectionSchema>>>;
export type _AssertContract = Assert<Mutual<Contract, z.infer<typeof ContractSchema>>>;
export type _AssertCreatorSearchHit = Assert<Mutual<CreatorSearchHit, z.infer<typeof CreatorSearchHitSchema>>>;
export type _AssertCreditCatalogItem = Assert<Mutual<CreditCatalogItem, z.infer<typeof CreditCatalogItemSchema>>>;
export type _AssertDataOnly = Assert<Mutual<DataOnly<unknown>, z.infer<ReturnType<typeof DataOnlySchema<z.ZodUnknown>>>>>;
export type _AssertDataTotal = Assert<Mutual<DataTotal<unknown>, z.infer<ReturnType<typeof DataTotalSchema<z.ZodUnknown>>>>>;
export type _AssertDataTotalString = Assert<Mutual<DataTotalString<unknown>, z.infer<ReturnType<typeof DataTotalStringSchema<z.ZodUnknown>>>>>;
export type _AssertEmoteData = Assert<Mutual<EmoteData, z.infer<typeof EmoteDataSchema>>>;
export type _AssertEnsData = Assert<Mutual<EnsData, z.infer<typeof EnsDataSchema>>>;
export type _AssertEstateData = Assert<Mutual<EstateData, z.infer<typeof EstateDataSchema>>>;
export type _AssertEstateParcel = Assert<Mutual<EstateParcel, z.infer<typeof EstateParcelSchema>>>;
export type _AssertFavoriteList = Assert<Mutual<FavoriteList, z.infer<typeof FavoriteListSchema>>>;
export type _AssertGroupedWearable = Assert<Mutual<GroupedWearable, z.infer<typeof GroupedWearableSchema>>>;
export type _AssertImportableListing = Assert<Mutual<ImportableListing, z.infer<typeof ImportableListingSchema>>>;
export type _AssertImportableResponseBody = Assert<Mutual<ImportableResponseBody, z.infer<typeof ImportableResponseBodySchema>>>;
export type _AssertIndividualData = Assert<Mutual<IndividualData, z.infer<typeof IndividualDataSchema>>>;
export type _AssertItem = Assert<Mutual<Item, z.infer<typeof ItemSchema>>>;
export type _AssertLegacyListing = Assert<Mutual<LegacyListing, z.infer<typeof LegacyListingSchema>>>;
export type _AssertListPickItem = Assert<Mutual<ListPickItem, z.infer<typeof ListPickItemSchema>>>;
export type _AssertListsEnvelope = Assert<Mutual<ListsEnvelope, z.infer<typeof ListsEnvelopeSchema>>>;
export type _AssertListsPage = Assert<Mutual<ListsPage, z.infer<typeof ListsPageSchema>>>;
export type _AssertNameOnly = Assert<Mutual<NameOnly, z.infer<typeof NameOnlySchema>>>;
export type _AssertNetwork = Assert<Mutual<Network, z.infer<typeof NetworkSchema>>>;
export type _AssertNft = Assert<Mutual<Nft, z.infer<typeof NftSchema>>>;
export type _AssertNftCategory = Assert<Mutual<NftCategory, z.infer<typeof NftCategorySchema>>>;
export type _AssertNftData = Assert<Mutual<NftData, z.infer<typeof NftDataSchema>>>;
export type _AssertNftResult = Assert<Mutual<NftResult, z.infer<typeof NftResultSchema>>>;
export type _AssertNftsResponseBody = Assert<Mutual<NftsResponseBody, z.infer<typeof NftsResponseBodySchema>>>;
export type _AssertOrder = Assert<Mutual<Order, z.infer<typeof OrderSchema>>>;
export type _AssertOwner = Assert<Mutual<Owner, z.infer<typeof OwnerSchema>>>;
export type _AssertPaginatedAssetsBody = Assert<Mutual<PaginatedAssetsBody<unknown>, z.infer<ReturnType<typeof PaginatedAssetsBodySchema<z.ZodUnknown>>>>>;
export type _AssertParcelData = Assert<Mutual<ParcelData, z.infer<typeof ParcelDataSchema>>>;
export type _AssertParcelEstate = Assert<Mutual<ParcelEstate, z.infer<typeof ParcelEstateSchema>>>;
export type _AssertPicksCount = Assert<Mutual<PicksCount, z.infer<typeof PicksCountSchema>>>;
export type _AssertPicksEnvelope = Assert<Mutual<PicksEnvelope, z.infer<typeof PicksEnvelopeSchema>>>;
export type _AssertPicksPage = Assert<Mutual<PicksPage, z.infer<typeof PicksPageSchema>>>;
export type _AssertPicksStatsEnvelope = Assert<Mutual<PicksStatsEnvelope, z.infer<typeof PicksStatsEnvelopeSchema>>>;
export type _AssertPickStats = Assert<Mutual<PickStats, z.infer<typeof PickStatsSchema>>>;
export type _AssertPickUnpickEnvelope = Assert<Mutual<PickUnpickEnvelope, z.infer<typeof PickUnpickEnvelopeSchema>>>;
export type _AssertPickUnpickResult = Assert<Mutual<PickUnpickResult, z.infer<typeof PickUnpickResultSchema>>>;
export type _AssertProfileName = Assert<Mutual<ProfileName, z.infer<typeof ProfileNameSchema>>>;
export type _AssertProfileWearable = Assert<Mutual<ProfileWearable, z.infer<typeof ProfileWearableSchema>>>;
export type _AssertRelatedResponseBody = Assert<Mutual<RelatedResponseBody, z.infer<typeof RelatedResponseBodySchema>>>;
export type _AssertRentalListing = Assert<Mutual<RentalListing, z.infer<typeof RentalListingSchema>>>;
export type _AssertRentalListingPeriod = Assert<Mutual<RentalListingPeriod, z.infer<typeof RentalListingPeriodSchema>>>;
export type _AssertSale = Assert<Mutual<Sale, z.infer<typeof SaleSchema>>>;
export type _AssertSalesSummary = Assert<Mutual<SalesSummary, z.infer<typeof SalesSummarySchema>>>;
export type _AssertSalesSummaryCollection = Assert<Mutual<SalesSummaryCollection, z.infer<typeof SalesSummaryCollectionSchema>>>;
export type _AssertSalesSummaryItem = Assert<Mutual<SalesSummaryItem, z.infer<typeof SalesSummaryItemSchema>>>;
export type _AssertSalesSummaryRoyalties = Assert<Mutual<SalesSummaryRoyalties, z.infer<typeof SalesSummaryRoyaltiesSchema>>>;
export type _AssertShopCoupon = Assert<Mutual<ShopCoupon, z.infer<typeof ShopCouponSchema>>>;
export type _AssertShopCouponRow = Assert<Mutual<ShopCouponRow, z.infer<typeof ShopCouponRowSchema>>>;
export type _AssertShopListing = Assert<Mutual<ShopListing, z.infer<typeof ShopListingSchema>>>;
export type _AssertShopSaleFields = Assert<Mutual<ShopSaleFields, z.infer<typeof ShopSaleFieldsSchema>>>;
export type _AssertSuggestedCollection = Assert<Mutual<SuggestedCollection, z.infer<typeof SuggestedCollectionSchema>>>;
export type _AssertSuggestedItem = Assert<Mutual<SuggestedItem, z.infer<typeof SuggestedItemSchema>>>;
export type _AssertSuggestionReason = Assert<Mutual<SuggestionReason, z.infer<typeof SuggestionReasonSchema>>>;
export type _AssertSuggestionReasonKind = Assert<Mutual<SuggestionReasonKind, z.infer<typeof SuggestionReasonKindSchema>>>;
export type _AssertSuggestionsResponse = Assert<Mutual<SuggestionsResponse, z.infer<typeof SuggestionsResponseSchema>>>;
export type _AssertTopCreator = Assert<Mutual<TopCreator, z.infer<typeof TopCreatorSchema>>>;
export type _AssertTopCreatorsResponseBody = Assert<Mutual<TopCreatorsResponseBody, z.infer<typeof TopCreatorsResponseBodySchema>>>;
export type _AssertTrendingItem = Assert<Mutual<TrendingItem, z.infer<typeof TrendingItemSchema>>>;
export type _AssertTrendingResponseBody = Assert<Mutual<TrendingResponseBody, z.infer<typeof TrendingResponseBodySchema>>>;
export type _AssertUnifiedItem = Assert<Mutual<UnifiedItem, z.infer<typeof UnifiedItemSchema>>>;
export type _AssertUnifiedListing = Assert<Mutual<UnifiedListing, z.infer<typeof UnifiedListingSchema>>>;
export type _AssertUrnToken = Assert<Mutual<UrnToken, z.infer<typeof UrnTokenSchema>>>;
export type _AssertWearableData = Assert<Mutual<WearableData, z.infer<typeof WearableDataSchema>>>;
