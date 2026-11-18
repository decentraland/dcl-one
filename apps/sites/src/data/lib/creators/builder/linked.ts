import { CurationStatus, type Collection, type PaginatedResource } from '@/data/lib/creators/collections'
import { type Item } from '@/data/lib/creators/items'
import { type ItemCuration, type LinkedItemsQuery } from '@/data/lib/creators/linkedItemStatus'
import { type ThirdParty } from '@/data/lib/creators/thirdParty'
import {
  fromRemoteCollection,
  fromRemoteItem,
  fromRemoteItemCuration,
  fromRemoteLinkedApprovalData,
  fromRemoteThirdParty,
  toLinkedItemsQueryString
} from './mappers'
import { request } from './transport'
import type {
  RemoteItemsPage,
  RemoteLinkedApprovalData,
  RemotePublishLinkedItemsResult,
  RemoteThirdParty,
  RemoteTOSBody
} from './wire'

/** A third party: GET /thirdParties/{id}. 404 for an unpublished one the signer doesn't manage. */
export async function fetchThirdParty(address: string, thirdPartyId: string): Promise<ThirdParty> {
  const remote = await request<RemoteThirdParty>(address, 'GET', `/thirdParties/${encodeURIComponent(thirdPartyId)}`)
  return fromRemoteThirdParty(remote)
}

/**
 * Item slots the third party still has: GET /thirdParties/{id}/slots (bought slots minus items ever submitted).
 * builder-server answers 0 on any internal failure, so 0 is also "unknown".
 */
export async function fetchThirdPartySlots(address: string, thirdPartyId: string): Promise<number> {
  const slots = await request<number>(address, 'GET', `/thirdParties/${encodeURIComponent(thirdPartyId)}/slots`)
  return Number(slots) || 0
}

/** Picks the third party's kind before its first publish: PATCH /thirdParties/{id} (virtual ones only). */
export async function setThirdPartyKind(address: string, thirdPartyId: string, isProgrammatic: boolean): Promise<void> {
  await request<unknown>(
    address,
    'PATCH',
    `/thirdParties/${encodeURIComponent(thirdPartyId)}`,
    '',
    { isProgrammatic },
    false
  )
}

/**
 * Drops the virtual third party once it exists on chain: DELETE /thirdParties/{id}. builder-server answers 400 until
 * the subgraph has indexed the registration, so callers retry on that status.
 */
export async function deleteVirtualThirdParty(address: string, thirdPartyId: string): Promise<void> {
  await request<unknown>(address, 'DELETE', `/thirdParties/${encodeURIComponent(thirdPartyId)}`, '', undefined, false)
}

export type LinkedItemsPage = PaginatedResource<Item>

/**
 * One page of a linked collection's items, server-filtered: GET /collections/{id}/items?page&limit&synced&mappingStatus.
 * Linked collections can hold thousands of items, so they are never fetched whole.
 */
export async function fetchLinkedCollectionItems(
  address: string,
  collectionId: string,
  page: number,
  limit: number,
  filter: LinkedItemsQuery = {}
): Promise<LinkedItemsPage> {
  const remote = await request<RemoteItemsPage>(
    address,
    'GET',
    `/collections/${collectionId}/items`,
    toLinkedItemsQueryString(page, limit, filter)
  )
  return { ...remote, results: remote.results.map(fromRemoteItem) }
}

/** Every item of a linked collection under a filter, page by page: what "Select all" picks across pages. */
export async function fetchAllLinkedCollectionItems(
  address: string,
  collectionId: string,
  filter: LinkedItemsQuery = {},
  limit = 500
): Promise<Item[]> {
  const items: Item[] = []
  for (let page = 1; ; page++) {
    const result = await fetchLinkedCollectionItems(address, collectionId, page, limit, filter)
    items.push(...result.results)
    if (page >= result.pages || result.results.length === 0) return items
  }
}

/** The linked items waiting for review, every page of them: GET /collections/{id}/items?status=pending. */
export async function fetchPendingLinkedItems(address: string, collectionId: string, limit = 50): Promise<Item[]> {
  const items: Item[] = []
  for (let page = 1; ; page++) {
    const query = toLinkedItemsQueryString(page, limit, { status: CurationStatus.PENDING })
    const remote = await request<RemoteItemsPage>(address, 'GET', `/collections/${collectionId}/items`, query)
    items.push(...remote.results.map(fromRemoteItem))
    if (page >= remote.pages || remote.results.length === 0) break
  }
  return items
}

/** The EIP-712 "ConsumeSlots" cheque the creator signs for a publish, as builder-server stores it. */
export type SlotsCheque = { signature: string; qty: number; salt: string }

export type PublishLinkedItemsResult = { collection: Collection; items: Item[]; itemCurations: ItemCuration[] }

/**
 * Submits linked items for review: POST /collections/{id}/publish with the item ids and the signed cheque. One
 * pending item curation per item; 409 when slots are short or a review is already pending.
 */
export async function publishLinkedItems(
  address: string,
  collectionId: string,
  itemIds: string[],
  cheque: SlotsCheque
): Promise<PublishLinkedItemsResult> {
  const result = await request<RemotePublishLinkedItemsResult>(
    address,
    'POST',
    `/collections/${collectionId}/publish`,
    '',
    { itemIds, cheque }
  )
  return {
    collection: fromRemoteCollection(result.collection),
    items: result.items.map(fromRemoteItem),
    itemCurations: result.itemCurations.map(fromRemoteItemCuration)
  }
}

const PUBLISH_LINKED_ITEMS_TOS_EVENT = 'publish_third_party_items_tos'

/** Records the creator's Terms of Service acceptance for a linked publish, with the submitted content hashes. */
export async function saveLinkedItemsTOS(
  address: string,
  collection: Collection,
  email: string,
  hashes: string[]
): Promise<void> {
  await request<unknown>(
    address,
    'POST',
    `/collections/${collection.id}/tos`,
    '',
    { event: PUBLISH_LINKED_ITEMS_TOS_EVENT, email, collection_address: collection.id, hashes } satisfies RemoteTOSBody,
    false
  )
}

/** What the committee needs to approve a linked collection's pending items (committee only). */
export type LinkedApprovalData = {
  cheque: SlotsCheque
  /** The third party's current Merkle root on chain, null before its first approval. */
  root: string | null
  /** The latest curated content hash of every item ever submitted, by item id: the whole tree's leaves. */
  contentHashes: Record<string, string>
  /** Whether the cheque's slots were already consumed on chain (a retry, or a push with no new slots). */
  chequeWasConsumed: boolean
}

/** GET /collections/{id}/approvalData. */
export async function fetchLinkedApprovalData(address: string, collectionId: string): Promise<LinkedApprovalData> {
  const remote = await request<RemoteLinkedApprovalData>(address, 'GET', `/collections/${collectionId}/approvalData`)
  return fromRemoteLinkedApprovalData(remote)
}
