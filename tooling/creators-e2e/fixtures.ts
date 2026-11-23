// Builder-server rows in the wire shape (snake_case, `{ ok, data }` unwrapped), as `lib/builder` reads them.
import { TEST_ADDRESS } from './helpers/session'

const NOW = '2026-11-01T12:00:00.000Z'
const MALE = 'urn:decentraland:off-chain:base-avatars:BaseMale'
const FEMALE = 'urn:decentraland:off-chain:base-avatars:BaseFemale'

export type RemoteCollectionRow = {
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
  item_count: string
  linked_contract_address: string | null
  linked_contract_network: string | null
  is_mapping_complete: boolean
  curation_status?: 'pending' | 'approved' | 'rejected' | null
  last_activity_at?: string
}

export type RemoteThirdPartyRow = {
  id: string
  name: string
  description: string
  root: string
  managers: string[]
  isApproved: boolean
  isProgrammatic: boolean
  maxItems: string
  contracts: Array<{ network: string; address: string }>
  published: boolean
}

export type RemoteItemRow = {
  id: string
  name: string
  description: string
  thumbnail: string
  eth_address: string
  collection_id: string | null
  blockchain_item_id: string | null
  price: string | null
  beneficiary: string | null
  rarity: string | null
  total_supply: number
  is_published: boolean
  is_approved: boolean
  in_catalyst: boolean
  type: 'wearable' | 'emote'
  data: Record<string, unknown>
  metrics: Record<string, number>
  contents: Record<string, string>
  content_hash?: string | null
  local_content_hash?: string | null
  catalyst_content_hash?: string | null
  mappings?: Record<string, unknown> | null
  urn: string | null
  created_at: string
  updated_at: string
}

/** Content hashes of the committed fixture files; the fake storage serves each one's real bytes. */
export const HASH = {
  hat: 'bafyhat',
  tshirtMale: 'bafytshirtmale',
  tshirtFemale: 'bafytshirtfemale',
  poloMale: 'bafypolomale',
  poloFemale: 'bafypolofemale',
  emote: 'bafyemote',
  thumbnail: 'bafythumbnail'
} as const

let sequence = 0
const nextId = () => `00000000-0000-4000-8000-${String(++sequence).padStart(12, '0')}`

const representation = (bodyShape: string, file: string) => ({
  bodyShapes: [bodyShape],
  mainFile: file,
  contents: [file],
  overrideHides: [],
  overrideReplaces: []
})

export function collection(overrides: Partial<RemoteCollectionRow> = {}): RemoteCollectionRow {
  const id = overrides.id ?? nextId()
  return {
    id,
    name: 'Neon Streetwear',
    eth_address: TEST_ADDRESS,
    salt: null,
    contract_address: null,
    urn: `urn:decentraland:amoy:collections-v2:${id}`,
    is_published: false,
    is_approved: false,
    minters: [],
    managers: [],
    forum_link: null,
    lock: null,
    reviewed_at: null,
    created_at: NOW,
    updated_at: NOW,
    item_count: '0',
    linked_contract_address: null,
    linked_contract_network: null,
    is_mapping_complete: false,
    curation_status: null,
    last_activity_at: NOW,
    ...overrides
  }
}

/** A collection on chain and approved: what the list shows as Published. */
export function publishedCollection(overrides: Partial<RemoteCollectionRow> = {}): RemoteCollectionRow {
  return collection({
    contract_address: '0x' + 'a1'.repeat(20),
    salt: '0x' + '00'.repeat(32),
    is_published: true,
    is_approved: true,
    reviewed_at: NOW,
    curation_status: 'approved',
    ...overrides
  })
}

export const THIRD_PARTY_ID = 'urn:decentraland:amoy:collections-thirdparty:brand-x'
/** The NFT contract every linked fixture maps its items to. */
export const LINKED_CONTRACT = '0x' + 'c3'.repeat(20)

export function thirdParty(overrides: Partial<RemoteThirdPartyRow> = {}): RemoteThirdPartyRow {
  return {
    id: THIRD_PARTY_ID,
    name: 'Brand X',
    description: 'Linked wearables for e2e runs',
    root: '',
    managers: [TEST_ADDRESS],
    isApproved: true,
    isProgrammatic: false,
    maxItems: '100',
    contracts: [{ network: 'amoy', address: LINKED_CONTRACT }],
    published: true,
    ...overrides
  }
}

/** A linked collection with a submitted item: what builder-server marks published. */
export function linkedCollection(overrides: Partial<RemoteCollectionRow> = {}): RemoteCollectionRow {
  const id = overrides.id ?? nextId()
  return collection({
    id,
    eth_address: '',
    urn: `${THIRD_PARTY_ID}:${id.slice(-4)}`,
    is_published: true,
    linked_contract_address: LINKED_CONTRACT,
    linked_contract_network: 'amoy',
    is_mapping_complete: true,
    ...overrides
  })
}

export function wearable(overrides: Partial<RemoteItemRow> = {}): RemoteItemRow {
  return {
    id: nextId(),
    name: 'Red Bandana',
    description: '',
    thumbnail: 'thumbnail.png',
    eth_address: TEST_ADDRESS,
    collection_id: null,
    blockchain_item_id: null,
    price: null,
    beneficiary: null,
    rarity: null,
    total_supply: 0,
    is_published: false,
    is_approved: false,
    in_catalyst: false,
    type: 'wearable',
    data: {
      category: 'hat',
      hides: [],
      replaces: [],
      tags: [],
      requiredPermissions: [],
      representations: [representation(MALE, 'male/hat.glb'), representation(FEMALE, 'female/hat.glb')]
    },
    metrics: { triangles: 540, materials: 1, textures: 1, meshes: 1, bodies: 1, entities: 1 },
    contents: { 'male/hat.glb': HASH.hat, 'female/hat.glb': HASH.hat, 'thumbnail.png': HASH.thumbnail },
    urn: null,
    created_at: NOW,
    updated_at: NOW,
    ...overrides
  }
}

/** An upper-body wearable with a model per body shape; `invalid` uses the polo pair, which fails texture-size. */
export function upperBody(overrides: Partial<RemoteItemRow> = {}, { invalid = false } = {}): RemoteItemRow {
  const [male, female] = invalid ? [HASH.poloMale, HASH.poloFemale] : [HASH.tshirtMale, HASH.tshirtFemale]
  return wearable({
    name: invalid ? 'Striped Polo' : 'Yellow T-Shirt',
    data: {
      category: 'upper_body',
      hides: [],
      replaces: [],
      tags: [],
      requiredPermissions: [],
      representations: [representation(MALE, 'male/shirt.glb'), representation(FEMALE, 'female/shirt.glb')]
    },
    metrics: { triangles: 800, materials: 2, textures: 2, meshes: 1, bodies: 1, entities: 1 },
    contents: { 'male/shirt.glb': male, 'female/shirt.glb': female, 'thumbnail.png': HASH.thumbnail },
    ...overrides
  })
}

export function emote(overrides: Partial<RemoteItemRow> = {}): RemoteItemRow {
  return wearable({
    name: 'Wave',
    type: 'emote',
    data: {
      category: 'greetings',
      loop: false,
      tags: [],
      representations: [representation(MALE, 'male/emote.glb'), representation(FEMALE, 'female/emote.glb')]
    },
    metrics: { sequences: 1, duration: 2, frames: 60, fps: 30, props: 0 },
    contents: { 'male/emote.glb': HASH.emote, 'female/emote.glb': HASH.emote, 'thumbnail.png': HASH.thumbnail },
    ...overrides
  })
}

/** Every rarity costs the same: 3.7 MANA, or 1 USD (10 credits), per item. */
const PRICES = { MANA: '3700000000000000000', USD: '1000000000000000000' }

export const RARITIES = [
  ['common', '100000'],
  ['uncommon', '10000'],
  ['rare', '5000'],
  ['epic', '1000'],
  ['legendary', '100'],
  ['exotic', '50'],
  ['mythic', '10'],
  ['unique', '1']
].map(([name, maxSupply]) => ({ id: name, name, price: PRICES.MANA, maxSupply, prices: PRICES }))

/** An item of a published collection: on chain with a token id, approved unless overridden. */
export function publishedItem(
  parent: RemoteCollectionRow,
  index: number,
  overrides: Partial<RemoteItemRow> = {}
): RemoteItemRow {
  const urn = `urn:decentraland:amoy:collections-v2:${parent.contract_address}:${index}`
  return wearable({
    collection_id: parent.id,
    blockchain_item_id: String(index),
    urn,
    rarity: 'common',
    is_published: true,
    is_approved: true,
    in_catalyst: true,
    ...overrides
  })
}

/** An item of a linked collection, mapped to any token of its contract. */
export function linkedItem(parent: RemoteCollectionRow, index: number, overrides: Partial<RemoteItemRow> = {}) {
  return wearable({
    name: `Linked Hat ${index}`,
    collection_id: parent.id,
    urn: `${parent.urn}:item-${index}`,
    rarity: null,
    mappings: { amoy: { [LINKED_CONTRACT]: [{ type: 'any' }] } },
    local_content_hash: `bafylinked${index}`,
    ...overrides
  })
}
