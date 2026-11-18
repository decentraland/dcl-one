import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { errorCode, track } from '~/lib/analytics'
import {
  readTransactionSender,
  sendContractTransaction,
  settleTransaction,
  signTypedData,
  type Session
} from '~/lib/auth'
import {
  deleteVirtualThirdParty,
  fetchItemCurations,
  fetchThirdParty,
  fetchThirdPartySlots,
  publishLinkedItems as postLinkedItems,
  pushItemCuration,
  saveLinkedItemsTOS,
  setThirdPartyKind,
  updateItemCuration,
  type SlotsCheque
} from '~/lib/builder'
import { type Collection } from '~/lib/collections'
import { type Item } from '~/lib/items'
import { canPushLinkedItem, type ItemCuration } from '~/lib/linkedItemStatus'
import { fetchManaAllowance } from '~/lib/mana'
import { captureError } from '~/lib/monitoring'
import { getMaticChainId } from '~/lib/publishCollection'
import {
  LinkedPublishError,
  getLinkedSlots,
  publishLinkedItems,
  signSlotsCheque,
  type LinkedPayment,
  type LinkedPublishStep,
  type LinkedSlots,
  type PublishLinkedParams
} from '~/lib/publishLinkedItems'
import { pushLinkedChanges, type PushLinkedResult } from '~/lib/pushLinkedChanges'
import { fetchRegisteredThirdParty, fetchThirdPartyPrice } from '~/lib/thirdPartyRegistry'
import { isWalletRejection } from '~/lib/walletErrors'
import { invalidateCollectionItems } from './usePublishCollection'
import { thirdPartyKey, thirdPartySlotsKey } from './useLinkedCollection'

/** The registry's slot price and the programmatic pack, with the oracle's MANA rate. */
export function useThirdPartyPrice(enabled = true) {
  const chainId = getMaticChainId()
  return useQuery({
    queryKey: ['third-party-price', chainId],
    queryFn: () => fetchThirdPartyPrice(chainId),
    enabled,
    staleTime: 60_000
  })
}

async function fetchLinkedSlots(address: string, chainId: number, thirdPartyId: string): Promise<LinkedSlots> {
  const [available, indexed, registry] = await Promise.all([
    fetchThirdPartySlots(address, thirdPartyId),
    fetchThirdParty(address, thirdPartyId),
    fetchRegisteredThirdParty(chainId, thirdPartyId)
  ])
  return getLinkedSlots(available, Number(indexed.maxItems) || 0, registry)
}

/** The item slots the third party has free for the publish fee, read against the registry as well. */
export function useLinkedSlots(address: string | undefined, thirdPartyId: string | undefined) {
  const chainId = getMaticChainId()
  return useQuery({
    queryKey: thirdPartySlotsKey(address, thirdPartyId),
    queryFn: () => fetchLinkedSlots(address!, chainId, thirdPartyId!),
    enabled: !!address && !!thirdPartyId,
    staleTime: 30_000
  })
}

/** Signs the creator's EIP-712 slots cheque for `qty` items of the third party. */
export function useSignSlotsCheque(session: Session) {
  const chainId = getMaticChainId()
  return useMutation({
    // Dismissing the wallet prompt is the creator's choice; anything else blocks publishing.
    onError: error => {
      if (!isWalletRejection(error)) captureError(error, { flow: 'linked_publish', step: 'sign_cheque' })
    },
    mutationFn: ({ thirdPartyId, qty }: { thirdPartyId: string; qty: number }) =>
      signSlotsCheque(
        {
          signTypedData: (domain, types, primaryType, value) =>
            signTypedData(session, domain, types, primaryType, value)
        },
        chainId,
        thirdPartyId,
        qty
      )
  })
}

export type PublishLinkedVariables = Omit<PublishLinkedParams, 'address' | 'chainId'> & {
  onStep?: (step: LinkedPublishStep) => void
  onPayment?: (payment: LinkedPayment | undefined) => void
}

/** Runs the whole linked publish for the connected wallet: ToS, kind, fee on chain, server publish, pushes. */
export function usePublishLinkedItems(session: Session, collection: Collection) {
  const queryClient = useQueryClient()
  const chainId = getMaticChainId()
  const { address } = session
  const published = (variables: PublishLinkedVariables, txHash: string | null | undefined) => {
    track('Publish TP Items', {
      collectionId: collection.id,
      thirdPartyId: variables.thirdParty.id,
      items: variables.itemsToPublish.length,
      pushed: variables.itemsToPush.length,
      txHash: txHash ?? undefined
    })
    void queryClient.invalidateQueries({ queryKey: thirdPartyKey(address, variables.thirdParty.id) })
    void queryClient.invalidateQueries({ queryKey: thirdPartySlotsKey(address, variables.thirdParty.id) })
    void queryClient.invalidateQueries({ queryKey: ['collections'] })
    void queryClient.invalidateQueries({ queryKey: ['mana-balance', address] })
    invalidateCollectionItems(queryClient, collection.id)
  }
  return useMutation({
    mutationFn: ({ onStep, onPayment, ...params }: PublishLinkedVariables) =>
      publishLinkedItems(
        { ...params, address, chainId },
        {
          fetchCurations: () => fetchItemCurations(address, collection.id),
          saveTOS: (target, email, hashes) => saveLinkedItemsTOS(address, target, email, hashes),
          setThirdPartyKind: (thirdPartyId, isProgrammatic) => setThirdPartyKind(address, thirdPartyId, isProgrammatic),
          fetchManaAllowance: (owner, spender) => fetchManaAllowance(owner, spender, chainId),
          sendTransaction: call => sendContractTransaction(session, call),
          settleTransaction: (txHash, sender, onSeen) => settleTransaction(chainId, txHash, sender, onSeen),
          readSender: txHash => readTransactionSender(chainId, txHash),
          fetchSlots: () => fetchLinkedSlots(address, chainId, params.thirdParty.id),
          deleteVirtualThirdParty: thirdPartyId => deleteVirtualThirdParty(address, thirdPartyId),
          publishLinkedItems: (collectionId, itemIds, cheque: SlotsCheque) =>
            postLinkedItems(address, collectionId, itemIds, cheque),
          pushChanges: (items, fresh) => pushChanges(address, items, fresh),
          onStep,
          onPayment
        }
      ),
    onSuccess: (result, variables) => {
      if (result.collection) queryClient.setQueryData(['collection', address, collection.id], result.collection)
      published(variables, result.txHash)
    },
    onError: (error, variables) => {
      const step = error instanceof LinkedPublishError ? error.step : 'unknown'
      track('Publish TP Items error', { collectionId: collection.id, step, error: errorCode(error) })
      // From the payment on, the creator may have paid: a failure there needs someone to look, a payment that never
      // confirms included. A rejected wallet prompt or a reverted transaction charged nothing.
      if (
        error instanceof LinkedPublishError &&
        ['wait_tx', 'delete_virtual', 'publish', 'push'].includes(error.step) &&
        error.reason !== 'rejected' &&
        error.reason !== 'reverted'
      ) {
        captureError(error.cause ?? error, {
          flow: 'linked_publish',
          step: error.step,
          collectionId: collection.id,
          txHash: error.txHash
        })
      }
      // Only some changes failed to push: the publish itself went through.
      if (error instanceof LinkedPublishError && error.reason === 'push_failed') published(variables, error.txHash)
      // Reads what the server managed to write before failing.
      invalidateCollectionItems(queryClient, collection.id)
      void queryClient.invalidateQueries({ queryKey: thirdPartyKey(address, variables.thirdParty.id) })
      void queryClient.invalidateQueries({ queryKey: thirdPartySlotsKey(address, variables.thirdParty.id) })
    }
  })
}

function pushChanges(
  address: string,
  items: Item[],
  curations: Map<string, ItemCuration>,
  onProgress?: (done: number, total: number) => void
): Promise<PushLinkedResult> {
  return pushLinkedChanges(
    items,
    curations,
    {
      pushItemCuration: itemId => pushItemCuration(address, itemId),
      updateItemCuration: (itemId, status) => updateItemCuration(address, itemId, status)
    },
    onProgress
  )
}

export type PushLinkedVariables = {
  items: Item[]
  email: string
  onProgress?: (done: number, total: number) => void
}

/** Sends already-approved linked items' changes for review again: ToS record, then one request per item. */
export function usePushLinkedChanges(session: Session, collection: Collection) {
  const queryClient = useQueryClient()
  const { address } = session
  return useMutation({
    mutationFn: async ({ items, email, onProgress }: PushLinkedVariables) => {
      await saveLinkedItemsTOS(
        address,
        collection,
        email,
        items.flatMap(item => (item.currentContentHash ? [item.currentContentHash] : []))
      )
      // Read now, not when the dialog opened: a retry must neither resend what went through nor misread its state.
      const curations = await fetchItemCurations(address, collection.id)
      const toPush = items.filter(item => canPushLinkedItem(item, curations.get(item.id) ?? null))
      return pushChanges(address, toPush, curations, onProgress)
    },
    onSuccess: (result, { items }) => {
      track('Push TP Changes', { collectionId: collection.id, items: items.length, failed: result.failed.length })
      if (result.failed.length > 0) {
        captureError(new Error(`${result.failed.length} linked items could not be pushed`), {
          flow: 'linked_push',
          collectionId: collection.id,
          failures: result.failed.map(failure => `${failure.item.id}: ${failure.message}`)
        })
      }
      invalidateCollectionItems(queryClient, collection.id)
    },
    onError: (error, { items }) => {
      track('Push TP Changes error', { collectionId: collection.id, items: items.length, error: errorCode(error) })
      captureError(error, { flow: 'linked_push', collectionId: collection.id })
      invalidateCollectionItems(queryClient, collection.id)
    }
  })
}
