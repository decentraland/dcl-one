import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { type TradeCreation } from '@dcl/schemas'
import {
  readContract,
  sendContractTransaction,
  signTypedData,
  waitForTransaction,
  type ContractCall,
  type Session
} from '~/lib/auth'
import { errorCode, track } from '~/lib/analytics'
import { type Collection } from '~/lib/collections'
import { allCollectionItemsKey } from '~/hooks/useCollection'
import { fetchFriends } from '~/lib/friends'
import { type Item } from '~/lib/items'
import { fetchItemTradeId, type ItemListing } from '~/lib/listings'
import { fetchManaUsdRate } from '~/lib/manaRate'
import { buildIssueTokensCall, copiesPerItem, flattenTransfers, sendBatchSize, type Transfer } from '~/lib/mint'
import { captureError } from '~/lib/monitoring'
import { isExpectedWalletError } from '~/lib/walletErrors'
import { getMaticChainId } from '~/lib/publishCollection'
import {
  buildEnableSalesCall,
  isSalesEnabled,
  removeListing,
  removeStoreListing,
  sellItem,
  SellItemError,
  updatePrice,
  withSalesEnabled,
  type ListingTerms,
  type PricedSale,
  type SalePrice
} from '~/lib/sales'
import { createTrade, fetchSignatureIndexes, fetchTrade, tradeSigner } from '~/lib/trades'

// Collections whose sales this tab enabled: builder-server reports minters from the subgraph, which
// lags the transaction by a while, so a refetch in between must not bring the Enable Sales step back.
// Never evicted: this app cannot remove the minter, so only a full reload picks up an external removal.
const enabledInSession = new Set<string>()

export function useSalesEnabled(collection: Collection): boolean {
  return enabledInSession.has(collection.id) || isSalesEnabled(collection, getMaticChainId())
}

export type EnableSalesVariables = {
  collection: Collection
  /** The wallet prompt is over; the transaction is mining. */
  onSigned?: () => void
}

/** Makes the off-chain marketplace a minter of the collection and waits until the change is mined. */
export function useEnableSales(session: Session | null) {
  const queryClient = useQueryClient()
  const chainId = getMaticChainId()
  return useMutation({
    mutationFn: async ({ collection, onSigned }: EnableSalesVariables): Promise<Collection> => {
      if (!session) throw new Error('Wallet disconnected')
      const txHash = await sendContractTransaction(session, buildEnableSalesCall(collection, chainId))
      onSigned?.()
      const mined = await waitForTransaction(chainId, txHash)
      if (!mined) throw new SellItemError('generic', 'The enable sales transaction reverted')
      return withSalesEnabled(collection, chainId)
    },
    onSuccess: collection => {
      track('Enable sales', { collectionId: collection.id })
      enabledInSession.add(collection.id)
      queryClient.setQueryData(['collection', session?.address, collection.id], collection)
    },
    onError: (error, { collection }) =>
      track('Enable sales error', { collectionId: collection.id, error: errorCode(error) })
  })
}

export type SellItemVariables = {
  collection: Collection
  item: Item
  price: SalePrice
  beneficiary: string
  expiresAt: number
  /** The wallet prompt is over; the order is being stored. */
  onSigned?: () => void
}

// The wallet-facing half of signing and storing an order, shared by selling and re-pricing.
function orderDeps(session: Session, chainId: number) {
  return {
    fetchTrade,
    fetchItemTradeId,
    fetchSignatureIndexes,
    sendTransaction: (call: ContractCall) => sendContractTransaction(session, call),
    waitForTransaction: (hash: string) => waitForTransaction(chainId, hash),
    signTrade: tradeSigner(
      (domain, types, primaryType, value) => signTypedData(session, domain, types, primaryType, value),
      chainId
    ),
    createTrade: (trade: TradeCreation) => createTrade(session.address, trade)
  }
}

/** Signs the item's primary order and stores it; the collection's listings pick it up right away. */
export function useSellItem(session: Session | null) {
  const queryClient = useQueryClient()
  const chainId = getMaticChainId()
  return useMutation({
    mutationFn: async ({ onSigned, ...variables }: SellItemVariables): Promise<ItemListing> => {
      if (!session) throw new Error('Wallet disconnected')
      return sellItem({ ...variables, address: session.address, chainId }, { ...orderDeps(session, chainId), onSigned })
    },
    onSuccess: (listing, { collection, item, price }) => {
      track(LISTING_EVENT, { ...listingProps(collection, item, price), is_update: false })
      setListing(queryClient, collection, listing)
    },
    onError: (error, { collection, item, price }) =>
      track(LISTING_FAILURE_EVENT, {
        ...listingProps(collection, item, price),
        is_update: false,
        error: errorCode(error)
      })
  })
}

// The legacy builder's put-on-sale event, reused so old and new UI are comparable; the mechanism differs
// (off-chain trades here, an on-chain price + beneficiary there), so the props are ours.
const LISTING_EVENT = 'Set price and beneficiary'
const LISTING_FAILURE_EVENT = 'Set price and beneficiary failure'

function listingProps(collection: Collection, item: Item, price: SalePrice) {
  return {
    collectionId: collection.id,
    itemId: item.id,
    price_kind: price.kind,
    is_giveaway: price.kind === 'free'
  }
}

function setListing(queryClient: ReturnType<typeof useQueryClient>, collection: Collection, listing: ItemListing) {
  queryClient.setQueryData<Map<string, ItemListing>>(['collection-listings', collection.contractAddress], current =>
    new Map(current ?? []).set(listing.itemId, listing)
  )
}

function removeListingFromCache(
  queryClient: ReturnType<typeof useQueryClient>,
  collection: Collection,
  itemId: string
) {
  queryClient.setQueryData<Map<string, ItemListing>>(['collection-listings', collection.contractAddress], current => {
    const next = new Map(current ?? [])
    next.delete(itemId)
    return next
  })
}

export type RemoveListingVariables = {
  collection: Collection
  item: Item
  listing: ItemListing
  /** The wallet prompt is over; the cancellation is mining. */
  onSigned?: () => void
}

/**
 * Takes the item off sale and waits until it is mined; the row drops its price right away. An off-chain
 * order gets its signature cancelled; a legacy CollectionStore price gets cleared on the collection.
 */
export function useRemoveListing(session: Session | null) {
  const queryClient = useQueryClient()
  const chainId = getMaticChainId()
  return useMutation({
    mutationFn: async ({ collection, item, listing, onSigned }: RemoveListingVariables): Promise<void> => {
      if (!session) throw new Error('Wallet disconnected')
      if (!collection.contractAddress) throw new SellItemError('not_published', 'The collection has no contract')
      const deps = { ...orderDeps(session, chainId), onSigned }
      if (!listing.tradeId) return removeStoreListing(collection, item, chainId, { ...deps, readContract })
      return removeListing(
        { tradeId: listing.tradeId, contractAddress: collection.contractAddress, itemId: listing.itemId },
        deps
      )
    },
    onSuccess: (_, { collection, item, listing }) => {
      track('Remove item listing', { collectionId: collection.id, itemId: item.id })
      removeListingFromCache(queryClient, collection, listing.itemId)
    },
    onError: (error, { collection, item }) =>
      track('Remove item listing error', { collectionId: collection.id, itemId: item.id, error: errorCode(error) })
  })
}

export type UpdatePriceVariables = {
  collection: Collection
  item: Item
  tradeId: string
  price: PricedSale
  onSigned?: (step: 'cancel' | 'sign') => void
  onCancelled?: (terms: ListingTerms) => void
}

/** Cancels the current order and signs a new one at the given price, keeping beneficiary and expiration. */
export function useUpdatePrice(session: Session | null) {
  const queryClient = useQueryClient()
  const chainId = getMaticChainId()
  return useMutation({
    mutationFn: async ({ onSigned, onCancelled, ...variables }: UpdatePriceVariables): Promise<ItemListing> => {
      if (!session) throw new Error('Wallet disconnected')
      return updatePrice(
        { ...variables, address: session.address, chainId },
        {
          ...orderDeps(session, chainId),
          onSigned,
          onCancelled: terms => {
            // The old order is gone even if the new one never lands.
            removeListingFromCache(queryClient, variables.collection, variables.item.tokenId!)
            onCancelled?.(terms)
          }
        }
      )
    },
    onSuccess: (listing, { collection, item, price }) => {
      track(LISTING_EVENT, { ...listingProps(collection, item, price), is_update: true })
      setListing(queryClient, collection, listing)
    },
    onError: (error, { collection, item, price }) =>
      track(LISTING_FAILURE_EVENT, {
        ...listingProps(collection, item, price),
        is_update: true,
        error: errorCode(error)
      })
  })
}

export type SendItemsVariables = {
  collection: Collection
  items: Item[]
  transfers: Transfer[]
  /** The wallet prompt is over; the transaction is mining. */
  onSigned?: () => void
}

/**
 * Mints the transfers straight to their recipients and waits until mined. builder-server reads
 * `total_supply` from the subgraph, which lags the transaction, so the items are patched in the cache
 * rather than refetched.
 */
export function useSendItems(session: Session | null) {
  const queryClient = useQueryClient()
  const chainId = getMaticChainId()
  return useMutation({
    mutationFn: async ({ collection, items, transfers, onSigned }: SendItemsVariables): Promise<void> => {
      if (!session) throw new Error('Wallet disconnected')
      const { beneficiaries, tokenIds } = flattenTransfers(transfers, items)
      const txHash = await sendContractTransaction(
        session,
        buildIssueTokensCall(collection, beneficiaries, tokenIds, chainId)
      )
      onSigned?.()
      const mined = await waitForTransaction(chainId, txHash)
      if (!mined) throw new SellItemError('generic', 'The send items transaction reverted')
    },
    onSuccess: (_, { collection, transfers }) => {
      const copies = copiesPerItem(transfers)
      track('Mint items', { collectionId: collection.id, ...sendBatchSize(transfers) })
      queryClient.setQueryData<Item[]>(allCollectionItemsKey(session?.address, collection.id), current =>
        current?.map(item =>
          copies[item.id] ? { ...item, totalSupply: (item.totalSupply ?? 0) + copies[item.id] } : item
        )
      )
    },
    onError: (error, { collection, transfers }) => {
      const size = sendBatchSize(transfers)
      const code = errorCode(error)
      track('Mint items error', { collectionId: collection.id, error: code, ...size })
      // A wallet cancel or a wallet on the wrong account is the creator's; anything else (revert, relayer refusal,
      // RPC) is ours to see.
      if (!isExpectedWalletError(error))
        captureError(error, { flow: 'send_items', collectionId: collection.id, ...size })
    }
  })
}

/** The MANA/USD rate behind the estimate next to a MANA price; `undefined` while loading or when the oracle can't be read. */
export function useManaUsdRate(enabled: boolean) {
  const chainId = getMaticChainId()
  return useQuery({
    queryKey: ['mana-usd-rate', chainId],
    queryFn: () => fetchManaUsdRate(chainId),
    enabled,
    staleTime: 5 * 60_000,
    retry: false
  })
}

/** The creator's friends, loaded once the beneficiary picker needs them. */
export function useFriends(session: Session | null, enabled: boolean) {
  return useQuery({
    queryKey: ['friends', session?.address],
    queryFn: () => fetchFriends(session!.identity),
    enabled: !!session && enabled,
    staleTime: 5 * 60_000
  })
}
