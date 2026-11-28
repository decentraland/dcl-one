// GENERATED from library/catalyst-api/builder by sites/scripts/gen-zod-schemas.mts. Do not edit.
import { z } from "zod";

import type { CollectionListingOut } from "@catalyst-api/builder/CollectionListingOut";
import type { CollectionListingRowOut } from "@catalyst-api/builder/CollectionListingRowOut";
import type { CollectionMetaOut } from "@catalyst-api/builder/CollectionMetaOut";
import type { CollectionOut } from "@catalyst-api/builder/CollectionOut";
import type { CollectionType } from "@catalyst-api/builder/CollectionType";
import type { FullItemOut } from "@catalyst-api/builder/FullItemOut";
import type { ItemCurationOut } from "@catalyst-api/builder/ItemCurationOut";
import type { ItemsOut } from "@catalyst-api/builder/ItemsOut";
import type { ItemsPageOut } from "@catalyst-api/builder/ItemsPageOut";
import type { NewsletterSubscribeOut } from "@catalyst-api/builder/NewsletterSubscribeOut";
import type { PublicationItem } from "@catalyst-api/builder/PublicationItem";
import type { PublicationPreparation } from "@catalyst-api/builder/PublicationPreparation";
import type { PublicationState } from "@catalyst-api/builder/PublicationState";

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

export const CollectionTypeSchema = z.literal("standard");

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

export const ItemCurationOutSchema = z.object({
  id: z.string(),
  item_id: z.string(),
  status: z.string(),
  created_at: z.string(),
  updated_at: z.string(),
  content_hash: z.string(),
  is_mapping_complete: z.boolean().nullable(),
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

export type _AssertCollectionListingOut = Assert<Mutual<CollectionListingOut, z.infer<typeof CollectionListingOutSchema>>>;
export type _AssertCollectionListingRowOut = Assert<Mutual<CollectionListingRowOut, z.infer<typeof CollectionListingRowOutSchema>>>;
export type _AssertCollectionMetaOut = Assert<Mutual<CollectionMetaOut, z.infer<typeof CollectionMetaOutSchema>>>;
export type _AssertCollectionOut = Assert<Mutual<CollectionOut, z.infer<typeof CollectionOutSchema>>>;
export type _AssertCollectionType = Assert<Mutual<CollectionType, z.infer<typeof CollectionTypeSchema>>>;
export type _AssertFullItemOut = Assert<Mutual<FullItemOut, z.infer<typeof FullItemOutSchema>>>;
export type _AssertItemCurationOut = Assert<Mutual<ItemCurationOut, z.infer<typeof ItemCurationOutSchema>>>;
export type _AssertItemsOut = Assert<Mutual<ItemsOut, z.infer<typeof ItemsOutSchema>>>;
export type _AssertItemsPageOut = Assert<Mutual<ItemsPageOut, z.infer<typeof ItemsPageOutSchema>>>;
export type _AssertNewsletterSubscribeOut = Assert<Mutual<NewsletterSubscribeOut, z.infer<typeof NewsletterSubscribeOutSchema>>>;
export type _AssertPublicationItem = Assert<Mutual<PublicationItem, z.infer<typeof PublicationItemSchema>>>;
export type _AssertPublicationPreparation = Assert<Mutual<PublicationPreparation, z.infer<typeof PublicationPreparationSchema>>>;
export type _AssertPublicationState = Assert<Mutual<PublicationState, z.infer<typeof PublicationStateSchema>>>;
