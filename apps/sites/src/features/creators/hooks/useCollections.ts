import { keepPreviousData, useQuery } from '@tanstack/react-query'
import { fetchCollectionItemPreviews, fetchCollections } from '~/lib/builder'
import {
  CollectionType,
  listFilterToParams,
  summarizeCollections,
  type CollectionListFilter,
  type CollectionSort
} from '~/lib/collections'

// Same page size as the legacy builder's collections page.
export const COLLECTIONS_PAGE_SIZE = 8

export type CollectionsFilters = {
  page: number
  search: string
  status: CollectionListFilter
  sort?: CollectionSort
  limit?: number
  /** List linked (third-party) collections too; off by default, since only My collections shows them. */
  includeLinked?: boolean
}

export function useCollections(
  address: string | undefined,
  { page, search, status, sort, limit = COLLECTIONS_PAGE_SIZE, includeLinked = false }: CollectionsFilters
) {
  return useQuery({
    queryKey: ['collections', address, page, search, status, sort, limit, includeLinked],
    queryFn: () =>
      fetchCollections(address!, {
        page,
        limit,
        q: search || undefined,
        sort,
        ...(includeLinked
          ? listFilterToParams(status)
          : { ...listFilterToParams(status), type: CollectionType.STANDARD })
      }),
    enabled: !!address,
    // Keeps the previous page rendered while the next one loads, like the legacy page.
    placeholderData: keepPreviousData,
    staleTime: 30_000
  })
}

/**
 * Every chip's count for the search, whichever chip is active: an unfiltered one-row page, since a filtered
 * list (the Linked chip's type filter) would count only what it lists.
 */
export function useCollectionsSummary(address: string | undefined, search: string) {
  return useQuery({
    queryKey: ['collections', address, 'summary', search],
    queryFn: () => fetchCollections(address!, { page: 1, limit: 1, q: search || undefined }),
    enabled: !!address,
    placeholderData: keepPreviousData,
    staleTime: 30_000,
    select: summarizeCollections
  })
}

/** First 4 item thumbnails of a collection, for the 2x2 mosaic cover. */
export function useCollectionPreview(address: string | undefined, collectionId: string, itemCount: number) {
  return useQuery({
    queryKey: ['collection-preview', address, collectionId],
    queryFn: () => fetchCollectionItemPreviews(address, collectionId),
    enabled: !!address && itemCount > 0,
    staleTime: 5 * 60_000
  })
}

// Far more than any creator has.
const ALL_DRAFTS_LIMIT = 1000

/** Every draft standard collection of the creator, for picking where to move an item. */
export function useDraftCollections(address: string | undefined, enabled = true) {
  return useQuery({
    queryKey: ['collections', address, 'drafts'],
    queryFn: () =>
      fetchCollections(address!, {
        page: 1,
        limit: ALL_DRAFTS_LIMIT,
        type: CollectionType.STANDARD,
        isPublished: false
      }),
    enabled: !!address && enabled,
    staleTime: 30_000,
    select: data => data.results
  })
}
