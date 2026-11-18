import { ContractName, getCollectionContract, getContract } from '~/lib/chain/contracts'
import { type ContractCall } from '~/lib/auth'
import { type Collection } from '~/lib/collections'
import { SellItemError } from '~/lib/sales'

/** The two on-chain roles an owner hands out: senders are the contract's minters, collaborators its managers. */
export type RoleKind = 'senders' | 'collaborators'

const FIELD: Record<RoleKind, 'minters' | 'managers'> = { senders: 'minters', collaborators: 'managers' }
const METHOD: Record<RoleKind, 'setMinters' | 'setManagers'> = { senders: 'setMinters', collaborators: 'setManagers' }

export function isCollectionOwner(collection: Collection, address: string | undefined): boolean {
  return !!address && collection.owner.toLowerCase() === address.toLowerCase()
}

/**
 * Minters that belong to Decentraland's sale plumbing rather than to a person: every generation of the
 * off-chain marketplace (what "enable sales" adds here or in the Shop) and the legacy CollectionStore. They never show in the senders list, and
 * removing one would silently take the collection off sale, so the list keeps them untouched.
 */
export function getSystemMinters(chainId: number): Set<string> {
  let minters = systemMinters.get(chainId)
  if (!minters) {
    minters = new Set(
      [
        ContractName.OffChainMarketplace,
        ContractName.OffChainMarketplaceV2,
        ContractName.OffChainMarketplaceV3,
        ContractName.CollectionStore
      ].map(name => getContract(name, chainId).address.toLowerCase())
    )
    systemMinters.set(chainId, minters)
  }
  return minters
}

// Static per chain, and asked for on every keystroke of the address picker.
const systemMinters = new Map<number, Set<string>>()

/** The addresses currently holding the role, lowercased and de-duplicated, without the system minters. */
export function getRoleAddresses(collection: Collection, kind: RoleKind, chainId: number): string[] {
  const hidden = kind === 'senders' ? getSystemMinters(chainId) : new Set<string>()
  const seen = new Set<string>()
  const addresses: string[] = []
  for (const raw of collection[FIELD[kind]]) {
    const address = raw.toLowerCase()
    if (hidden.has(address) || seen.has(address)) continue
    seen.add(address)
    addresses.push(address)
  }
  return addresses
}

export type RoleAddressError = 'duplicate' | 'system'

/**
 * Why an address can't be added to the list, if it can't. The owner may list themselves, so the role survives a
 * transfer of ownership.
 */
export function getRoleAddressError(
  kind: RoleKind,
  current: string[],
  address: string,
  chainId: number
): RoleAddressError | null {
  const target = address.toLowerCase()
  if (current.includes(target)) return 'duplicate'
  if (kind === 'senders' && getSystemMinters(chainId).has(target)) return 'system'
  return null
}

export type RoleDiff = { addresses: string[]; values: boolean[] }

/** Only what changed goes on-chain: removed addresses with `false`, added ones with `true`. */
export function diffRoles(current: string[], next: string[]): RoleDiff {
  const before = new Set(current)
  const after = new Set(next)
  const addresses: string[] = []
  const values: boolean[] = []
  for (const address of current) {
    if (!after.has(address)) {
      addresses.push(address)
      values.push(false)
    }
  }
  for (const address of next) {
    if (!before.has(address)) {
      addresses.push(address)
      values.push(true)
    }
  }
  return { addresses, values }
}

/** `setMinters` / `setManagers(address[], bool[])` on the collection contract itself; owner-only on-chain. */
export function buildSetRolesCall(
  collection: Collection,
  kind: RoleKind,
  diff: RoleDiff,
  chainId: number
): ContractCall {
  if (!collection.contractAddress) throw new SellItemError('not_published', 'The collection has no contract yet')
  const contract = getCollectionContract(chainId, collection.contractAddress)
  return { contract, method: METHOD[kind], args: [diff.addresses, diff.values] }
}

/** The collection as builder-server will report it once the subgraph catches up with the role change. */
export function withRoles(collection: Collection, kind: RoleKind, next: string[], chainId: number): Collection {
  const field = FIELD[kind]
  const kept =
    kind === 'senders' ? collection.minters.filter(address => getSystemMinters(chainId).has(address.toLowerCase())) : []
  return { ...collection, [field]: [...kept, ...next] }
}
