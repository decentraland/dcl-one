import type { Collection, CollectionsList, FetchCollectionsParams } from '../collections'
import type { Item } from '../items'
import {
  fromRemoteCollection,
  fromRemoteItem,
  toCollectionItemPreviews,
  toCollectionsQueryString,
  toRemoteCollection
} from './mappers'
import { request } from './transport'
import type {
  RemoteCollection,
  RemoteCollectionsPage,
  RemoteItem,
  RemoteItemsPage,
  RemoteLockBody,
  RemotePublishCollectionResult,
  RemoteTOSBody
} from './wire'

export type CollectionItemPreview = {
  id: string
  name: string
  thumbnailHash: string
  isPublished: boolean
  rarity?: string
}

/**
 * The creator's collections: GET /{address}/collections. Always sends page+limit — without both,
 * builder-server answers with a bare array instead of the paginated envelope.
 */
export async function fetchCollections(address: string, params: FetchCollectionsParams): Promise<CollectionsList> {
  const page = params.page ?? 1
  const limit = params.limit ?? 20
  const query = toCollectionsQueryString({ ...params, page, limit })
  const remote = await request<RemoteCollectionsPage>(address, 'GET', `/${address}/collections`, query)
  return { ...remote, results: remote.results.map(fromRemoteCollection) }
}

/** A single collection: GET /collections/{id}. 401/404 when the signer can't see it. */
export async function fetchCollection(address: string, collectionId: string): Promise<Collection> {
  const remote = await request<RemoteCollection>(address, 'GET', `/collections/${collectionId}`)
  return fromRemoteCollection(remote)
}

/** Every item of a collection (bare-array legacy response) — the rename flow re-encodes them all. */
export async function fetchAllCollectionItems(address: string, collectionId: string): Promise<Item[]> {
  const remote = await request<RemoteItem[]>(address, 'GET', `/collections/${collectionId}/items`)
  return remote.map(fromRemoteItem)
}

/**
 * Create or update a collection: PUT /collections/{id} with { collection, data }, where `data` is
 * the ERC721CollectionV2 initialize calldata the server derives the contract address from.
 */
export async function saveCollection(address: string, collection: Collection, data: string): Promise<Collection> {
  const remote = await request<RemoteCollection>(address, 'PUT', `/collections/${collection.id}`, '', {
    collection: toRemoteCollection(collection),
    data
  })
  return fromRemoteCollection(remote)
}

/** Delete an unpublished collection and its items: DELETE /collections/{id} (409 published, 423 locked). */
export async function deleteCollection(address: string, collectionId: string): Promise<void> {
  await request<boolean>(address, 'DELETE', `/collections/${collectionId}`, '', undefined, false)
}

/**
 * The first items of a collection, for the 2x2 mosaic cover: GET /collections/{id}/items.
 * Same request the legacy CollectionImage makes.
 */
export async function fetchCollectionItemPreviews(
  address: string | undefined,
  collectionId: string,
  limit = 4
): Promise<CollectionItemPreview[]> {
  const { results } = await request<RemoteItemsPage>(
    address,
    'GET',
    `/collections/${collectionId}/items`,
    `?page=1&limit=${limit}`
  )
  return toCollectionItemPreviews(results)
}

const PUBLISH_COLLECTION_TOS_EVENT = 'publish_collection_tos'

/** Records the creator's Terms of Service acceptance for a publication: POST /collections/{id}/tos. */
export async function saveCollectionTOS(address: string, collection: Collection, email: string): Promise<void> {
  await request<unknown>(
    address,
    'POST',
    `/collections/${collection.id}/tos`,
    '',
    {
      event: PUBLISH_COLLECTION_TOS_EVENT,
      email,
      collection_address: collection.contractAddress
    } satisfies RemoteTOSBody,
    false
  )
}

/**
 * Starts the one-day publish lock: POST /collections/{id}/lock. Answers the lock timestamp; the
 * body mirrors the legacy client (the server ignores it).
 */
export async function lockCollection(address: string, collectionId: string): Promise<number> {
  const lock = await request<string>(address, 'POST', `/collections/${collectionId}/lock`, '', {
    collection_address: collectionId
  } satisfies RemoteLockBody)
  return +new Date(lock)
}

/**
 * Stores the collection's on-chain owner: POST /collections/{id}/owner/sync. The answer carries the owner
 * builder-server read from the subgraph, which is still the previous one until the transfer is indexed.
 */
export async function syncCollectionOwner(address: string, collectionId: string): Promise<Collection> {
  const remote = await request<RemoteCollection>(address, 'POST', `/collections/${collectionId}/owner/sync`)
  return fromRemoteCollection(remote)
}

/**
 * Consolidates a published collection with the chain: POST /collections/{id}/publish. builder-server
 * reads the collection from the subgraph and assigns every item its on-chain id; it answers 401
 * while the graph hasn't indexed the transaction yet, so callers retry on that status.
 */
export async function publishCollectionItems(
  address: string,
  collectionId: string
): Promise<{ collection: Collection; items: Item[] }> {
  const result = await request<RemotePublishCollectionResult>(address, 'POST', `/collections/${collectionId}/publish`)
  return { collection: fromRemoteCollection(result.collection), items: result.items.map(fromRemoteItem) }
}
