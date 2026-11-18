import { getCollectionContract } from '~/lib/chain/contracts'
import { type ContractCall } from '~/lib/auth'
import { type Collection } from '~/lib/collections'
import { isRefusedRequest } from '~/lib/builder'
import { getSystemMinters, isCollectionOwner } from '~/lib/collectionRoles'
import { SellItemError } from '~/lib/sales'

export type NewOwnerError = 'self' | 'zero' | 'system' | 'contract'

const ZERO_ADDRESS = '0x0000000000000000000000000000000000000000'

/** Why an address can't become the new owner, if it can't: a transfer to a contract can't be undone. */
export function getNewOwnerError(collection: Collection, address: string, chainId: number): NewOwnerError | null {
  const target = address.toLowerCase()
  if (isCollectionOwner(collection, target)) return 'self'
  if (target === ZERO_ADDRESS) return 'zero'
  if (getSystemMinters(chainId).has(target)) return 'system'
  if (target === collection.contractAddress?.toLowerCase()) return 'contract'
  return null
}

/** Whether the typed confirmation is the collection's name; surrounding spaces and case don't count. */
export function matchesCollectionName(collection: Collection, typed: string): boolean {
  return typed.trim().toLowerCase() === collection.name.trim().toLowerCase()
}

/** `transferCreatorship(address)` on the collection contract itself; the on-chain creator is the UI's owner. */
export function buildTransferOwnershipCall(collection: Collection, newOwner: string, chainId: number): ContractCall {
  if (!collection.contractAddress) throw new SellItemError('not_published', 'The collection has no contract yet')
  const contract = getCollectionContract(chainId, collection.contractAddress)
  return { contract, method: 'transferCreatorship', args: [newOwner] }
}

export type OwnerSyncDeps = {
  syncCollectionOwner: (collectionId: string) => Promise<Collection>
}

// Ten minutes of 5s polls: the subgraph usually indexes within a couple of minutes, and builder-server
// also picks the new owner up on their first write, so a longer wait buys little.
const OWNER_SYNC_DELAY_MS = 5000
const OWNER_SYNC_RETRIES = (10 * 60 * 1000) / OWNER_SYNC_DELAY_MS

/**
 * Asks builder-server to store the new owner until it reports it: the subgraph lags the transaction. A
 * failed request is retried as well, except a refusal (4xx), which no amount of waiting fixes.
 */
export async function waitForOwnerSync(
  collectionId: string,
  newOwner: string,
  deps: OwnerSyncDeps,
  retries = OWNER_SYNC_RETRIES,
  retryDelayMs = OWNER_SYNC_DELAY_MS
): Promise<Collection> {
  const owner = newOwner.toLowerCase()
  for (let attempt = 0; ; attempt++) {
    try {
      const collection = await deps.syncCollectionOwner(collectionId)
      if (collection.owner.toLowerCase() === owner) return collection
      if (attempt >= retries) throw new SellItemError('generic', 'The transfer was not indexed in time')
    } catch (error) {
      if (isRefusedRequest(error) || attempt >= retries) throw error
    }
    await new Promise(resolve => setTimeout(resolve, retryDelayMs))
  }
}
