// On-chain committee writes on a standard collection, ported from the legacy builder's
// changeCollectionStatus / handleRescueItemsRequest: every call goes through
// Committee.manageCollection so the collection sees the committee as the caller.
import {
  ContractName,
  encodeContractCall,
  getCollectionContract,
  getContract,
  type ContractCall
} from '~/lib/chain/contracts'
import { type Collection } from '~/lib/collections'

// Same chunk size as the legacy rescue, which keeps each transaction under the block gas limit.
export const RESCUE_CHUNK_SIZE = 50

function collectionContract(chainId: number, collection: Collection) {
  if (!collection.contractAddress) throw new Error(`Collection ${collection.id} has no contract`)
  return getCollectionContract(chainId, collection.contractAddress)
}

function committeeCall(chainId: number, collection: Collection, inner: ContractCall): ContractCall {
  return {
    contract: getContract(ContractName.Committee, chainId),
    method: 'manageCollection',
    args: [
      getContract(ContractName.CollectionManager, chainId).address,
      getContract(ContractName.Forwarder, chainId).address,
      collection.contractAddress,
      [encodeContractCall(inner)]
    ]
  }
}

/** Approves (mintable) or disables the collection on chain. */
export function buildSetApprovedCall(chainId: number, collection: Collection, approved: boolean): ContractCall {
  const inner = { contract: collectionContract(chainId, collection), method: 'setApproved', args: [approved] }
  return committeeCall(chainId, collection, inner)
}

export type RescueEntry = { tokenId: string; contentHash: string; metadata: string }

/** Rewrites the on-chain content hash and metadata of published items, one transaction per chunk. */
export function buildRescueItemsCalls(chainId: number, collection: Collection, entries: RescueEntry[]): ContractCall[] {
  const calls: ContractCall[] = []
  for (let start = 0; start < entries.length; start += RESCUE_CHUNK_SIZE) {
    const chunk = entries.slice(start, start + RESCUE_CHUNK_SIZE)
    const inner = {
      contract: collectionContract(chainId, collection),
      method: 'rescueItems',
      args: [
        chunk.map(entry => entry.tokenId),
        chunk.map(entry => entry.contentHash),
        chunk.map(entry => entry.metadata)
      ]
    }
    calls.push(committeeCall(chainId, collection, inner))
  }
  return calls
}
