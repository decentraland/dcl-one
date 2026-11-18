import { useMutation, useQueryClient } from '@tanstack/react-query'
import { errorCode, track } from '~/lib/analytics'
import { sendContractTransaction, waitForTransaction, type Session } from '~/lib/auth'
import { type Collection } from '~/lib/collections'
import { syncCollectionOwner } from '~/lib/builder'
import { buildTransferOwnershipCall, waitForOwnerSync } from '~/lib/collectionOwnership'
import { captureError } from '~/lib/monitoring'
import { getMaticChainId } from '~/lib/publishCollection'
import { SellItemError } from '~/lib/sales'

export type TransferOwnershipVariables = {
  collection: Collection
  newOwner: string
  /** The wallet prompt is over; the transaction is mining. */
  onSigned?: () => void
}

// A sped-up or replaced transaction gets a new hash this one never sees mined, so the wait gives up.
const MINING_TIMEOUT_MS = 5 * 60_000

/** Hands the collection to another wallet in one transaction and waits until it is mined. */
export function useTransferCollectionOwnership(session: Session | null) {
  const queryClient = useQueryClient()
  const chainId = getMaticChainId()
  return useMutation({
    mutationKey: ['transfer-collection-ownership'],
    mutationFn: async ({ collection, newOwner, onSigned }: TransferOwnershipVariables): Promise<Collection> => {
      if (!session) throw new Error('Wallet disconnected')
      const call = buildTransferOwnershipCall(collection, newOwner, chainId)
      const txHash = await sendContractTransaction(session, call)
      onSigned?.()
      const mined = await waitForTransaction(chainId, txHash, MINING_TIMEOUT_MS)
      if (!mined) throw new SellItemError('generic', 'The transferCreatorship transaction reverted')
      return { ...collection, owner: newOwner.toLowerCase() }
    },
    onSuccess: collection => {
      track('Transfer collection ownership', { collectionId: collection.id, newOwner: collection.owner })
      if (!session) return
      const address = session.address
      queryClient.setQueryData(['collection', address, collection.id], collection)
      void queryClient.invalidateQueries({ queryKey: ['collections'] })
      // Detached on purpose, like the publish sync: the dialog moves on and the server catches up on its own.
      // The transfer is done either way, and the server also picks the new owner up on their first write.
      waitForOwnerSync(collection.id, collection.owner, {
        syncCollectionOwner: id => syncCollectionOwner(address, id)
      })
        .then(synced => queryClient.setQueryData(['collection', address, collection.id], synced))
        .catch((error: unknown) => captureError(error, { flow: 'transfer_ownership', collectionId: collection.id }))
        .finally(() => void queryClient.invalidateQueries({ queryKey: ['collections'] }))
    },
    onError: (error, { collection }) =>
      track('Transfer collection ownership error', { collectionId: collection.id, error: errorCode(error) })
  })
}
