import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { type Entity } from '@dcl/schemas'
import { errorCode, track } from '~/lib/analytics'
import { fetchContent, saveItem } from '~/lib/builder'
import { fetchCatalystContent } from '~/lib/catalyst'
import { buildResetItem } from '~/lib/itemSync'
import { type Item } from '~/lib/items'
import { invalidateCollectionItems } from './usePublishCollection'

/** Moves a draft item into another draft collection: the same item save with a new collection id. */
export function useMoveItem(address: string | undefined) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async ({ item, collectionId }: { item: Item; collectionId: string }) => {
      if (!address) throw new Error('Wallet disconnected')
      return saveItem(address, { ...item, collectionId }, {})
    },
    // Both collections change, so every collection query is refetched rather than the two by id.
    onSuccess: (_, { item, collectionId }) => {
      track('Move item', { itemId: item.id, collectionId })
      void queryClient.invalidateQueries({ queryKey: ['collection-items-all'] })
      void queryClient.invalidateQueries({ queryKey: ['collection'] })
      void queryClient.invalidateQueries({ queryKey: ['collections'] })
    },
    onError: (error, { item }) => track('Move item error', { itemId: item.id, error: errorCode(error) })
  })
}

/**
 * Puts an approved item back to the version deployed on the Catalyst: re-uploads the entity's files
 * and saves its metadata over the builder copy, as the legacy reset-item flow does.
 */
export function useResetItem(address: string | undefined) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async ({ item, entity }: { item: Item; entity: Entity }) => {
      if (!address) throw new Error('Wallet disconnected')
      const reset = buildResetItem(item, entity)
      // Re-upload the deployed files only: the video and anything else the Catalyst never received is
      // already in builder storage under its hash.
      const entries = await Promise.all(
        (entity.content ?? []).map(async ({ file, hash }) => [file, await fetchCatalystContent(hash)] as const)
      )
      return saveItem(address, reset, Object.fromEntries(entries))
    },
    onSuccess: item => {
      track('Reset changes', { itemId: item.id })
      if (item.collectionId) invalidateCollectionItems(queryClient, item.collectionId)
      void queryClient.invalidateQueries({ queryKey: ['item-entities'] })
    },
    onError: (error, { item }) => track('Reset changes error', { itemId: item.id, error: errorCode(error) })
  })
}

/** A stored video by hash, read once per hash. */
export function useItemVideo(hash: string | undefined, enabled: boolean) {
  return useQuery({
    queryKey: ['item-video', hash],
    queryFn: () => fetchContent(hash!),
    enabled: enabled && !!hash,
    staleTime: Infinity
  })
}
