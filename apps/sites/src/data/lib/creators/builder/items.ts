import type { BlockchainRarity } from '../rarities'
import { VIDEO_PATH, tracksVideoOnSave, type Item } from '../items'
import { fromRemoteItem, toRemoteItem } from './mappers'
import { request } from './transport'
import type { RemoteItem } from './wire'

/** One item as builder-server holds it right now: GET /items/{id}. */
export async function fetchItem(address: string, itemId: string): Promise<Item> {
  return fromRemoteItem(await request<RemoteItem>(address, 'GET', `/items/${itemId}`))
}

/**
 * Create or update an item and upload its files: PUT /items/{id} with the remote item, then
 * POST /items/{id}/files as multipart where each field name is the file's content hash — the
 * same two-step save the legacy builder performs. A smart wearable's preview video goes to
 * POST /items/{id}/videos instead (field name = path), which has its own 250MB cap.
 */
export type SaveItemOptions = {
  /** The video hash the item had before this save, restored if the new video fails to upload. */
  previousVideo?: string
}

export async function saveItem(
  address: string,
  item: Item,
  blobs: Record<string, Blob>,
  { previousVideo }: SaveItemOptions = {}
): Promise<Item> {
  const remote = await request<RemoteItem>(address, 'PUT', `/items/${item.id}`, '', { item: toRemoteItem(item) })
  const { [VIDEO_PATH]: video, ...fileBlobs } = blobs
  if (Object.keys(fileBlobs).length > 0) {
    const files = new FormData()
    for (const path in fileBlobs) files.append(item.contents[path], fileBlobs[path])
    await request<unknown>(address, 'POST', `/items/${item.id}/files`, '', files, false)
  }
  if (video) {
    const videos = new FormData()
    videos.append(VIDEO_PATH, video)
    try {
      await request<unknown>(address, 'POST', `/items/${item.id}/videos`, '', videos, false)
    } catch (error) {
      // The PUT already stored the new video hash; point the item back at the video it had so it never
      // references a file that was not uploaded. Callers that know the previous hash pass it; otherwise an
      // approved item keeps its curation-approved video (`item.video`, which the save leaves alone) and an
      // unapproved one, whose `video` already moved to the new hash, drops it. Best effort: the caller gets
      // the upload error either way.
      const previous = previousVideo ?? (tracksVideoOnSave(item) ? undefined : item.video)
      const contents = { ...item.contents }
      if (previous) contents[VIDEO_PATH] = previous
      else delete contents[VIDEO_PATH]
      await request<RemoteItem>(address, 'PUT', `/items/${item.id}`, '', {
        item: toRemoteItem({ ...item, video: previous, contents })
      }).catch(() => undefined)
      throw error
    }
  }
  return fromRemoteItem(remote)
}

/** Rarities with their current USD and MANA prices: GET /rarities. Every rarity costs the same. */
export async function fetchRarities(address: string | undefined): Promise<BlockchainRarity[]> {
  return request<BlockchainRarity[]>(address, 'GET', '/rarities')
}

/** Removes a draft item (and its files): DELETE /items/{id}. */
export async function deleteItem(address: string, itemId: string): Promise<void> {
  await request<boolean>(address, 'DELETE', `/items/${itemId}`)
}
