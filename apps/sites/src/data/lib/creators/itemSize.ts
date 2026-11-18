// An item's size as the catalyst counts it (ADR-246; core-libs content-validator `validations/size.ts`): the bytes of
// every distinct content hash the entity deploys, so every representation's models and textures plus thumbnail.png
// and image.png (the rarity-background image), with a file both body shapes share counted once. The smart-wearable
// preview video never deploys and never counts. The wearable-validator's file-size check (S-05) counts the same.
// Every place that shows or enforces an item's size goes through here.
import { getEntityContent, getLinkedEntityContent, type MissingImage } from './catalystEntity'
import { type BuiltItem } from './itemFactory'
import { ItemFileError, MAX_EMOTE_FILE_SIZE, MAX_SKIN_FILE_SIZE, MAX_WEARABLE_FILE_SIZE } from './itemFiles'
import { IMAGE_PATH, ItemType, VIDEO_PATH, type Item } from './items'
import { captureError } from './monitoring'
import { THIRD_PARTY_URN_TYPE } from './thirdParty'

const SKIN_CATEGORY = 'skin'
const MIB = 1024 * 1024

/** Stored files measured at once, app-wide: a collection's worth of sizes must not open dozens of requests together. */
export const MAX_CONCURRENT_MEASUREMENTS = 6
/** A stored file's size that hasn't arrived by then counts as unmeasurable (non-blocking), never as pending. */
export const MEASUREMENT_TIMEOUT_MS = 20_000

export type ItemSize = { bytes: number; limit: number }

/** Bytes of a stored file, by content hash. */
export type SizeOf = (hash: string) => Promise<number>

/** The image.png an item saved without one gets at deploy (lib/catalystEntity#buildMissingImage). */
export type ImageOf = (item: Item) => Promise<MissingImage | null>

/**
 * How stored files are measured: their bytes by hash, and the image.png a legacy item would get at deploy. `remember`
 * is told the bytes of every file measured from memory (an upload or a generated image), so storage need not be asked.
 */
export type MeasureDeps = { sizeOf: SizeOf; imageOf?: ImageOf; remember?: (hash: string, bytes: number) => void }

/** An item, with the files a save is about to upload (keyed by path) when there are any. */
export type MeasurableItem = Pick<BuiltItem, 'item'> & { blobs?: Record<string, Blob> }

/** The ceiling for an item's whole deployment: 3 MB for wearables and emotes, 9 MB for skins. */
export function getItemSizeLimit(type: ItemType | null | undefined, category?: string | null): number {
  if (type === ItemType.EMOTE) return MAX_EMOTE_FILE_SIZE
  return category === SKIN_CATEGORY ? MAX_SKIN_FILE_SIZE : MAX_WEARABLE_FILE_SIZE
}

/** The catalyst's sum: every distinct content hash once, however many paths point at it. */
export function sumUniqueContent(files: Iterable<{ hash: string; bytes: number }>): number {
  const byHash = new Map<string, number>()
  for (const { hash, bytes } of files) if (!byHash.has(hash)) byHash.set(hash, bytes)
  let total = 0
  for (const bytes of byHash.values()) total += bytes
  return total
}

/** A linked (third-party) item: its URN, like its collection's, is a `collections-thirdparty` one. */
function isLinkedItem(item: Pick<Item, 'urn'>): boolean {
  return item.urn?.split(':')[3] === THIRD_PARTY_URN_TYPE
}

/**
 * The bytes the item's entity deploys. Files about to be uploaded are measured from `blobs`, stored ones through
 * `sizeOf`; `imageOf` renders the image.png a standard item saved without one would get at deploy, so it counts
 * too. A linked item deploys its stored files exactly (getLinkedEntityContent), never a generated image.
 */
export async function measureItemBytes(
  { item, blobs = {} }: MeasurableItem,
  { sizeOf, imageOf, remember }: MeasureDeps
): Promise<number> {
  const linked = isLinkedItem(item)
  const image = !linked && !item.contents[IMAGE_PATH] && imageOf ? await imageOf(item) : null
  const content = linked ? getLinkedEntityContent(item) : getEntityContent(item, image?.hash)
  const known = new Map<string, number>()
  if (image) known.set(image.hash, image.blob.size)
  for (const [path, hash] of Object.entries(content)) {
    if (blobs[path]) known.set(hash, blobs[path].size)
  }
  for (const [hash, bytes] of known) remember?.(hash, bytes)
  const hashes = [...new Set(Object.values(content))]
  const files = await Promise.all(hashes.map(async hash => ({ hash, bytes: known.get(hash) ?? (await sizeOf(hash)) })))
  return sumUniqueContent(files)
}

/** The item's size against the limit for its type and category. */
export async function measureItem(built: MeasurableItem, deps: MeasureDeps): Promise<ItemSize> {
  const bytes = await measureItemBytes(built, deps)
  return { bytes, limit: getItemSizeLimit(built.item.type, built.item.data.category) }
}

export function isOverSizeLimit(size: ItemSize): boolean {
  return size.bytes > size.limit
}

const toMiB = (bytes: number) => Math.round((bytes / MIB) * 100) / 100

/**
 * Size and limit in MB with two decimals, rounded as the wearable-validator reports them ("2.95" of "3.00"). An item
 * over its limit never reads as at it: a few bytes over shows the next hundredth.
 */
export function formatItemSize({ bytes, limit }: ItemSize): { size: string; limit: string } {
  const cap = toMiB(limit)
  const shown = bytes > limit && toMiB(bytes) <= cap ? cap + 0.01 : toMiB(bytes)
  return { size: shown.toFixed(2), limit: cap.toFixed(2) }
}

/** The error a save refuses an item over its limit with: `add_items_modal.file_error.size_exceeded`. */
function itemTooBigError(size: ItemSize): ItemFileError {
  return new ItemFileError('size_exceeded', formatItemSize(size))
}

export function isItemTooBigError(error: unknown): error is ItemFileError {
  return error instanceof ItemFileError && error.messageKey === 'size_exceeded'
}

/** Measures the item and refuses it, with the creator-facing error, when it is over its limit. */
export async function assertItemSize(built: MeasurableItem, deps: MeasureDeps): Promise<ItemSize> {
  const size = await measureItem(built, deps)
  if (isOverSizeLimit(size)) throw itemTooBigError(size)
  return size
}

/**
 * Whether a save can change the item's size or its limit: it uploads files other than the preview video, or it moves
 * the item to another limit (to or from a skin). Any other edit saves an item as big as it already was.
 */
export function changesItemSize({ item, blobs = {} }: MeasurableItem, previous?: Item): boolean {
  if (Object.keys(blobs).some(path => path !== VIDEO_PATH)) return true
  if (!previous) return false
  return getItemSizeLimit(previous.type, previous.data.category) !== getItemSizeLimit(item.type, item.data.category)
}

/**
 * The save-time guard for an existing item: over its limit, the save is refused with the creator-facing error. A
 * measurement that fails (storage unreachable) is reported and lets the save through: infrastructure never blocks a
 * save, and the catalyst still checks at deploy.
 */
export async function guardItemSize(built: MeasurableItem, deps: MeasureDeps, flow: string): Promise<void> {
  try {
    await assertItemSize(built, deps)
  } catch (error) {
    if (isItemTooBigError(error)) throw error
    captureError(error, { flow, step: 'measure_size', itemId: built.item.id })
  }
}
