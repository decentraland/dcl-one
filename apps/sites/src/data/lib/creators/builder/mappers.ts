// Conversions between builder-server's wire shapes and the app's domain types, ported from the legacy builder
// (lib/api/builder.ts) so both apps read and write the same API identically.
import { CollectionType, toSafeLink, type Collection, type FetchCollectionsParams } from '@/data/lib/creators/collections'
import {
  CURATION_PAGE_SIZE,
  CurationStatusFilter,
  CurationTypeFilter,
  type CollectionCuration,
  type CurationFilters
} from '@/data/lib/creators/curation'
import { isRejectReasonCode, type CollectionEvent, type RejectReasonCode } from '@/data/lib/creators/events'
import type { Item } from '@/data/lib/creators/items'
import type { ItemCuration, LinkedItemsQuery } from '@/data/lib/creators/linkedItemStatus'
import type { ThirdParty } from '@/data/lib/creators/thirdParty'
import type { CollectionItemPreview } from './collections'
import { getContentsStorageUrl } from './contents'
import type { LinkedApprovalData } from './linked'
import type {
  RemoteCollection,
  RemoteCollectionCuration,
  RemoteCollectionEvent,
  RemoteItem,
  RemoteItemCuration,
  RemoteLinkedApprovalData,
  RemoteThirdParty
} from './wire'

export function fromRemoteCollection(remote: RemoteCollection): Collection {
  const collection: Collection = {
    id: remote.id,
    name: remote.name,
    owner: remote.eth_address,
    urn: remote.urn,
    isPublished: remote.is_published,
    isApproved: remote.is_approved,
    itemCount: Number(remote.item_count ?? 0),
    minters: remote.minters || [],
    managers: remote.managers || [],
    forumLink: toSafeLink(remote.forum_link),
    lock: remote.lock ? +new Date(remote.lock) : undefined,
    reviewedAt: remote.reviewed_at ? +new Date(remote.reviewed_at) : undefined,
    linkedContractAddress: remote.linked_contract_address || undefined,
    linkedContractNetwork: remote.linked_contract_network || undefined,
    createdAt: +new Date(remote.created_at),
    updatedAt: +new Date(remote.updated_at),
    isMappingComplete: remote.is_mapping_complete,
    isProgrammatic: remote.is_programmatic,
    curationStatus: remote.curation_status,
    lastActivityAt: remote.last_activity_at ? +new Date(remote.last_activity_at) : undefined
  }
  if (remote.salt) collection.salt = remote.salt
  if (remote.contract_address) collection.contractAddress = remote.contract_address
  return collection
}

// The PUT payload shape, mirroring the legacy toRemoteCollection: is_published/is_approved are
// forced false because the server rejects attempts to change them through an upsert.
export function toRemoteCollection(
  collection: Collection
): Omit<RemoteCollection, 'created_at' | 'updated_at' | 'lock' | 'is_mapping_complete'> {
  return {
    id: collection.id,
    name: collection.name,
    eth_address: collection.owner,
    salt: collection.salt || null,
    contract_address: collection.contractAddress || null,
    urn: collection.urn,
    is_published: false,
    is_approved: false,
    linked_contract_address: collection.linkedContractAddress || null,
    linked_contract_network: collection.linkedContractNetwork || null,
    minters: collection.minters,
    managers: collection.managers,
    forum_link: collection.forumLink || null,
    reviewed_at: collection.reviewedAt ? new Date(collection.reviewedAt).toISOString() : null
  }
}

// Snake_case query params, falsy values dropped — matches the legacy builder's serializer, so
// builder-server receives byte-identical requests.
export function toCollectionsQueryString(params: FetchCollectionsParams): string {
  const query = new URLSearchParams()
  if (params.isPublished !== undefined) query.append('is_published', String(params.isPublished))
  if (params.status) query.append('status', params.status)
  if (params.type) query.append('type', params.type)
  if (params.sort) query.append('sort', params.sort)
  if (params.q) query.append('q', params.q)
  if (params.contractAddress) query.append('contract_address', params.contractAddress)
  if (params.page) query.append('page', String(params.page))
  if (params.limit) query.append('limit', String(params.limit))
  const s = query.toString()
  return s ? `?${s}` : ''
}

export function fromRemoteItem(remote: RemoteItem): Item {
  const item: Item = {
    id: remote.id,
    name: remote.name,
    description: remote.description,
    thumbnail: remote.thumbnail,
    owner: remote.eth_address,
    isPublished: remote.is_published,
    isApproved: remote.is_approved,
    inCatalyst: remote.in_catalyst,
    type: remote.type,
    data: remote.data,
    metrics: remote.metrics,
    contents: remote.contents,
    createdAt: +new Date(remote.created_at),
    updatedAt: +new Date(remote.updated_at)
  }
  if (remote.collection_id) item.collectionId = remote.collection_id
  if (remote.price) item.price = remote.price
  if (remote.beneficiary) item.beneficiary = remote.beneficiary
  if (remote.rarity) item.rarity = remote.rarity
  if (remote.urn) item.urn = remote.urn
  if (remote.video) item.video = remote.video
  if (remote.utility) item.utility = remote.utility
  if (remote.total_supply !== undefined && remote.total_supply !== null) item.totalSupply = remote.total_supply
  if (remote.blockchain_item_id) item.tokenId = remote.blockchain_item_id
  if (remote.local_content_hash) item.currentContentHash = remote.local_content_hash
  if (remote.content_hash) item.blockchainContentHash = remote.content_hash
  if (remote.mappings) item.mappings = remote.mappings
  if (remote.isMappingComplete !== undefined) item.isMappingComplete = remote.isMappingComplete
  if (remote.catalyst_content_hash) item.catalystContentHash = remote.catalyst_content_hash
  return item
}

/**
 * The wire shape for PUT /items/:id, byte-compatible with the legacy builder's toRemoteItem. Meant for
 * creating/updating draft items only: publication flags are always sent as false, since the server owns them.
 */
export function toRemoteItem(item: Item): Omit<RemoteItem, 'created_at' | 'updated_at' | 'in_catalyst'> {
  return {
    id: item.id,
    name: item.name,
    description: item.description || '',
    thumbnail: item.thumbnail,
    video: item.video || null,
    eth_address: item.owner,
    collection_id: item.collectionId || null,
    blockchain_item_id: item.tokenId || null,
    price: item.price || null,
    urn: item.urn || null,
    beneficiary: item.beneficiary || null,
    rarity: item.rarity || null,
    total_supply: item.totalSupply === undefined ? null : item.totalSupply,
    is_published: false,
    is_approved: false,
    utility: item.utility || null,
    mappings: item.mappings ?? null,
    type: item.type,
    data: item.data,
    metrics: item.metrics,
    contents: item.contents,
    content_hash: null
  }
}

/** The items a collection cover can show: the ones whose thumbnail is among their contents. */
export function toCollectionItemPreviews(items: RemoteItem[]): CollectionItemPreview[] {
  return items
    .filter(item => item.contents[item.thumbnail])
    .map(item => ({
      id: item.id,
      name: item.name,
      thumbnailUrl: getContentsStorageUrl(item.contents[item.thumbnail]),
      rarity: item.rarity ?? undefined
    }))
}

function toRejectReasons(codes: string[] | null | undefined): RejectReasonCode[] | null {
  if (!codes) return null
  return codes.filter(isRejectReasonCode)
}

export function fromRemoteCuration(remote: RemoteCollectionCuration): CollectionCuration {
  return {
    id: remote.id,
    collectionId: remote.collection_id,
    status: remote.status,
    assignee: remote.assignee ? remote.assignee.toLowerCase() : null,
    reviewedBy: remote.reviewed_by ? remote.reviewed_by.toLowerCase() : null,
    rejectionReasons: toRejectReasons(remote.rejection_reasons),
    rejectionMessage: remote.rejection_message ?? null,
    createdAt: +new Date(remote.created_at),
    updatedAt: +new Date(remote.updated_at)
  }
}

export function fromRemoteEvent(remote: RemoteCollectionEvent): CollectionEvent {
  return {
    id: remote.id,
    collectionId: remote.collection_id,
    type: remote.type,
    actor: remote.actor,
    actorAddress: remote.actor_address ? remote.actor_address.toLowerCase() : null,
    payload: remote.payload ?? {},
    createdAt: +new Date(remote.created_at)
  }
}

// builder-server splits a pending request by assignee; the picker doesn't, since the assignee filter already does.
const SERVER_STATUSES: Record<CurationStatusFilter, string[]> = {
  [CurationStatusFilter.TO_REVIEW]: ['to_review', 'under_review'],
  [CurationStatusFilter.AUTO_REVIEWING]: ['auto_reviewing'],
  [CurationStatusFilter.APPROVED]: ['approved'],
  [CurationStatusFilter.REJECTED]: ['rejected'],
  [CurationStatusFilter.DISABLED]: ['disabled']
}

/** GET /collections query string for the curation list: published collections, standard and linked. */
export function toCurationQueryString(filters: CurationFilters, limit = CURATION_PAGE_SIZE): string {
  const query = new URLSearchParams()
  query.append('is_published', 'true')
  for (const assignee of filters.assignees) query.append('assignee', assignee)
  for (const status of filters.statuses.flatMap(status => SERVER_STATUSES[status])) query.append('status', status)
  if (filters.reviewType) query.append('review_type', filters.reviewType)
  if (filters.type === CurationTypeFilter.STANDARD) query.append('type', CollectionType.STANDARD)
  if (filters.type === CurationTypeFilter.LINKED) query.append('type', CollectionType.THIRD_PARTY)
  query.append('sort', filters.sort)
  if (filters.search) query.append('q', filters.search)
  if (filters.tag) query.append('tag', filters.tag.toLowerCase())
  query.append('page', String(filters.page))
  query.append('limit', String(limit))
  return `?${query.toString()}`
}

export function fromRemoteItemCuration(remote: RemoteItemCuration): ItemCuration {
  return {
    id: remote.id,
    itemId: remote.item_id,
    status: remote.status,
    contentHash: remote.content_hash,
    isMappingComplete: remote.is_mapping_complete,
    createdAt: +new Date(remote.created_at),
    updatedAt: +new Date(remote.updated_at)
  }
}

/** GET /collections/{id}/items query string for one page of a linked collection's items. */
export function toLinkedItemsQueryString(page: number, limit: number, filter: LinkedItemsQuery = {}): string {
  const query = new URLSearchParams({ page: String(page), limit: String(limit) })
  if (filter.status) query.set('status', filter.status)
  if (filter.synced !== undefined) query.set('synced', String(filter.synced))
  if (filter.mappingStatus) query.set('mappingStatus', filter.mappingStatus)
  return `?${query.toString()}`
}

export function fromRemoteLinkedApprovalData(remote: RemoteLinkedApprovalData): LinkedApprovalData {
  return {
    cheque: remote.cheque,
    root: remote.root,
    contentHashes: remote.content_hashes,
    chequeWasConsumed: remote.chequeWasConsumed
  }
}

export function fromRemoteThirdParty(remote: RemoteThirdParty): ThirdParty {
  return {
    id: remote.id,
    root: remote.root ?? '',
    managers: remote.managers ?? [],
    name: remote.name ?? '',
    description: remote.description ?? '',
    isApproved: !!remote.isApproved,
    isProgrammatic: !!remote.isProgrammatic,
    maxItems: remote.maxItems ?? '0',
    contracts: remote.contracts ?? [],
    published: !!remote.published
  }
}
