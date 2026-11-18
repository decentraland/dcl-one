import { describe, expect, it } from 'vitest'
import { toTransaction } from '~/test/chain'
import { type Collection } from './collections'
import { ItemType, type Item } from './items'
import {
  MAX_ITEMS_PER_SEND,
  buildIssueTokensCall,
  canSendCollectionItems,
  copiesPerItem,
  copiesPerRecipient,
  flattenTransfers,
  getStock,
  hasSendDraft,
  isSendableItem,
  holdsAddresses,
  maxAmount,
  parseRecipientList,
  sendBatchSize,
  totalCopies,
  type Transfer
} from './mint'

const OWNER = '0x00000000000000000000000000000000000000aa'
const MINTER = '0x00000000000000000000000000000000000000bb'
const MANAGER = '0x00000000000000000000000000000000000000cc'
const CONTRACT = '0x00000000000000000000000000000000000000dd'
const CHAIN_ID = 80002

const collection: Collection = {
  id: 'c1',
  name: 'Hats',
  owner: OWNER,
  urn: `urn:decentraland:amoy:collections-v2:${CONTRACT}`,
  contractAddress: CONTRACT,
  isPublished: true,
  isApproved: true,
  itemCount: 2,
  minters: [MINTER.toUpperCase()],
  managers: [MANAGER],
  createdAt: 1,
  updatedAt: 1
}

function makeItem(id: string, tokenId: string, totalSupply: number, rarity = 'legendary'): Item {
  return {
    id,
    name: id,
    description: '',
    thumbnail: 'thumbnail.png',
    owner: OWNER,
    collectionId: 'c1',
    rarity,
    totalSupply,
    tokenId,
    isPublished: true,
    isApproved: true,
    inCatalyst: true,
    type: ItemType.WEARABLE,
    data: { category: 'hat', representations: [] },
    contents: {},
    createdAt: 1,
    updatedAt: 1
  }
}

// Legendary: 100 copies. hat has 93 left, cap 2.
const hat = makeItem('hat', '1', 7)
const cap = makeItem('cap', '2', 98)

describe('canSendCollectionItems', () => {
  it('lets the owner and minters of an approved collection send, but not collaborators or strangers', () => {
    expect(canSendCollectionItems(collection, OWNER)).toBe(true)
    expect(canSendCollectionItems(collection, MINTER)).toBe(true)
    expect(canSendCollectionItems(collection, MANAGER)).toBe(false)
    expect(canSendCollectionItems(collection, undefined)).toBe(false)
    expect(canSendCollectionItems({ ...collection, isApproved: false, reviewedAt: 1, createdAt: 1 }, OWNER)).toBe(false)
  })

  it('keeps a former owner who listed themselves as a sender able to send after a transfer', () => {
    const transferred = { ...collection, owner: MANAGER, minters: [OWNER] }
    expect(canSendCollectionItems(transferred, OWNER)).toBe(true)
    expect(canSendCollectionItems({ ...transferred, minters: [] }, OWNER)).toBe(false)
  })
})

describe('isSendableItem / getStock', () => {
  it('needs a published, approved item with a token id and a rarity; sold out still counts as sendable', () => {
    expect(isSendableItem(hat)).toBe(true)
    expect(isSendableItem(makeItem('x', '9', 100))).toBe(true)
    expect(isSendableItem({ ...hat, tokenId: undefined })).toBe(false)
    expect(isSendableItem({ ...hat, isApproved: false })).toBe(false)
    expect(isSendableItem({ ...hat, rarity: undefined })).toBe(false)
    expect(getStock(hat)).toEqual({ available: 93, total: 100 })
    expect(getStock(makeItem('x', '9', 120))).toEqual({ available: 0, total: 100 })
  })
})

describe('allocation math', () => {
  const transfers: Transfer[] = [
    { recipients: ['0xa', '0xb'], amounts: { hat: 3, cap: 1 } },
    { recipients: ['0xc'], amounts: { hat: 2 } }
  ]

  it('has a draft once any transfer names a recipient or sets a copy', () => {
    expect(hasSendDraft([{ recipients: [], amounts: {} }])).toBe(false)
    expect(hasSendDraft([{ recipients: [], amounts: { a: 0 } }])).toBe(false)
    expect(
      hasSendDraft([
        { recipients: [], amounts: {} },
        { recipients: [MINTER], amounts: {} }
      ])
    ).toBe(true)
    expect(hasSendDraft([{ recipients: [], amounts: { a: 1 } }])).toBe(true)
  })

  it('counts copies as recipients × amounts across transfers', () => {
    expect(totalCopies(transfers)).toBe(2 * 4 + 2)
    expect(copiesPerItem(transfers)).toEqual({ hat: 8, cap: 2 })
    expect(sendBatchSize([...transfers, { recipients: ['0xA'], amounts: { hat: 0 } }])).toEqual({
      item_count: 2,
      copies: 10,
      recipient_count: 3
    })
  })

  it('merges what every wallet receives across transfers, skipping empty ones', () => {
    const repeated: Transfer[] = [
      ...transfers,
      { recipients: ['0xa'], amounts: { cap: 2 } },
      { recipients: ['0xd'], amounts: {} }
    ]
    expect(copiesPerRecipient(repeated)).toEqual([
      { address: '0xa', amounts: { hat: 3, cap: 3 } },
      { address: '0xb', amounts: { hat: 3, cap: 1 } },
      { address: '0xc', amounts: { hat: 2 } }
    ])
  })

  it('caps an amount by the stock left after the other transfers, per recipient', () => {
    // cap: 2 left, 2 recipients in the first transfer → 1 each; nothing left for the second transfer.
    expect(maxAmount(cap, transfers, 0)).toBe(1)
    expect(maxAmount(cap, transfers, 1)).toBe(0)
    // hat: 93 - 2 (other transfer) = 91 for the first transfer's 2 recipients → 45 each.
    expect(maxAmount(hat, transfers, 0)).toBe(45)
    // A transfer without recipients yet is sized as if it had one.
    expect(maxAmount(hat, [{ recipients: [], amounts: {} }], 0)).toBe(93)
  })
})

describe('flattenTransfers / buildIssueTokensCall', () => {
  it('repeats each recipient once per copy, in parallel with the item token ids, skipping incomplete transfers', () => {
    const transfers: Transfer[] = [
      { recipients: ['0xa', '0xb'], amounts: { hat: 2, cap: 1 } },
      { recipients: [], amounts: { hat: 5 } },
      { recipients: ['0xc'], amounts: { hat: 0 } }
    ]
    expect(flattenTransfers(transfers, [hat, cap])).toEqual({
      beneficiaries: ['0xa', '0xa', '0xa', '0xb', '0xb', '0xb'],
      tokenIds: ['1', '1', '2', '1', '1', '2']
    })
  })

  it('refuses more copies than one transaction can mint', () => {
    const atCap: Transfer[] = [{ recipients: ['0xa'], amounts: { hat: MAX_ITEMS_PER_SEND } }]
    expect(flattenTransfers(atCap, [hat]).beneficiaries).toHaveLength(MAX_ITEMS_PER_SEND)

    const overCap: Transfer[] = [{ recipients: ['0xa'], amounts: { hat: MAX_ITEMS_PER_SEND + 1 } }]
    expect(() => flattenTransfers(overCap, [hat])).toThrow(/more than/)
  })

  it('encodes issueTokens on the collection contract and refuses a collection without one', () => {
    const call = buildIssueTokensCall(collection, ['0x00000000000000000000000000000000000000ee'], ['1'], CHAIN_ID)
    expect(toTransaction(call)).toEqual({
      to: '0x00000000000000000000000000000000000000dd',
      data:
        '0x7c8f76a1' +
        '0000000000000000000000000000000000000000000000000000000000000040' +
        '0000000000000000000000000000000000000000000000000000000000000080' +
        '0000000000000000000000000000000000000000000000000000000000000001' +
        '00000000000000000000000000000000000000000000000000000000000000ee' +
        '0000000000000000000000000000000000000000000000000000000000000001' +
        '0000000000000000000000000000000000000000000000000000000000000001'
    })
    expect(() => buildIssueTokensCall({ ...collection, contractAddress: undefined }, [], [], CHAIN_ID)).toThrow(
      /no contract/
    )
  })
})

describe('parseRecipientList', () => {
  const A = '0x00000000000000000000000000000000000000a1'
  const B = '0x00000000000000000000000000000000000000b2'
  const C = '0x00000000000000000000000000000000000000c3'

  it('takes the addresses out of a pasted sheet, whatever surrounds them, lowercased and deduped', () => {
    const sheet = `wallet\tname\n${A.toUpperCase().replace('0X', '0x')}\tJane\n"${B}", ${C};${A}\n${B}\n`
    expect(parseRecipientList(sheet)).toEqual({ added: [A, B, C], invalid: 0, repeated: 2, cut: 0 })
  })

  it('counts repeats of the transfer apart from wrong checksums and mistyped or overlong hex runs', () => {
    const badChecksum = '0xd8da6bf26964af9d7eed9e03e53415d37aa96045'.replace('d8da', 'D8dA')
    const short = A.slice(0, -1)
    const hash = `0x${'ab'.repeat(32)}`
    expect(parseRecipientList(`${A}\n${badChecksum}\n${short}, ${hash}\n${B}`, [A])).toEqual({
      added: [B],
      invalid: 3,
      repeated: 1,
      cut: 0
    })
  })

  it('cuts a list that would take the transfer past the send cap, and reports the cut', () => {
    const list = Array.from({ length: 5 }, (_, i) => `0x${String(i + 1).padStart(40, '0')}`).join(', ')
    expect(parseRecipientList(list, [A], 3)).toEqual({
      added: ['0x' + '1'.padStart(40, '0'), '0x' + '2'.padStart(40, '0')],
      invalid: 0,
      repeated: 0,
      cut: 3
    })
    expect(parseRecipientList(list, [A, B, C], 3)).toEqual({ added: [], invalid: 0, repeated: 0, cut: 5 })
    expect(parseRecipientList(list).added).toHaveLength(5)
  })

  it('tells text that carries an address from text that does not', () => {
    expect(holdsAddresses(`name, ${A}`)).toBe(true)
    expect(holdsAddresses(`${A}${B}`)).toBe(false)
    expect(holdsAddresses('0x1234')).toBe(false)
    expect(holdsAddresses(`0x${'ab'.repeat(32)}`)).toBe(false)
  })
})
