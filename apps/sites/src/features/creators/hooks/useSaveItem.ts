import { useMutation, useQueryClient } from '@tanstack/react-query'
import { errorCode, track } from '~/lib/analytics'
import { captureError } from '~/lib/monitoring'
import { fetchContent, saveItem } from '~/lib/builder'
import { withCatalystImageForRarity, withThumbnail, type BuiltItem } from '~/lib/itemFactory'
import { IMAGE_PATH } from '~/lib/itemFiles'
import { changesItemSize, guardItemSize, isItemTooBigError } from '~/lib/itemSize'
import { type Item } from '~/lib/items'
import { useNotifications } from '~/lib/notifications'
import { useTranslation } from '~/intl'
import { useItemSizeDeps } from './useItemSize'
import { invalidateCollectionItems } from './usePublishCollection'

export type SaveItemVariables = Partial<BuiltItem> & {
  item: Item
  /** A raw PNG to become the item's thumbnail; hashed here, with the catalyst image alongside. */
  thumbnail?: Blob
  /** The rarity changed without a new thumbnail: the catalyst image is rebuilt from the stored one. */
  imageStale?: boolean
  /** The video hash before the edit, so a failed upload of a new video can restore it. */
  previousVideo?: string
  /** The item as stored before the edit, so that a change of category that moves its size limit is checked too. */
  previous?: Item
  /** A save the app made on its own: it has its own event, so it is not counted as the user's `Save item`. */
  background?: boolean
}

/** A rarity change that could not refresh the catalyst image, because the stored thumbnail could not be loaded. */
export function wasImageRefreshSkipped(variables: SaveItemVariables, built: BuiltItem): boolean {
  return !!variables.imageStale && !variables.thumbnail && !built.blobs[IMAGE_PATH]
}

/**
 * The item's stored thumbnail, or null when it has none or storage fails: a rarity change then saves without
 * refreshing the catalyst image rather than failing the whole save, and the failure is reported.
 */
export async function fetchStoredThumbnail(item: Item): Promise<Blob | null> {
  const hash = item.contents[item.thumbnail]
  if (!hash) return null
  try {
    return await fetchContent(hash)
  } catch (error) {
    captureError(error, { flow: 'save-item', itemId: item.id })
    return null
  }
}

/** What the save sends: a new thumbnail wins; a rarity change alone rebuilds the catalyst image. */
export async function buildSaveItem({ item, blobs, thumbnail, imageStale }: SaveItemVariables): Promise<BuiltItem> {
  if (thumbnail) return withThumbnail(item, thumbnail)
  const stored = imageStale ? await fetchStoredThumbnail(item) : null
  if (stored) return withCatalystImageForRarity(item, stored)
  return { item, blobs: blobs ?? {} }
}

/**
 * Saves an item together with any files the edit produced (thumbnail, video, replaced model). A save that changes the
 * item's files or its size limit and would leave it over that limit, thumbnail and rarity image included, is refused
 * with an ItemFileError (`size_exceeded`) before anything is sent; other edits save an oversized item as it is.
 */
export function useSaveItem(address: string | undefined) {
  const queryClient = useQueryClient()
  const { t } = useTranslation()
  const showToast = useNotifications(state => state.showToast)
  const sizeDeps = useItemSizeDeps()
  return useMutation({
    mutationFn: async (variables: SaveItemVariables) => {
      if (!address) throw new Error('Wallet disconnected')
      const built = await buildSaveItem(variables)
      if (changesItemSize(built, variables.previous)) await guardItemSize(built, sizeDeps, 'save-item')
      if (wasImageRefreshSkipped(variables, built)) showToast(t('item_editor.image_refresh_skipped'), { type: 'warn' })
      return saveItem(address, built.item, built.blobs, { previousVideo: variables.previousVideo })
    },
    onSuccess: (item, { background }) => {
      if (!background) track('Save item', { itemId: item.id })
      if (item.collectionId) invalidateCollectionItems(queryClient, item.collectionId)
    },
    onError: (error, { item, background }) => {
      if (background) return
      track('Save item error', {
        itemId: item.id,
        error: isItemTooBigError(error) ? 'size_exceeded' : errorCode(error)
      })
    }
  })
}
