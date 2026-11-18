// Re-points a linked collection to another NFT contract, in an order that stays safe to repeat: the server takes the
// contract first (it refuses one another collection already links), the registry is rewritten only when it does not
// list the contract yet, and every item is then moved under it. A run that stops halfway is finished by running again.
import { type ContractCall, type TransactionSettlement } from '~/lib/auth'
import { type Collection } from '~/lib/collections'
import { type Item } from '~/lib/items'
import { getItemMapping, relinkMappings } from '~/lib/mappings'
import { isSameContract, withContractReplaced, type LinkedContract, type ThirdParty } from '~/lib/thirdParty'
import { buildUpdateThirdPartyCall } from '~/lib/thirdPartyRegistry'

export type RelinkStep = 'collection' | 'signing' | 'pending' | 'items'

export type RelinkDeps = {
  saveCollection: (collection: Collection) => Promise<Collection>
  /** What the registry lists right now; only read once the third party is on chain. */
  fetchRegisteredContracts: () => Promise<LinkedContract[]>
  sendTransaction: (call: ContractCall) => Promise<string>
  settleTransaction: (txHash: string) => Promise<TransactionSettlement>
  fetchItems: () => Promise<Item[]>
  saveItem: (item: Item, patch: Pick<Item, 'mappings'>) => Promise<Item>
  onStep?: (step: RelinkStep) => void
  /** Asked once the wallet answered: a run the user backed out of stops there, leaving only the sent transaction. */
  isCancelled?: () => boolean
}

/** The user backed out while the wallet prompt was open; whatever was signed afterwards goes through on its own. */
export class RelinkCancelledError extends Error {
  constructor() {
    super('Contract change cancelled by the user')
    this.name = 'RelinkCancelledError'
  }
}

export type RelinkParams = {
  chainId: number
  collection: Collection
  thirdParty: ThirdParty
  /** The contract the collection had when the change was started, kept across retries. */
  from: LinkedContract | null
  to: LinkedContract
}

const FROM_KEY_PREFIX = 'linked-collection.relink-from.'

/** The contract a relink started from, kept across reloads so a later run still knows what to drop from the registry. */
export function loadRelinkFrom(collectionId: string): LinkedContract | null {
  try {
    const raw = localStorage.getItem(FROM_KEY_PREFIX + collectionId)
    const { network, address } = raw ? (JSON.parse(raw) as Partial<LinkedContract>) : {}
    return network && address ? { network, address } : null
  } catch {
    return null
  }
}

export function storeRelinkFrom(collectionId: string, from: LinkedContract | null): void {
  try {
    if (from) localStorage.setItem(FROM_KEY_PREFIX + collectionId, JSON.stringify(from))
    else localStorage.removeItem(FROM_KEY_PREFIX + collectionId)
  } catch {
    // Storage may be blocked; the run still works, only a later retry loses `from`.
  }
}

export type RelinkResult = {
  /** Items that point to the new contract once the run ends, those moved by an earlier run included. */
  linked: number
}

export async function relinkCollection(deps: RelinkDeps, params: RelinkParams): Promise<RelinkResult> {
  const { chainId, collection, thirdParty, from, to } = params

  const current =
    collection.linkedContractAddress && collection.linkedContractNetwork
      ? {
          address: collection.linkedContractAddress,
          network: collection.linkedContractNetwork as LinkedContract['network']
        }
      : null
  if (!current || !isSameContract(current, to)) {
    deps.onStep?.('collection')
    await deps.saveCollection({ ...collection, linkedContractAddress: to.address, linkedContractNetwork: to.network })
  }
  // Remembered only once the server took the contract: a refused run must not mark `from` for removal.
  if (from && !isSameContract(from, to)) storeRelinkFrom(collection.id, from)

  if (thirdParty.published) {
    const listed = await deps.fetchRegisteredContracts()
    // A record this code cannot read would be rewritten with `to` alone, dropping the sibling contracts.
    if (listed.length === 0 && thirdParty.contracts.length > 0) {
      throw new Error(`Registry record of ${thirdParty.id} could not be parsed`)
    }
    if (!listed.some(contract => isSameContract(contract, to))) {
      // Both the contract the change started from and the one the collection held now are being replaced: they
      // differ after a run that saved the collection but never reached the registry.
      const contracts = withContractReplaced(listed, [from, current], to)
      deps.onStep?.('signing')
      const txHash = await deps.sendTransaction(buildUpdateThirdPartyCall(chainId, { ...thirdParty, contracts }))
      if (deps.isCancelled?.()) throw new RelinkCancelledError()
      deps.onStep?.('pending')
      const outcome = await deps.settleTransaction(txHash)
      // A dropped or still-pending transaction may have been replaced by a speed-up that did the update.
      if (outcome !== 'mined') {
        const after = await deps.fetchRegisteredContracts()
        if (!after.some(contract => isSameContract(contract, to))) {
          throw new Error(`Third party update transaction ${txHash} ${outcome}`)
        }
      }
    }
  }

  deps.onStep?.('items')
  let linked = 0
  for (const item of await deps.fetchItems()) {
    const mappings = relinkMappings(item.mappings, to)
    if (mappings) await deps.saveItem(item, { mappings })
    else if (!getItemMapping(item.mappings, to)) continue
    linked++
  }
  storeRelinkFrom(collection.id, null)
  return { linked }
}
