// Catalyst entities of published items, ported from the legacy builder (modules/item/export.ts) so a curator
// here deploys byte-identical entities and computes the same content hashes the chain (or, for linked items,
// builder-server's Merkle tree) holds.
import {
  calculateMultipleHashesADR32,
  calculateMultipleHashesADR32LegacyQmHash,
  hashV1,
  keccak256Hash
} from '@dcl/hashing'
import { EntityType } from '@dcl/schemas'
import { fetchContent } from '~/lib/builder'
import { type Collection } from '~/lib/collections'
import { THUMBNAIL_PATH } from '~/lib/itemFiles'
import { IMAGE_PATH, ItemType, VIDEO_PATH, type Item, type ItemData, type ItemRepresentation } from '~/lib/items'
import { getDeployableFiles } from '~/lib/itemSync'
import { generateCatalystImage } from '~/lib/media'
const EMPTY_ITEM_METRICS = { triangles: 0, materials: 0, textures: 0, meshes: 0, bodies: 0, entities: 1 }

export type EntityContent = Record<string, string>

/** A generated `image.png` for an item saved without one (legacy generateCatalystImage). */
export type MissingImage = { hash: string; blob: Blob }

export type ImageDeps = {
  fetchContent: (hash: string) => Promise<Blob>
  renderCatalystImage: (thumbnail: Blob, rarity: string) => Promise<Blob>
}

/** How the approval flow renders the image.png an item saved without one deploys; measuring renders it the same way. */
export const catalystImageDeps: ImageDeps = {
  fetchContent: hash => fetchContent(hash),
  renderCatalystImage: (thumbnail, rarity) => generateCatalystImage(thumbnail, rarity)
}

export async function buildMissingImage(item: Item, deps: ImageDeps): Promise<MissingImage | null> {
  if (item.contents[IMAGE_PATH]) return null
  const thumbnail = await deps.fetchContent(item.contents[item.thumbnail])
  const blob = await deps.renderCatalystImage(thumbnail, item.rarity ?? '')
  return { hash: await hashV1(new Uint8Array(await blob.arrayBuffer())), blob }
}

/**
 * The files an item deploys: its stored contents minus the smart-wearable preview video, directory
 * entries and empty files, with `image.png` taken from `imageHash` when the item was saved without one.
 */
export function getEntityContent(item: Item, imageHash?: string): EntityContent {
  const contents: EntityContent = { ...item.contents }
  if (!contents[IMAGE_PATH] && imageHash) contents[IMAGE_PATH] = imageHash
  const deployable = getDeployableFiles(contents)
  deployable.delete(VIDEO_PATH)
  return Object.fromEntries([...deployable].map(path => [path, contents[path]]))
}

function stripRepresentations(representations: ItemRepresentation[], content: EntityContent) {
  return representations.map(representation => ({
    ...representation,
    contents: representation.contents.filter(path => path in content)
  }))
}

/** The wearable `data` block; its key order is part of the content hash and must never change. */
function buildWearableData(data: ItemData, content: EntityContent) {
  return {
    replaces: data.replaces,
    hides: data.hides,
    ...('removesDefaultHiding' in data ? { removesDefaultHiding: data.removesDefaultHiding } : {}),
    tags: data.tags,
    category: data.category,
    representations: stripRepresentations(data.representations, content),
    ...('blockVrmExport' in data ? { blockVrmExport: data.blockVrmExport } : {}),
    ...('outlineCompatible' in data ? { outlineCompatible: data.outlineCompatible } : {}),
    ...(data.springBones ? { springBones: data.springBones } : {})
  }
}

/** The entity metadata. Key order is part of the content hash and must never change. */
export function buildItemEntityMetadata(collection: Collection, item: Item, content: EntityContent) {
  if (!collection.contractAddress || !item.tokenId || !item.urn) {
    throw new Error(`Item ${item.id} is not published`)
  }
  const data = item.data
  const representations = stripRepresentations(data.representations, content)
  if (item.type === ItemType.EMOTE) {
    return {
      id: item.urn,
      name: item.name,
      description: item.description,
      collectionAddress: collection.contractAddress,
      rarity: item.rarity,
      i18n: [{ code: 'en', text: item.name }],
      emoteDataADR74: {
        category: data.category,
        representations,
        tags: data.tags,
        loop: data.loop,
        startAnimation: data.startAnimation,
        randomizeOutcomes: data.randomizeOutcomes,
        outcomes: data.outcomes
      },
      image: IMAGE_PATH,
      thumbnail: THUMBNAIL_PATH,
      metrics: EMPTY_ITEM_METRICS
    }
  }
  return {
    id: item.urn,
    name: item.name,
    description: item.description,
    collectionAddress: collection.contractAddress,
    rarity: item.rarity,
    i18n: [{ code: 'en', text: item.name }],
    data: buildWearableData(data, content),
    image: IMAGE_PATH,
    thumbnail: THUMBNAIL_PATH,
    metrics: item.metrics
  }
}

/** The leaf of the third party's Merkle tree for one item, as `@dcl/content-hash-tree` computes it. */
export type LinkedMerkleProof = { index: number; proof: string[]; entityHash: string }

/**
 * What a linked item hashes into its metadata and deploys: its stored contents, minus directory entries and empty
 * files. The Catalyst requires the uploaded files to match this exactly, so nothing is added (no generated image)
 * nor left out.
 */
export function getLinkedEntityContent(item: Item): EntityContent {
  return Object.fromEntries([...getDeployableFiles(item.contents)].map(path => [path, item.contents[path]]))
}

/**
 * The metadata builder-server hashes for a linked item (its `buildTPWearableEntityMetadata`); the legacy
 * builder's `buildTPItemEntityMetadata` has the same key order. Linked collections hold wearables only.
 */
function buildLinkedItemBaseMetadata(collection: Collection, item: Item) {
  if (!item.urn || !item.urn.startsWith(`${collection.urn}:`)) {
    throw new Error(`Item ${item.id} is not a published item of collection ${collection.id}`)
  }
  if (item.type !== ItemType.WEARABLE) throw new Error(`Item ${item.id} is not a wearable`)
  const content = getLinkedEntityContent(item)
  return {
    id: item.urn,
    name: item.name,
    description: item.description,
    i18n: [{ code: 'en', text: item.name }],
    data: buildWearableData(item.data, content),
    image: IMAGE_PATH,
    thumbnail: THUMBNAIL_PATH,
    metrics: item.metrics,
    content,
    ...(item.mappings ? { mappings: item.mappings } : {})
  }
}

/** The keccak hash builder-server stores as the item curation's `content_hash`: the tree leaf the proof is for. */
export function computeLinkedItemContentHash(collection: Collection, item: Item): string {
  const metadata = buildLinkedItemBaseMetadata(collection, item)
  return keccak256Hash(metadata, Object.keys(metadata))
}

/** The linked item's entity metadata: the hashed fields plus the Merkle proof the Catalyst checks them against. */
export function buildLinkedItemEntityMetadata(collection: Collection, item: Item, merkleProof: LinkedMerkleProof) {
  const base = buildLinkedItemBaseMetadata(collection, item)
  return {
    ...base,
    merkleProof: {
      index: merkleProof.index,
      proof: merkleProof.proof,
      hashingKeys: Object.keys(base),
      entityHash: merkleProof.entityHash
    }
  }
}

function toContentList(content: EntityContent) {
  return Object.entries(content).map(([file, hash]) => ({ file, hash }))
}

/** ADR-32 hash of the item as the collection contract stores it; `v0` is the older Qm format some items still carry. */
export async function computeItemContentHash(
  collection: Collection,
  item: Item,
  content: EntityContent,
  version: 'v0' | 'v1' = 'v1'
): Promise<string> {
  const metadata = buildItemEntityMetadata(collection, item, content)
  const hash = version === 'v0' ? calculateMultipleHashesADR32LegacyQmHash : calculateMultipleHashesADR32
  return (await hash(toContentList(content), metadata)).hash
}

export type BuiltEntity = { entityId: string; entityFile: Uint8Array<ArrayBuffer> }

/** The entity file and its id, exactly as dcl-catalyst-client's buildEntity produces them. */
async function encodeEntity(
  type: EntityType,
  content: EntityContent,
  metadata: { id: string },
  timestamp: number
): Promise<BuiltEntity> {
  const entity = { version: 'v3', type, pointers: [metadata.id], timestamp, content: toContentList(content), metadata }
  const entityFile = new TextEncoder().encode(JSON.stringify(entity))
  return { entityId: await hashV1(entityFile), entityFile }
}

export function buildItemEntity(
  collection: Collection,
  item: Item,
  content: EntityContent,
  timestamp = Date.now()
): Promise<BuiltEntity> {
  const type = item.type === ItemType.EMOTE ? EntityType.EMOTE : EntityType.WEARABLE
  return encodeEntity(type, content, buildItemEntityMetadata(collection, item, content), timestamp)
}

/** A linked item's entity: `content` is what it deploys, which must be exactly the content its metadata declares. */
export function buildLinkedItemEntity(
  collection: Collection,
  item: Item,
  content: EntityContent,
  merkleProof: LinkedMerkleProof,
  timestamp = Date.now()
): Promise<BuiltEntity> {
  const metadata = buildLinkedItemEntityMetadata(collection, item, merkleProof)
  return encodeEntity(EntityType.WEARABLE, content, metadata, timestamp)
}

export type AuthLink = { type: string; payload: string; signature?: string }

/** The multipart body of POST /content/entities; `files` holds only what the Catalyst doesn't store yet. */
export function buildDeploymentForm(entity: BuiltEntity, authChain: AuthLink[], files: Map<string, Blob>): FormData {
  const form = new FormData()
  form.append('entityId', entity.entityId)
  authChain.forEach((link, index) => {
    form.append(`authChain[${index}][type]`, link.type)
    form.append(`authChain[${index}][payload]`, link.payload)
    if (link.signature !== undefined) form.append(`authChain[${index}][signature]`, link.signature)
  })
  form.append(entity.entityId, new Blob([entity.entityFile]), entity.entityId)
  for (const [hash, blob] of files) form.append(hash, blob, hash)
  return form
}
