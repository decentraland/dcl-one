import { useCallback, useMemo } from 'react'
import { keepPreviousData, useQuery } from '@tanstack/react-query'
import {
  fetchAllLinkedCollectionItems,
  fetchItemCurations,
  fetchLinkedCollectionItems,
  fetchThirdParty,
  type LinkedItemsPage
} from '~/lib/builder'
import { type Collection } from '~/lib/collections'
import { type Item } from '~/lib/items'
import {
  LinkedItemFilter,
  hasLinkedWork,
  isLinkedDraft,
  linkedItemFilterToQuery,
  type ItemCuration
} from '~/lib/linkedItemStatus'
import { getThirdPartyId } from '~/lib/thirdParty'

/**
 * The drafts of a linked collection (never submitted, or rejected). builder-server filters by a curation's status but
 * not by its absence, so every item and curation is read and filtered here.
 */
async function fetchDraftLinkedItems(address: string, collectionId: string): Promise<Item[]> {
  const [items, curations] = await Promise.all([
    fetchAllLinkedCollectionItems(address, collectionId),
    fetchItemCurations(address, collectionId)
  ])
  return items.filter(item => isLinkedDraft(curations.get(item.id)))
}

/**
 * Reads every item of a linked collection across pages: under a filter (drafts included) for "Select all", or all
 * of them without one.
 */
export function useFetchLinkedItems(address: string, collectionId: string) {
  return useCallback(
    (filter?: LinkedItemFilter): Promise<Item[]> =>
      filter === LinkedItemFilter.DRAFT
        ? fetchDraftLinkedItems(address, collectionId)
        : fetchAllLinkedCollectionItems(address, collectionId, filter ? linkedItemFilterToQuery(filter) : {}),
    [address, collectionId]
  )
}

// Same page size as the legacy third-party collection page.
export const LINKED_ITEMS_PAGE_SIZE = 50

export function itemCurationsKey(address: string | undefined, collectionId: string | undefined) {
  return ['item-curations', address, collectionId] as const
}

/** The latest curation of each item of a linked collection, keyed by item id. */
export function useItemCurations(address: string | undefined, collection: Collection | undefined, items?: Item[]) {
  const itemIds = useMemo(() => items?.map(item => item.id), [items])
  return useQuery({
    queryKey: [...itemCurationsKey(address, collection?.id), itemIds],
    queryFn: () => fetchItemCurations(address!, collection!.id, itemIds),
    enabled: !!address && !!collection && (!items || items.length > 0),
    placeholderData: keepPreviousData,
    staleTime: 30_000
  })
}

export function thirdPartyKey(address: string | undefined, thirdPartyId: string | undefined) {
  return ['third-party', address, thirdPartyId] as const
}

/** The linked collection's third party: its name for the header, its managers for access, its kind and root. */
export function useThirdParty(address: string | undefined, collection: Collection | undefined) {
  const thirdPartyId = collection ? getThirdPartyId(collection) : undefined
  return useQuery({
    queryKey: thirdPartyKey(address, thirdPartyId),
    queryFn: () => fetchThirdParty(address!, thirdPartyId!),
    enabled: !!address && !!thirdPartyId,
    staleTime: 5 * 60_000,
    retry: false
  })
}

export function thirdPartySlotsKey(address: string | undefined, thirdPartyId: string | undefined) {
  return ['third-party-slots', address, thirdPartyId] as const
}

export function linkedItemsKey(address: string | undefined, collectionId: string | undefined) {
  return ['linked-items', address, collectionId] as const
}

/** One server-filtered page of a linked collection's items. */
export function useLinkedCollectionItems(
  address: string | undefined,
  collection: Collection | undefined,
  page: number,
  filter: LinkedItemFilter,
  limit = LINKED_ITEMS_PAGE_SIZE
) {
  const isDraft = filter === LinkedItemFilter.DRAFT
  const pageQuery = useQuery<LinkedItemsPage>({
    queryKey: [...linkedItemsKey(address, collection?.id), page, filter, limit],
    queryFn: () => fetchLinkedCollectionItems(address!, collection!.id, page, limit, linkedItemFilterToQuery(filter)),
    enabled: !!address && !!collection && !isDraft,
    placeholderData: keepPreviousData,
    staleTime: 30_000
  })
  // Drafts are filtered here from the whole collection: read once, then each page is a slice of the same list.
  const draftsQuery = useQuery<Item[], Error, LinkedItemsPage>({
    queryKey: [...linkedItemsKey(address, collection?.id), LinkedItemFilter.DRAFT],
    queryFn: () => fetchDraftLinkedItems(address!, collection!.id),
    enabled: !!address && !!collection && isDraft,
    select: drafts => ({
      results: drafts.slice((page - 1) * limit, page * limit),
      total: drafts.length,
      page,
      pages: Math.ceil(drafts.length / limit),
      limit
    }),
    staleTime: 30_000
  })
  return isDraft ? draftsQuery : pageQuery
}

const NO_IDS: string[] = []

/** How many items the server counts under a filter, from a one-row page. */
function useLinkedItemCount(address: string | undefined, collection: Collection | undefined, filter: LinkedItemFilter) {
  return useQuery({
    queryKey: [...linkedItemsKey(address, collection?.id), 'count', filter],
    queryFn: async () =>
      (await fetchLinkedCollectionItems(address!, collection!.id, 1, 1, linkedItemFilterToQuery(filter))).total,
    enabled: !!address && !!collection,
    staleTime: 30_000
  })
}

/**
 * Whether the collection holds anything to publish or push, across every page: never-submitted items with a mapping,
 * rejected ones, and approved ones whose content changed. Undefined until known.
 */
export function useHasLinkedWork(
  address: string | undefined,
  collection: Collection | undefined,
  curations: Map<string, ItemCuration> | undefined
): boolean | undefined {
  const unsynced = useLinkedItemCount(address, collection, LinkedItemFilter.READY_TO_PUSH)
  // Counted first: only collections from before mappings existed have any, and only those pay for listing them whole
  // (to tell the curated ones apart).
  const missingCount = useLinkedItemCount(address, collection, LinkedItemFilter.PENDING_MAPPING)
  const missingMapping = useQuery({
    queryKey: [...linkedItemsKey(address, collection?.id), 'ids', LinkedItemFilter.PENDING_MAPPING],
    queryFn: async () =>
      (
        await fetchAllLinkedCollectionItems(
          address!,
          collection!.id,
          linkedItemFilterToQuery(LinkedItemFilter.PENDING_MAPPING)
        )
      ).map(item => item.id),
    enabled: !!address && !!collection && (missingCount.data ?? 0) > 0,
    staleTime: 30_000
  })
  const missingMappingIds = missingCount.data === 0 ? NO_IDS : missingMapping.data
  // A failed count fails open: hiding Publish would leave the creator with no way forward and no message, while the
  // button's own checks and the publish run still stop anything that can't go through.
  const countFailed = unsynced.isError || missingCount.isError || missingMapping.isError
  return useMemo(() => {
    if (countFailed && collection) return true
    if (!collection || !curations || unsynced.data === undefined || missingMappingIds === undefined) return undefined
    return hasLinkedWork({
      itemCount: collection.itemCount,
      curations,
      unsynced: unsynced.data,
      missingMappingIds
    })
  }, [collection, curations, unsynced.data, missingMappingIds, countFailed])
}
