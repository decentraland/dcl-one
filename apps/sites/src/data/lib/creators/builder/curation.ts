import { type CollectionCuration, type CurationFilters, type CurationRequestStatus } from '~/lib/curation'
import { type Collection, type PaginatedResource } from '~/lib/collections'
import { type RejectReasonCode } from '~/lib/events'
import { type ItemCuration } from '~/lib/linkedItemStatus'
import { fromRemoteCollection, fromRemoteCuration, fromRemoteItemCuration, toCurationQueryString } from './mappers'
import { toValidationLimitError } from './review'
import { BuilderServerError, UNPUBLISHED_COLLECTION_STATUS, request } from './transport'
import type {
  RemoteCollection,
  RemoteCollectionCuration,
  RemoteCollectionCurationBody,
  RemoteItemCuration,
  RemoteItemCurationBody
} from './wire'

/** The collection's latest curation request: GET /collections/{id}/curation. Empty data when it was never reviewed. */
export async function fetchCollectionCuration(
  address: string,
  collectionId: string
): Promise<CollectionCuration | null> {
  const curation = await request<RemoteCollectionCuration | null | undefined>(
    address,
    'GET',
    `/collections/${collectionId}/curation`,
    '',
    undefined,
    false
  )
  return curation ? fromRemoteCuration(curation) : null
}

/** The committee's address list: GET /committee (public). */
export async function fetchCommittee(): Promise<string[]> {
  const accounts = await request<{ address: string }[]>(undefined, 'GET', '/committee')
  return accounts.map(account => account.address.toLowerCase())
}

/** Every published standard collection the committee reviews: GET /collections (committee only). */
export async function fetchCurationCollections(
  address: string,
  filters: CurationFilters
): Promise<PaginatedResource<Collection>> {
  const remote = await request<PaginatedResource<RemoteCollection>>(
    address,
    'GET',
    '/collections',
    toCurationQueryString(filters)
  )
  return { ...remote, results: remote.results.map(fromRemoteCollection) }
}

/** The latest curation request of every collection the signer may see (all of them for the committee). */
export async function fetchCurations(address: string): Promise<CollectionCuration[]> {
  const remote = await request<RemoteCollectionCuration[]>(address, 'GET', '/curations')
  return remote.map(fromRemoteCuration)
}

/**
 * Opens a new review request: POST /collections/{id}/curation. Creators use it to push changes of an
 * approved collection, optionally with a note saying what changed; the committee to assign a collection that
 * was never requested. 400 while one is pending.
 */
export async function pushCollectionCuration(
  address: string,
  collectionId: string,
  assignee?: string | null,
  note?: string
): Promise<CollectionCuration> {
  const curation = { ...(assignee === undefined ? {} : { assignee }), ...(note ? { note } : {}) }
  const body = Object.keys(curation).length > 0 ? ({ curation } satisfies RemoteCollectionCurationBody) : undefined
  try {
    const remote = await request<RemoteCollectionCuration>(
      address,
      'POST',
      `/collections/${collectionId}/curation`,
      '',
      body
    )
    return fromRemoteCuration(remote)
  } catch (error) {
    // A creator's changes wait for the day's automated reviews like a retry does.
    throw toValidationLimitError(error)
  }
}

export type CurationUpdate = {
  status?: CurationRequestStatus
  assignee?: string | null
  rejectionReasons?: RejectReasonCode[]
  rejectionMessage?: string
  /** Kept with an approval only: what the curator wants the creator to know. */
  note?: string
}

/** Updates the latest request: PATCH /collections/{id}/curation. Status changes and assignees are committee-only. */
export async function updateCollectionCuration(
  address: string,
  collectionId: string,
  curation: CurationUpdate
): Promise<CollectionCuration> {
  const remote = await request<RemoteCollectionCuration>(
    address,
    'PATCH',
    `/collections/${collectionId}/curation`,
    '',
    {
      curation
    } satisfies RemoteCollectionCurationBody
  )
  return fromRemoteCuration(remote)
}

/**
 * Opens the collection's forum topic: POST /collections/{id}/post. The server writes the post and saves the link as
 * `forum_link`; 409 when the collection isn't published yet or already has a topic (its link in the error data).
 */
export async function createCollectionForumPost(address: string, collectionId: string): Promise<string> {
  return request<string>(address, 'POST', `/collections/${collectionId}/post`)
}

/** Replies on the collection's forum topic (committee only): POST /collections/{id}/curation/post. */
export async function createCurationForumReply(
  address: string,
  collectionId: string,
  forumPost: { raw: string }
): Promise<void> {
  await request(address, 'POST', `/collections/${collectionId}/curation/post`, '', { forumPost }, false)
}

/**
 * The latest curation of each item of a linked collection: GET /collections/{id}/itemCurations, optionally only
 * for some items (`itemIds[]`, sent in chunks like the legacy builder does).
 */
export async function fetchItemCurations(
  address: string,
  collectionId: string,
  itemIds?: string[]
): Promise<Map<string, ItemCuration>> {
  try {
    const chunks = itemIds ? chunk(itemIds, ITEM_CURATIONS_CHUNK) : [undefined]
    const pages = await Promise.all(
      chunks.map(ids => {
        const query = ids ? `?${new URLSearchParams(ids.map(id => ['itemIds[]', id])).toString()}` : ''
        return request<RemoteItemCuration[]>(address, 'GET', `/collections/${collectionId}/itemCurations`, query)
      })
    )
    return new Map(pages.flat().map(remote => [remote.item_id, fromRemoteItemCuration(remote)]))
  } catch (error) {
    // builder-server answers 409 "Unpublished collection" until an item of the collection has been submitted.
    if (error instanceof BuilderServerError && error.status === UNPUBLISHED_COLLECTION_STATUS) return new Map()
    throw error
  }
}

const ITEM_CURATIONS_CHUNK = 30

function chunk<T>(list: T[], size: number): T[][] {
  const chunks: T[][] = []
  for (let start = 0; start < list.length; start += size) chunks.push(list.slice(start, start + size))
  return chunks.length > 0 ? chunks : [[]]
}

/** Sends an already-curated linked item's changes for review again: POST /items/{id}/curation (no cheque). */
export async function pushItemCuration(address: string, itemId: string): Promise<ItemCuration> {
  const remote = await request<RemoteItemCuration>(address, 'POST', `/items/${itemId}/curation`)
  return fromRemoteItemCuration(remote)
}

/**
 * Sets a linked item curation's status: PATCH /items/{id}/curation. Creators send `pending` to refresh a pending
 * request with the item's current hash; committee members approve or reject.
 */
export async function updateItemCuration(
  address: string,
  itemId: string,
  status: CurationRequestStatus
): Promise<ItemCuration> {
  const remote = await request<RemoteItemCuration>(address, 'PATCH', `/items/${itemId}/curation`, '', {
    curation: { status }
  } satisfies RemoteItemCurationBody)
  return fromRemoteItemCuration(remote)
}
