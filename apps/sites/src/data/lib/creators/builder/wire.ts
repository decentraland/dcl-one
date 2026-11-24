// builder-server's wire contract: the request and response shapes as the legacy server sends and takes them.
// Only this folder sees them; the rest of the app works with the domain types the mappers produce.
import type { ContractNetwork, Mappings } from '@dcl/schemas'
import type { CollectionStatusCounts, CurationStatus, PaginatedResource } from '../collections'
import type { CurationRequestStatus } from '../curation'
import type { CollectionEventActor, CollectionEventPayload } from '../events'
import type { ItemData, ItemMetrics, ItemType } from '../items'
import type { SlotsCheque } from './linked'

export type RemoteCollection = {
  id: string
  name: string
  eth_address: string
  salt: string | null
  contract_address: string | null
  urn: string
  is_published: boolean
  is_approved: boolean
  minters: string[]
  managers: string[]
  forum_link: string | null
  lock: string | null
  reviewed_at: string | null
  created_at: string
  updated_at: string
  item_count?: string
  linked_contract_address: string | null
  linked_contract_network: string | null
  is_mapping_complete: boolean
  is_programmatic?: boolean
  /** Only in `/:address/collections` lists. */
  curation_status?: CurationStatus | null
  last_activity_at?: string
}

export type RemoteCollectionsPage = PaginatedResource<RemoteCollection> & { counts?: CollectionStatusCounts }

export type RemoteItem = {
  id: string
  name: string
  description: string
  thumbnail: string
  video?: string | null
  urn?: string | null
  eth_address: string
  collection_id: string | null
  blockchain_item_id?: string | null
  price: string | null
  beneficiary: string | null
  rarity: string | null
  total_supply?: number | null
  is_published: boolean
  is_approved: boolean
  in_catalyst: boolean
  utility?: string | null
  mappings?: Mappings | null
  /** Linked items: whether the latest item curation covered a mapping (`isMappingComplete` on the wire). */
  isMappingComplete?: boolean
  type: ItemType
  data: ItemData
  metrics?: ItemMetrics
  contents: Record<string, string>
  content_hash?: string | null
  local_content_hash?: string | null
  /** Linked items: the hash of the entity the Catalyst holds, null until the item is approved. */
  catalyst_content_hash?: string | null
  created_at: string
  updated_at: string
}

export type RemoteItemsPage = PaginatedResource<RemoteItem>

export type RemoteCollectionCuration = {
  id: string
  collection_id: string
  status: CurationRequestStatus
  assignee?: string | null
  reviewed_by?: string | null
  rejection_reasons?: string[] | null
  rejection_message?: string | null
  created_at: string
  updated_at: string
}

export type RemoteCollectionEvent = {
  id: string
  collection_id: string
  type: string
  actor: CollectionEventActor
  actor_address?: string | null
  payload?: CollectionEventPayload | null
  created_at: string
}

export type RemoteCollectionEventsPage = {
  results: RemoteCollectionEvent[]
  total: number
  page: number
  limit: number
}

export type RemoteItemCuration = {
  id: string
  item_id: string
  status: CurationStatus
  content_hash: string
  is_mapping_complete: boolean | null
  created_at: string
  updated_at: string
}

/** Already camelCase on the wire; a virtual third party may leave any field but its id out. */
export type RemoteThirdParty = {
  id: string
  root?: string
  managers?: string[]
  name?: string
  description?: string
  isApproved?: boolean
  isProgrammatic?: boolean
  maxItems?: string
  contracts?: { network: ContractNetwork; address: string }[]
  published?: boolean
}

export type RemotePublishCollectionResult = { collection: RemoteCollection; items: RemoteItem[] }

export type RemotePublishLinkedItemsResult = {
  collection: RemoteCollection
  items: RemoteItem[]
  itemCurations: RemoteItemCuration[]
}

export type RemoteLinkedApprovalData = {
  cheque: SlotsCheque
  root: string | null
  content_hashes: Record<string, string>
  chequeWasConsumed: boolean
}

/** POST /collections/{id}/tos: linked publishes also send the submitted content hashes. */
export type RemoteTOSBody = {
  event: string
  email: string
  collection_address: string | undefined
  hashes?: string[]
}

export type RemoteLockBody = { collection_address: string }

export type RemoteCollectionCurationBody = {
  curation: {
    status?: CurationRequestStatus
    assignee?: string | null
    rejectionReasons?: string[]
    rejectionMessage?: string
    note?: string
  }
}

export type RemoteItemCurationBody = { curation: { status: CurationRequestStatus; contentHash?: string } }
