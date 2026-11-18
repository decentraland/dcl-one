// Item domain model, ported from the legacy builder (src/modules/item).
import {
  CollectionDisplayStatus,
  canManageCollectionItems,
  canSellCollectionItems,
  hasBeenApproved,
  isCollectionLocked,
  type Collection
} from './collections'
import { type Mappings, type SpringBonesData } from '@dcl/schemas'
import { type ItemListing } from './listings'
import { getRarityMaxSupply } from './rarities'

export enum ItemType {
  WEARABLE = 'wearable',
  EMOTE = 'emote'
}

export enum BodyShapeType {
  BOTH = 'both',
  MALE = 'male',
  FEMALE = 'female'
}

export const BODY_SHAPE_MALE = 'urn:decentraland:off-chain:base-avatars:BaseMale'
export const BODY_SHAPE_FEMALE = 'urn:decentraland:off-chain:base-avatars:BaseFemale'

/** A smart wearable's preview video; stored with the item but never deployed to the Catalyst. */
export const VIDEO_PATH = 'video.mp4'
/** Catalyst image: the thumbnail over its rarity gradient, referenced by every deployed entity. */
export const IMAGE_PATH = 'image.png'

export type ItemRepresentation = {
  bodyShapes: string[]
  mainFile: string
  contents: string[]
  overrideHides?: string[]
  overrideReplaces?: string[]
}

// Wearables and emotes share this shape on the wire; emote-only fields are optional extras.
export type ItemData = {
  category?: string
  representations: ItemRepresentation[]
  hides?: string[]
  replaces?: string[]
  removesDefaultHiding?: string[]
  tags?: string[]
  blockVrmExport?: boolean
  outlineCompatible?: boolean
  /** Smart wearables only: the scene permissions declared in its scene.json. */
  requiredPermissions?: string[]
  /** Spring bone physics per representation GLB, keyed by content hash. */
  springBones?: SpringBonesData | null
  loop?: boolean
  /** Social emotes only: the clips played when the emote starts. Not editable here. */
  startAnimation?: unknown
  outcomes?: unknown[]
  randomizeOutcomes?: boolean
}

// Wearable render stats or emote animation stats, as reported to builder-server.
export type ItemMetrics = {
  triangles?: number
  materials?: number
  textures?: number
  meshes?: number
  bodies?: number
  entities?: number
  sequences?: number
  duration?: number
  frames?: number
  fps?: number
  props?: number
  additionalArmatures?: number
}

/**
 * Metrics with no geometry: what a read of the preview's model-less boot scene reports (Babylon keeps one empty
 * mesh there, so meshes alone can't tell it from a real model).
 */
export function hasEmptyModelMetrics(metrics: ItemMetrics): boolean {
  return !metrics.triangles && !metrics.materials && !metrics.textures
}

/** Texture wearables have no mesh to measure: fixed metrics, as the legacy builder stored them. */
export const IMAGE_WEARABLE_METRICS: ItemMetrics = {
  triangles: 100,
  materials: 1,
  textures: 1,
  meshes: 1,
  bodies: 1,
  entities: 1
}

export type Item = {
  id: string
  name: string
  description: string
  thumbnail: string
  video?: string
  urn?: string
  owner: string
  collectionId?: string
  price?: string
  beneficiary?: string
  rarity?: string
  /** In-world effect text (≤ 64 chars), wearables only. */
  utility?: string
  totalSupply?: number
  /** On-chain item id (`blockchain_item_id`), assigned once the collection is published. */
  tokenId?: string
  /** Content hash of the builder copy, computed by builder-server once the item is published. */
  currentContentHash?: string
  /** Content hash the collection contract holds for the item. */
  blockchainContentHash?: string
  isPublished: boolean
  isApproved: boolean
  inCatalyst: boolean
  /** Linked items only: which tokens of the linked contract unlock the item (`@dcl/schemas` Mappings). */
  mappings?: Mappings | null
  /** Linked items only: false when the item was published before mappings existed and its mapping awaits review. */
  isMappingComplete?: boolean
  /** Linked items only: the content hash of the entity the Catalyst holds, once approved. */
  catalystContentHash?: string
  type: ItemType
  data: ItemData
  metrics?: ItemMetrics
  contents: Record<string, string>
  createdAt: number
  updatedAt: number
}

/** All distinct body shapes across an item's representations. */
function getBodyShapes(item: Item): string[] {
  const bodyShapes = new Set<string>()
  for (const representation of item.data.representations) {
    for (const bodyShape of representation.bodyShapes) bodyShapes.add(bodyShape)
  }
  return Array.from(bodyShapes)
}

/** The Body Type column value: male / female / both (unisex), or null for no representations. */
export function getItemBodyShapeType(item: Item): BodyShapeType | null {
  const bodyShapes = getBodyShapes(item)
  const hasMale = bodyShapes.includes(BODY_SHAPE_MALE)
  const hasFemale = bodyShapes.includes(BODY_SHAPE_FEMALE)
  if (hasMale && hasFemale) return BodyShapeType.BOTH
  if (hasMale) return BodyShapeType.MALE
  if (hasFemale) return BodyShapeType.FEMALE
  return null
}

/** The body shape an item is still missing (male/female), or null when it's already unisex. */
export function getMissingBodyShapeType(item: Item): BodyShapeType.MALE | BodyShapeType.FEMALE | null {
  const bodyShapeType = getItemBodyShapeType(item)
  if (bodyShapeType === BodyShapeType.MALE) return BodyShapeType.FEMALE
  if (bodyShapeType === BodyShapeType.FEMALE) return BodyShapeType.MALE
  return null
}

/** Same published / under-review / draft derivation as collections, at the item level. */
export type ItemSales = {
  minted: number
  maxSupply: number
}

/** Minted vs. maximum supply of a published item; undefined without a rarity. */
export function getItemSales(item: Item): ItemSales | undefined {
  const maxSupply = getRarityMaxSupply(item.rarity)
  if (maxSupply === undefined) return undefined
  return { minted: Math.min(item.totalSupply ?? 0, maxSupply), maxSupply }
}

export function getItemDisplayStatus(item: Item): CollectionDisplayStatus {
  if (!item.isPublished) return CollectionDisplayStatus.DRAFT
  return item.isApproved ? CollectionDisplayStatus.PUBLISHED : CollectionDisplayStatus.UNDER_REVIEW
}

// On-chain metadata string, byte-identical to the legacy builder's getMetadata (modules/item/utils.ts):
// version:type:name:description:category:bodyShapes, plus loop/props/outcome suffixes for emotes.
// It feeds the collection initialize calldata, which the server hashes into the contract address.
export function getItemMetadata(item: Item): string {
  const category = item.data.category
  if (!category) throw new Error(`Item "${item.id}" has no category`)
  // "BaseMale" / "BaseFemale" — the URN suffix.
  const bodyShapeTypes = getBodyShapes(item)
    .map(urn => urn.split(':').pop())
    .join(',')
  const base = `1:${getItemMetadataType(item)}:${item.name}:${item.description}:${category}:${bodyShapeTypes}`
  if (item.type !== ItemType.EMOTE) return base
  const additionalProperties = getEmoteAdditionalProperties(item)
  const outcomeType = getEmoteOutcomeType(item)
  return `${base}:${item.data.loop ? 1 : 0}${additionalProperties ? `:${additionalProperties}` : ''}${
    outcomeType ? `:${outcomeType}` : ''
  }`
}

/**
 * Contents shipping scene code (a `.js` file) make a smart wearable. The same heuristic as the
 * legacy builder and builder-server; older smart wearables carry no scene.json in their contents.
 */
export function isSceneCodeFile(path: string): boolean {
  return path.toLowerCase().endsWith('.js')
}

export function hasSceneCode(contents: Record<string, unknown>): boolean {
  return Object.keys(contents).some(isSceneCodeFile)
}

export function isSmartWearable(item: Item): boolean {
  return item.type === ItemType.WEARABLE && hasSceneCode(item.contents)
}

/**
 * A multi-armature "social" emote (two avatars and/or props). Replacing such an emote's file is blocked.
 */
export function isSocialEmote(item: Item): boolean {
  return item.type === ItemType.EMOTE && (!!item.metrics?.additionalArmatures || item.data.startAnimation !== undefined)
}

/** A smart wearable can't be published until its preview video has been uploaded (legacy isComplete). */
export function isMissingSmartWearableVideo(item: Item): boolean {
  return isSmartWearable(item) && !(VIDEO_PATH in item.contents)
}

function getItemMetadataType(item: Item): 'w' | 'sw' | 'e' {
  if (item.type === ItemType.EMOTE) return 'e'
  return isSmartWearable(item) ? 'sw' : 'w'
}

function getEmoteAdditionalProperties(item: Item): string {
  let properties = ''
  if (Object.keys(item.contents).some(content => content.includes('.mp3'))) properties += 's'
  if ((item.metrics?.props ?? 0) > 0) properties += 'g'
  return properties
}

function getEmoteOutcomeType(item: Item): string {
  const { outcomes, randomizeOutcomes } = item.data
  if (!outcomes || outcomes.length === 0) return ''
  if (outcomes.length === 1) return 'so'
  return randomizeOutcomes ? 'ro' : 'mo'
}

/**
 * Whoever manages the collection may edit, move, reset or delete its items. Having created the item grants
 * nothing on its own: builder-server rejects edits from a creator who no longer holds a role on the collection.
 */
export function canManageItem(collection: Collection, _item: Item, address: string | undefined): boolean {
  return canManageCollectionItems(collection, address)
}

/** Name and thumbnail stay editable after publishing (edits go through curation), but not during the publish lock. */
export function canEditItemDetails(collection: Collection, item: Item, address: string | undefined): boolean {
  return canManageItem(collection, item, address) && !isCollectionLocked(collection)
}

/** Only the collection owner may delete an item, and only while the collection is a draft outside the publish lock. */
export function canDeleteItem(collection: Collection, address: string | undefined): boolean {
  if (!address || collection.isPublished || isCollectionLocked(collection)) return false
  return collection.owner.toLowerCase() === address.toLowerCase()
}

/**
 * Only an off-chain order can be re-priced, by whoever may sell (the owner), and only while some
 * supply is left to sell. A legacy CollectionStore price has no `tradeId` and can only be removed.
 */
export function canEditItemPrice(
  collection: Collection,
  item: Item,
  listing: ItemListing | null | undefined,
  address: string | undefined
): listing is ItemListing & { tradeId: string } {
  if (!listing?.tradeId || !hasBeenApproved(collection)) return false
  if (!canSellCollectionItems(collection, address)) return false
  const sales = getItemSales(item)
  return !(sales && sales.minted >= sales.maxSupply)
}

/**
 * `item.video` is the hash curation last approved. A save replaces it directly until the item is approved; after
 * that only `contents['video.mp4']` moves, so the item reads as changed and goes through review (builder-server
 * copies the hash over on approval).
 */
export function tracksVideoOnSave(item: Item): boolean {
  return !item.isPublished || !item.isApproved
}
