// GENERATED from library/catalyst-api/builder by sites/scripts/gen-zod-schemas.mts. Do not edit.
import { z } from "zod";

import type { BulkItemStatusPatchOut } from "@catalyst-api/builder/BulkItemStatusPatchOut";
import type { CollectionListingOut } from "@catalyst-api/builder/CollectionListingOut";
import type { CollectionListingRowOut } from "@catalyst-api/builder/CollectionListingRowOut";
import type { CollectionMetaOut } from "@catalyst-api/builder/CollectionMetaOut";
import type { CollectionOut } from "@catalyst-api/builder/CollectionOut";
import type { CollectionStatus } from "@catalyst-api/builder/CollectionStatus";
import type { CollectionType } from "@catalyst-api/builder/CollectionType";
import type { CommitteeMemberOut } from "@catalyst-api/builder/CommitteeMemberOut";
import type { CurationCollectionsOut } from "@catalyst-api/builder/CurationCollectionsOut";
import type { CurationStatus } from "@catalyst-api/builder/CurationStatus";
import type { FullItemOut } from "@catalyst-api/builder/FullItemOut";
import type { ItemsOut } from "@catalyst-api/builder/ItemsOut";
import type { ItemsPageOut } from "@catalyst-api/builder/ItemsPageOut";
import type { ItemStatusPatchOut } from "@catalyst-api/builder/ItemStatusPatchOut";
import type { NewsletterSubscribeOut } from "@catalyst-api/builder/NewsletterSubscribeOut";
import type { PublicationItem } from "@catalyst-api/builder/PublicationItem";
import type { PublicationPreparation } from "@catalyst-api/builder/PublicationPreparation";
import type { PublicationState } from "@catalyst-api/builder/PublicationState";
import type { ReviewCurationOut } from "@catalyst-api/builder/ReviewCurationOut";
import type { ReviewRowOut } from "@catalyst-api/builder/ReviewRowOut";

export const BulkItemStatusPatchOutSchema = z.object({
  collection_id: z.string(),
  requested: z.number(),
  status: z.string(),
  updated: z.number(),
});

export const CollectionListingOutSchema = z.object({
  item_count: z.coerce.bigint(),
  id: z.string(),
  name: z.string(),
  eth_address: z.string(),
  contract_address: z.string().nullable(),
  urn: z.string().nullable(),
  third_party_id: z.string().nullable(),
  is_published: z.boolean(),
  is_approved: z.boolean(),
  created_at: z.string(),
  updated_at: z.string(),
});

export const CollectionListingRowOutSchema = z.object({
  curation_status: z.string().nullable(),
  last_activity_at: z.string(),
  id: z.string(),
  name: z.string(),
  eth_address: z.string(),
  salt: z.string().nullable(),
  contract_address: z.string().nullable(),
  is_published: z.boolean(),
  created_at: z.string(),
  updated_at: z.string(),
  managers: z.array(z.string()),
  minters: z.array(z.string()),
  is_approved: z.boolean(),
  reviewed_at: z.string().nullable(),
  forum_link: z.string().nullable(),
  lock: z.string().nullable(),
  forum_id: z.number().nullable(),
  linked_contract_address: z.string().nullable(),
  linked_contract_network: z.string().nullable(),
  is_mapping_complete: z.boolean(),
  item_count: z.string(),
  is_programmatic: z.boolean().optional(),
  urn: z.string(),
});

export const CollectionMetaOutSchema = z.object({
  id: z.string(),
  name: z.string(),
  eth_address: z.string(),
  contract_address: z.string().nullable(),
  urn: z.string().nullable(),
  third_party_id: z.string().nullable(),
  is_published: z.boolean(),
  is_approved: z.boolean(),
  created_at: z.string(),
  updated_at: z.string(),
});

export const CollectionOutSchema = z.object({
  id: z.string(),
  name: z.string(),
  eth_address: z.string(),
  salt: z.string().nullable(),
  contract_address: z.string().nullable(),
  is_published: z.boolean(),
  created_at: z.string(),
  updated_at: z.string(),
  managers: z.array(z.string()),
  minters: z.array(z.string()),
  is_approved: z.boolean(),
  reviewed_at: z.string().nullable(),
  forum_link: z.string().nullable(),
  lock: z.string().nullable(),
  forum_id: z.number().nullable(),
  linked_contract_address: z.string().nullable(),
  linked_contract_network: z.string().nullable(),
  is_mapping_complete: z.boolean(),
  item_count: z.string(),
  is_programmatic: z.boolean().optional(),
  urn: z.string(),
});

export const CollectionStatusSchema = z.enum(["synced", "under_review", "unsynced"]);

export const CollectionTypeSchema = z.literal("standard");

export const CommitteeMemberOutSchema = z.object({
  address: z.string(),
  name: z.string(),
});

export const CurationStatusSchema = z.enum(["approved", "rejected"]);

export const ReviewCurationOutSchema = z.object({
  assignee: z.string(),
  collection_id: z.string(),
  created_at: z.string().nullable(),
  id: z.string(),
  status: CurationStatusSchema,
  updated_at: z.string().nullable(),
});

export const ReviewRowOutSchema = z.object({
  created_at: z.string().nullable(),
  curation: ReviewCurationOutSchema.nullable(),
  has_reviews: z.boolean(),
  id: z.string(),
  is_approved: z.boolean(),
  is_programmatic: z.boolean(),
  item_count: z.number(),
  name: z.string(),
  owner: z.string().nullable(),
  reviewed_at: z.string().nullable(),
  status: CollectionStatusSchema,
  type: CollectionTypeSchema,
});

export const CurationCollectionsOutSchema = z.object({
  collections: z.array(ReviewRowOutSchema),
  committee: z.array(CommitteeMemberOutSchema),
});

export const FullItemOutSchema = z.object({
  id: z.string(),
  name: z.string(),
  description: z.string().nullable(),
  eth_address: z.string(),
  collection_id: z.string().nullable(),
  blockchain_item_id: z.string().nullable(),
  price: z.string().nullable(),
  beneficiary: z.string().nullable(),
  rarity: z.string().nullable(),
  type: z.string(),
  data: z.record(z.string(), z.unknown()),
  created_at: z.string(),
  updated_at: z.string(),
  metrics: z.record(z.string(), z.unknown()).nullable(),
  thumbnail: z.string().nullable(),
  contents: z.record(z.string(), z.string()),
  total_supply: z.number(),
  local_content_hash: z.string().nullable().optional(),
  video: z.string().nullable(),
  utility: z.string().nullable(),
  mappings: z.record(z.string(), z.unknown()).nullable(),
  urn: z.string().nullable(),
  in_catalyst: z.boolean(),
  is_approved: z.boolean(),
  is_published: z.boolean(),
  content_hash: z.string().nullable(),
  catalyst_content_hash: z.string().nullable(),
  isMappingComplete: z.boolean().optional(),
});

export const ItemStatusPatchOutSchema = z.object({
  collection_id: z.string(),
  id: z.string(),
  status: z.string(),
  updated: z.number(),
});

export const ItemsPageOutSchema = z.object({
  total: z.number().nullable(),
  limit: z.number(),
  pages: z.number().nullable(),
  page: z.number(),
  results: z.array(FullItemOutSchema),
});

export const ItemsOutSchema = z.union([z.array(FullItemOutSchema), ItemsPageOutSchema]);

export const NewsletterSubscribeOutSchema = z.object({
  ok: z.boolean(),
});

export const PublicationItemSchema = z.object({
  id: z.string(),
  rarity: z.string(),
  price: z.string(),
  beneficiary: z.string(),
  metadata: z.string(),
});

export const PublicationPreparationSchema = z.object({
  id: z.string(),
  revision: z.string(),
  chain_id: z.number(),
  manager: z.string(),
  factory: z.string(),
  forwarder: z.string(),
  salt: z.string(),
  name: z.string(),
  symbol: z.string(),
  base_uri: z.string(),
  creator: z.string(),
  items: z.array(PublicationItemSchema),
});

export const PublicationStateSchema = z.object({
  preparation: PublicationPreparationSchema,
  status: z.string(),
  tx_hash: z.string().nullable(),
  contract_address: z.string().nullable(),
});

type AssignableTo<Sub, Sup> = Sub extends Sup ? true : false;
type Mutual<A, B> = AssignableTo<A, B> extends true ? AssignableTo<B, A> : false;
type Assert<T extends true> = T;

export type _AssertBulkItemStatusPatchOut = Assert<Mutual<BulkItemStatusPatchOut, z.infer<typeof BulkItemStatusPatchOutSchema>>>;
export type _AssertCollectionListingOut = Assert<Mutual<CollectionListingOut, z.infer<typeof CollectionListingOutSchema>>>;
export type _AssertCollectionListingRowOut = Assert<Mutual<CollectionListingRowOut, z.infer<typeof CollectionListingRowOutSchema>>>;
export type _AssertCollectionMetaOut = Assert<Mutual<CollectionMetaOut, z.infer<typeof CollectionMetaOutSchema>>>;
export type _AssertCollectionOut = Assert<Mutual<CollectionOut, z.infer<typeof CollectionOutSchema>>>;
export type _AssertCollectionStatus = Assert<Mutual<CollectionStatus, z.infer<typeof CollectionStatusSchema>>>;
export type _AssertCollectionType = Assert<Mutual<CollectionType, z.infer<typeof CollectionTypeSchema>>>;
export type _AssertCommitteeMemberOut = Assert<Mutual<CommitteeMemberOut, z.infer<typeof CommitteeMemberOutSchema>>>;
export type _AssertCurationCollectionsOut = Assert<Mutual<CurationCollectionsOut, z.infer<typeof CurationCollectionsOutSchema>>>;
export type _AssertCurationStatus = Assert<Mutual<CurationStatus, z.infer<typeof CurationStatusSchema>>>;
export type _AssertFullItemOut = Assert<Mutual<FullItemOut, z.infer<typeof FullItemOutSchema>>>;
export type _AssertItemsOut = Assert<Mutual<ItemsOut, z.infer<typeof ItemsOutSchema>>>;
export type _AssertItemsPageOut = Assert<Mutual<ItemsPageOut, z.infer<typeof ItemsPageOutSchema>>>;
export type _AssertItemStatusPatchOut = Assert<Mutual<ItemStatusPatchOut, z.infer<typeof ItemStatusPatchOutSchema>>>;
export type _AssertNewsletterSubscribeOut = Assert<Mutual<NewsletterSubscribeOut, z.infer<typeof NewsletterSubscribeOutSchema>>>;
export type _AssertPublicationItem = Assert<Mutual<PublicationItem, z.infer<typeof PublicationItemSchema>>>;
export type _AssertPublicationPreparation = Assert<Mutual<PublicationPreparation, z.infer<typeof PublicationPreparationSchema>>>;
export type _AssertPublicationState = Assert<Mutual<PublicationState, z.infer<typeof PublicationStateSchema>>>;
export type _AssertReviewCurationOut = Assert<Mutual<ReviewCurationOut, z.infer<typeof ReviewCurationOutSchema>>>;
export type _AssertReviewRowOut = Assert<Mutual<ReviewRowOut, z.infer<typeof ReviewRowOutSchema>>>;
