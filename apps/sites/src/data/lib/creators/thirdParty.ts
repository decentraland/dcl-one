// Third parties and their linked collections, ported from the legacy builder (modules/thirdParty + lib/urn):
// a linked collection belongs to a third party (one per collection, created on first save), is tied to one
// external NFT contract, and its items are unlocked by tokens of that contract (see lib/mappings).
import { ContractNetwork } from '@dcl/schemas'
import { config } from './config'
import type { Collection } from './collections'

export const THIRD_PARTY_URN_TYPE = 'collections-thirdparty'

/** The linked wearables guide, under DOCS_URL. */
export const LINKED_WEARABLES_DOCS_PATH = '/creator/wearables/linked-wearables/'

export type LinkedContract = { network: ContractNetwork; address: string }

/** builder-server's third party: the on-chain registry record, or the "virtual" row kept until the first publish. */
export type ThirdParty = {
  id: string
  /** Merkle root of the approved items; empty until the first approval. */
  root: string
  managers: string[]
  name: string
  description: string
  /** Whether the registry has the third party enabled (curators can disable it). */
  isApproved: boolean
  /** A flat fee for a pack of slots instead of a fee per item; chosen once, before the first publish. */
  isProgrammatic: boolean
  /** Item slots bought so far, as the registry counts them; '0' while virtual. */
  maxItems: string
  contracts: LinkedContract[]
  /** False while the third party only exists in builder-server (not yet registered on chain). */
  published: boolean
}

/** URL-safe, lowercase slug (legacy used the `slug` package; same output for ASCII names). */
export function slugify(text: string): string {
  return text
    .normalize('NFKD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
}

/** Legacy getDefaultThirdPartyUrnSuffix: the slugged name plus four random hex characters. */
export function getDefaultUrnSuffix(name: string): string {
  const random = Array.from({ length: 4 }, () => Math.floor(Math.random() * 16).toString(16)).join('')
  return `${slugify(name) || 'default'}-${random}`
}

export function buildThirdPartyId(slug: string): string {
  return `urn:decentraland:${config.get('MATIC_URN_PROTOCOL')}:${THIRD_PARTY_URN_TYPE}:${slug}`
}

export function buildLinkedCollectionUrn(thirdPartyId: string, collectionSuffix: string): string {
  return `${thirdPartyId}:${collectionSuffix}`
}

export function buildLinkedItemUrn(collectionUrn: string, itemSuffix: string): string {
  return `${collectionUrn}:${itemSuffix}`
}

export type DecodedLinkedUrn = {
  thirdPartyId: string
  collectionSuffix: string
  itemSuffix?: string
}

/** The parts of a linked collection or item URN, or null for any other URN. */
export function decodeLinkedUrn(urn: string | undefined): DecodedLinkedUrn | null {
  if (!urn) return null
  const parts = urn.split(':')
  if (parts.length < 6 || parts.length > 7 || parts[3] !== THIRD_PARTY_URN_TYPE) return null
  if (parts.some(part => part.length === 0)) return null
  return {
    thirdPartyId: parts.slice(0, 5).join(':'),
    collectionSuffix: parts[5],
    ...(parts.length === 7 ? { itemSuffix: parts[6] } : {})
  }
}

/** The third party's id is its collection URN without the collection suffix. */
export function getThirdPartyId(collection: Pick<Collection, 'urn'>): string | undefined {
  return decodeLinkedUrn(collection.urn)?.thirdPartyId
}

// URN segments may not contain ':' nor whitespace; builder-server's matcher is `[^:|\s]+`.
const URN_SEGMENT = /^[^:|\s]+$/

export function isValidUrnSuffix(suffix: string): boolean {
  return URN_SEGMENT.test(suffix)
}

/**
 * Linked collections belong to a third party, not a wallet (builder-server leaves their owner empty):
 * its managers are who may edit them, the same check builder-server makes.
 */
export function isThirdPartyManager(thirdParty: Pick<ThirdParty, 'managers'>, address: string | undefined): boolean {
  if (!address) return false
  return thirdParty.managers.some(manager => manager.toLowerCase() === address.toLowerCase())
}

/**
 * Whether the item editor opens this linked collection: like standard ones, once published (one submitted item) any
 * signer may view it read-only there; before that, only the third party's managers. The detail page is managers-only.
 */
export function canViewLinkedCollection(
  collection: Pick<Collection, 'isPublished'>,
  thirdParty: Pick<ThirdParty, 'managers'>,
  address: string | undefined
): boolean {
  return !!address && (collection.isPublished || isThirdPartyManager(thirdParty, address))
}

export function isSameContract(a: LinkedContract, b: LinkedContract): boolean {
  return a.network === b.network && a.address.toLowerCase() === b.address.toLowerCase()
}

/** `contracts` without the `replaced` ones and with `to` listed once. */
export function withContractReplaced(
  contracts: LinkedContract[],
  replaced: (LinkedContract | null)[],
  to: LinkedContract
): LinkedContract[] {
  const kept = contracts.filter(
    contract => !replaced.some(old => old && !isSameContract(old, to) && isSameContract(contract, old))
  )
  return kept.some(contract => isSameContract(contract, to)) ? kept : [...kept, to]
}

export const REGISTRY_NETWORKS = new Set<string>(Object.values(ContractNetwork))

/** The contracts of a raw registry metadata string (builder-server's parseRawMetadata, contracts only). */
export function contractsFromRawMetadata(raw: string | undefined): LinkedContract[] {
  // Contracts are the last segment and never contain ':', so a ':' inside a legacy description is harmless.
  const parts = (raw ?? '').split(':')
  if (parts.length < 5) return []
  const entries = parts[parts.length - 1].split(';')
  const contracts = entries.flatMap(entry => {
    const [network, address] = entry.split('-')
    return network && address && REGISTRY_NETWORKS.has(network) && /^0x[0-9a-fA-F]{40}$/.test(address)
      ? [{ network: network as ContractNetwork, address: address.toLowerCase() }]
      : []
  })
  // No readable entry is description text on a record without contracts. A partly readable segment is a record this
  // code must not rewrite, or the unreadable sibling would be dropped from the chain.
  if (contracts.length > 0 && contracts.length < entries.length) {
    throw new Error('Registry record lists a contract entry that could not be parsed')
  }
  return contracts
}

/** Legacy convertThirdPartyMetadataToRawMetadata: what `addThirdParties` stores on chain. */
export function toRawMetadata(thirdParty: Pick<ThirdParty, 'name' | 'description' | 'contracts'>): string {
  const contracts = thirdParty.contracts.map(contract => `${contract.network}-${contract.address}`).join(';')
  return `tp:1:${thirdParty.name}:${thirdParty.description}${contracts ? `:${contracts}` : ''}`
}

// The linked contract's networks the legacy create modal offers (the ones with a public RPC to check the contract on).
export const LINKED_CONTRACT_CHAIN_IDS: Partial<Record<ContractNetwork, number>> = {
  mainnet: 1,
  matic: 137,
  sepolia: 11155111,
  amoy: 80002
}

const TESTNETS = new Set(['sepolia', 'amoy'])

/** Networks a new linked collection may pick; testnets only outside production. */
export function getLinkedContractNetworks(isProduction: boolean): ContractNetwork[] {
  return (Object.keys(LINKED_CONTRACT_CHAIN_IDS) as ContractNetwork[]).filter(
    network => !isProduction || !TESTNETS.has(network)
  )
}

export const LINKED_COLLECTION_NAME_MAX_LENGTH = 42

export type LinkedCollectionNameError = 'empty' | 'too_long' | 'invalid_character'

/** Same rules as the legacy third-party modal: non-empty, ≤42 chars, no ':' (names feed URNs). */
export function validateLinkedCollectionName(name: string): LinkedCollectionNameError | null {
  const trimmed = name.trim()
  if (!trimmed) return 'empty'
  if (trimmed.length > LINKED_COLLECTION_NAME_MAX_LENGTH) return 'too_long'
  if (trimmed.includes(':')) return 'invalid_character'
  return null
}

/**
 * A brand-new linked collection, as the legacy CreateThirdPartyCollectionModal builds it: its own third party,
 * named after the collection, which builder-server creates as a virtual record on the first save.
 */
export function buildNewLinkedCollection(
  name: string,
  owner: string,
  contract: LinkedContract,
  thirdPartySlug = getDefaultUrnSuffix(name)
): Collection {
  const now = Date.now()
  const id = crypto.randomUUID()
  return {
    id,
    name,
    owner,
    urn: buildLinkedCollectionUrn(buildThirdPartyId(thirdPartySlug), id),
    isPublished: false,
    isApproved: false,
    itemCount: 0,
    minters: [],
    managers: [],
    linkedContractAddress: contract.address.toLowerCase(),
    linkedContractNetwork: contract.network,
    createdAt: now,
    updatedAt: now
  }
}

/** The collection with a new URN suffix; its items' URNs follow the collection's on the server. */
export function withCollectionUrnSuffix(collection: Collection, suffix: string): Collection {
  const decoded = decodeLinkedUrn(collection.urn)
  if (!decoded) throw new Error(`Collection ${collection.id} is not a linked collection`)
  return { ...collection, urn: buildLinkedCollectionUrn(decoded.thirdPartyId, suffix) }
}

// The exact error data builder-server answers when the linked contract already belongs to another collection.
export const LINKED_CONTRACT_IN_USE_ERROR = 'linkedContract_linkedNetwork_thirdPartyId_unique'
