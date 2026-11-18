import { useMemo } from 'react'
import { useQuery } from '@tanstack/react-query'
import { fetchEntitiesByPointers } from '~/lib/catalyst'
import { useCollectionCuration } from '~/hooks/useCuration'
import { type Collection } from '~/lib/collections'
import { ItemSyncStatus, getItemSyncStatus, mapEntitiesByItemId, type ItemSync } from '~/lib/itemSync'
import { type Item } from '~/lib/items'

/** The deployed entities of a collection's items, shared with the approval flow so it plans from what the editor shows. */
export function itemEntitiesKey(collectionId: string | undefined, pointers: string[]) {
  return ['item-entities', collectionId, pointers] as const
}

/**
 * Each item's Catalyst sync status, as every view shows it: tags are left out, since a tag edit is live
 * without a review, and so are metrics the editor corrected (the approval's deploy plan still compares both). Only published items have anything
 * deployed, so drafts cost no request; the collection's pending curation turns "unsynced" into "under review".
 */
export function useItemSyncs(
  address: string | undefined,
  collection: Collection | undefined,
  items: Item[]
): Map<string, ItemSync> {
  const collectionId = collection?.id
  const isPublished = !!collection?.isPublished
  const pointers = useMemo(
    () => (isPublished ? items.flatMap(item => (item.isPublished && item.urn ? [item.urn] : [])) : []),
    [isPublished, items]
  )

  const entitiesQuery = useQuery({
    queryKey: itemEntitiesKey(collectionId, pointers),
    queryFn: () => fetchEntitiesByPointers(pointers),
    enabled: pointers.length > 0,
    staleTime: 30_000
  })
  const curationQuery = useCollectionCuration(address, collection)

  const entities = entitiesQuery.data
  // Only a successful answer settles it: during a Catalyst outage an approved item reads as loading, not as missing
  // its entity (which would offer Deploy missing entities and Publish changes for nothing).
  const entitiesLoaded = pointers.length === 0 || entitiesQuery.isSuccess
  const curationPending = curationQuery.data?.status === 'pending'
  // A failed or skipped request settles too, read as "nothing pending": edited rows then say Modified where Under
  // review would be right, which beats never resolving. Publish changes stays hidden, since the page gates it on success.
  const curationLoaded = !isPublished || !address || curationQuery.isSuccess || curationQuery.isError
  const lookupFailed = pointers.length > 0 && entitiesQuery.isError

  return useMemo(() => {
    const byItemId = mapEntitiesByItemId(items, entities ?? [])
    const syncs = new Map<string, ItemSync>()
    for (const item of items) {
      const entity = byItemId.get(item.id)
      const status = getItemSyncStatus(item, entity, {
        isCurationPending: curationPending,
        curationLoaded,
        entitiesLoaded,
        ignoreTags: true,
        ignoreMetrics: true
      })
      if (entity) syncs.set(item.id, { status, entity })
      else if (lookupFailed && status === ItemSyncStatus.LOADING) syncs.set(item.id, { status, lookupFailed })
      else syncs.set(item.id, { status })
    }
    return syncs
  }, [items, entities, curationPending, curationLoaded, entitiesLoaded, lookupFailed])
}
