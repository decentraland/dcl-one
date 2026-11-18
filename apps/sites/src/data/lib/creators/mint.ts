import { getCollectionContract } from '~/lib/chain/contracts'
import { type ContractCall } from '~/lib/auth'
import { hasBeenApproved, type Collection } from '~/lib/collections'
import { getItemSales, type Item } from '~/lib/items'
import { SellItemError, isValidAddress } from '~/lib/sales'

// Bor caps a transaction at 2^25 gas (EIP-7825 as of the Madhugiri fork, live on Polygon and Amoy).
// issueTokens costs ~157k gas per copy to never-held recipients (estimated on Amoy, 2026-10): 100 copies
// is ~15.7M, under half the cap even with a wallet padding the estimate. The legacy builder caps at 50.
export const MAX_ITEMS_PER_SEND = 100

/** One bundle: every recipient gets `amounts[itemId]` copies of each item. */
export type Transfer = {
  recipients: string[]
  amounts: Record<string, number>
}

/** A transfer being edited: `key` keeps each card's React state with its own transfer when one is removed. */
export type TransferDraft = Transfer & { key: number }

let nextTransferKey = 0

export function createTransfer(): TransferDraft {
  return { key: nextTransferKey++, recipients: [], amounts: {} }
}

/** Only the owner and minters may call issueTokens; collaborators would get a reverted transaction. */
export function canSendCollectionItems(collection: Collection, address: string | undefined): boolean {
  if (!address || !hasBeenApproved(collection)) return false
  const target = address.toLowerCase()
  return collection.owner.toLowerCase() === target || collection.minters.some(m => m.toLowerCase() === target)
}

export type RecipientList = { added: string[]; invalid: number; repeated: number; cut: number }

// A `0x` hex run on its own, whatever its length: a mistyped address still counts as one that didn't make it.
// (No lookbehind: Safari before 16.4 can't parse one, and a regex it can't parse breaks the whole module.)
const HEX_TOKEN = /(?:^|[^0-9a-zA-Z])(0x[0-9a-fA-F]+)(?![0-9a-zA-Z])/g

function hexTokens(text: string): string[] {
  return [...text.matchAll(HEX_TOKEN)].map(match => match[1])
}

/** Whether the text carries at least one address, as a pasted list does. */
export function holdsAddresses(text: string): boolean {
  return hexTokens(text).some(token => token.length === 42)
}

/**
 * The wallets in a pasted list (a spreadsheet column, comma or line separated, whatever else is around them):
 * every address token, lowercased and deduped. `invalid` counts the `0x` tokens that aren't an address (wrong
 * length or checksum), `repeated` the repeats, including those already in `existing`, and `cut` the valid
 * ones left out to keep `existing` plus `added` within `max`.
 */
export function parseRecipientList(text: string, existing: string[] = [], max = MAX_ITEMS_PER_SEND): RecipientList {
  const seen = new Set(existing.map(address => address.toLowerCase()))
  const added: string[] = []
  let invalid = 0
  let repeated = 0
  for (const token of hexTokens(text)) {
    const address = token.toLowerCase()
    if (!isValidAddress(token)) invalid++
    else if (seen.has(address)) repeated++
    else {
      seen.add(address)
      added.push(address)
    }
  }
  const room = Math.max(0, max - existing.length)
  return { added: added.slice(0, room), invalid, repeated, cut: Math.max(0, added.length - room) }
}

/** Published and approved on chain: sold-out items are listed disabled, so they stay sendable here. */
export function isSendableItem(item: Item): boolean {
  return item.isPublished && item.isApproved && !!item.tokenId && getItemSales(item) !== undefined
}

/**
 * Copies not yet minted, before this dialog's allocations. Read from the item as loaded with the page:
 * a mint landing in the meantime makes the contract revert, which the flow reports as a generic error.
 */
export function getStock(item: Item): { available: number; total: number } {
  const sales = getItemSales(item)
  if (!sales) return { available: 0, total: 0 }
  return { available: sales.maxSupply - sales.minted, total: sales.maxSupply }
}

/** Something was entered: a recipient or a copy in any transfer. Closing the dialog then asks first. */
export function hasSendDraft(transfers: Transfer[]): boolean {
  return transfers.some(
    ({ recipients, amounts }) => recipients.length > 0 || Object.values(amounts).some(amount => amount > 0)
  )
}

export function totalCopies(transfers: Transfer[]): number {
  return transfers.reduce(
    (sum, { recipients, amounts }) => sum + recipients.length * Object.values(amounts).reduce((a, b) => a + b, 0),
    0
  )
}

/** Copies of `itemId` claimed by every transfer other than `except`. */
export function allocatedElsewhere(transfers: Transfer[], itemId: string, except?: number): number {
  return transfers.reduce(
    (sum, transfer, index) =>
      index === except ? sum : sum + transfer.recipients.length * (transfer.amounts[itemId] ?? 0),
    0
  )
}

/** Most copies per recipient a transfer may still set for an item (a recipient-less transfer counts as one). */
export function maxAmount(item: Item, transfers: Transfer[], index: number): number {
  const remaining = getStock(item).available - allocatedElsewhere(transfers, item.id, index)
  return Math.max(0, Math.floor(remaining / Math.max(1, transfers[index].recipients.length)))
}

export type RecipientBundle = { address: string; amounts: Record<string, number> }

/** What each wallet ends up with once every transfer is merged: an address in two transfers gets both bundles. */
export function copiesPerRecipient(transfers: Transfer[]): RecipientBundle[] {
  const bundles = new Map<string, Record<string, number>>()
  for (const transfer of transfers) {
    const chosen = Object.entries(transfer.amounts).filter(([, amount]) => amount > 0)
    if (chosen.length === 0) continue
    for (const address of transfer.recipients) {
      const amounts = bundles.get(address) ?? {}
      for (const [itemId, amount] of chosen) amounts[itemId] = (amounts[itemId] ?? 0) + amount
      bundles.set(address, amounts)
    }
  }
  return [...bundles].map(([address, amounts]) => ({ address, amounts }))
}

/**
 * The parallel `issueTokens(beneficiaries, itemIds)` arrays: one entry per copy. Re-checks the cap the UI
 * enforces so a programming error can't pay gas for a transaction bound to run out of it.
 */
export function flattenTransfers(
  transfers: Transfer[],
  items: Item[]
): { beneficiaries: string[]; tokenIds: string[] } {
  const tokenIdOf = new Map(items.map(item => [item.id, item.tokenId]))
  const beneficiaries: string[] = []
  const tokenIds: string[] = []
  for (const transfer of transfers) {
    for (const recipient of transfer.recipients) {
      for (const [itemId, amount] of Object.entries(transfer.amounts)) {
        const tokenId = tokenIdOf.get(itemId)
        if (!tokenId) throw new SellItemError('not_published', `Item ${itemId} has no token id`)
        for (let i = 0; i < amount; i++) {
          beneficiaries.push(recipient)
          tokenIds.push(tokenId)
        }
      }
    }
  }
  if (beneficiaries.length > MAX_ITEMS_PER_SEND) {
    throw new SellItemError('generic', `Cannot send more than ${MAX_ITEMS_PER_SEND} items at once`)
  }
  return { beneficiaries, tokenIds }
}

export function buildIssueTokensCall(
  collection: Collection,
  beneficiaries: string[],
  tokenIds: string[],
  chainId: number
): ContractCall {
  if (!collection.contractAddress) throw new SellItemError('not_published', 'The collection has no contract yet')
  const contract = getCollectionContract(chainId, collection.contractAddress)
  return { contract, method: 'issueTokens', args: [beneficiaries, tokenIds] }
}

/** Copies per item across every transfer, to bump `totalSupply` once the transaction is mined. */
export function copiesPerItem(transfers: Transfer[]): Record<string, number> {
  const copies: Record<string, number> = {}
  for (const transfer of transfers) {
    for (const [itemId, amount] of Object.entries(transfer.amounts)) {
      copies[itemId] = (copies[itemId] ?? 0) + transfer.recipients.length * amount
    }
  }
  return copies
}

/** The size of a send, as the analytics events report it: distinct items, total copies and distinct wallets. */
export function sendBatchSize(transfers: Transfer[]): { item_count: number; copies: number; recipient_count: number } {
  const copies = copiesPerItem(transfers)
  return {
    item_count: Object.values(copies).filter(amount => amount > 0).length,
    copies: Object.values(copies).reduce((total, amount) => total + amount, 0),
    recipient_count: new Set(transfers.flatMap(transfer => transfer.recipients.map(to => to.toLowerCase()))).size
  }
}
