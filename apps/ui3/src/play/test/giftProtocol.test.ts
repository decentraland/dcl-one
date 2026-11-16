// The Gift protocol (lib/giftProtocol): what the shell admits from the app frame, and what the HUD reads back.
import { describe, expect, it } from 'vitest'
import { GIFT_EVENT, GIFT_PROTOCOL, isGiftEvent, parseGiftCapabilities, parseGiftRequest, parsePendingGifts } from '../lib/giftProtocol'

const URN = 'urn:decentraland:amoy:collections-v2:0x' + 'a'.repeat(40) + ':0:105'
const REQUEST = { v: GIFT_PROTOCOL, recipient: '0x' + 'B'.repeat(40), urn: URN, tokenId: '105', recipientName: 'Bob', transferredAt: 1700000000 }

describe('parseGiftRequest', () => {
  it('admits the recipient, the token and a display name, with the recipient lowercased', () => {
    expect(parseGiftRequest(REQUEST)).toEqual({ ...REQUEST, recipient: '0x' + 'b'.repeat(40) })
  })

  it('refuses unknown fields, other versions and malformed values', () => {
    expect(() => parseGiftRequest({ ...REQUEST, amount: 1 })).toThrow('unexpected gift.open fields: amount')
    expect(() => parseGiftRequest({ ...REQUEST, v: 2 })).toThrow('unsupported')
    expect(() => parseGiftRequest({ ...REQUEST, recipient: 'bob' })).toThrow('recipient')
    expect(() => parseGiftRequest({ ...REQUEST, tokenId: '0x1' })).toThrow('tokenId')
    expect(() => parseGiftRequest({ ...REQUEST, urn: URN.replace(':105', ':106') })).toThrow('urn')
    expect(() => parseGiftRequest({ ...REQUEST, urn: 'urn:decentraland:off-chain:base-avatars:hat:105' })).toThrow('urn')
    expect(() => parseGiftRequest({ ...REQUEST, recipientName: 'x'.repeat(101) })).toThrow('recipientName')
    expect(() => parseGiftRequest({ ...REQUEST, transferredAt: -1 })).toThrow('transferredAt')
    expect(() => parseGiftRequest(null)).toThrow()
  })
})

describe('what the HUD reads back', () => {
  it('reads anything unreadable as unavailable or empty', () => {
    expect(parseGiftCapabilities({ v: GIFT_PROTOCOL, available: true }).available).toBe(true)
    expect(parseGiftCapabilities({ v: 9, available: true }).available).toBe(false)
    expect(parsePendingGifts({ v: GIFT_PROTOCOL, pending: [{ urn: URN, transferredAt: 5 }, { urn: 'nope', transferredAt: 1 }] })).toEqual([{ urn: URN, transferredAt: 5 }])
    expect(parsePendingGifts(null)).toEqual([])
  })

  it('tells the shell\'s "gift sent"', () => {
    expect(isGiftEvent({ type: GIFT_EVENT, v: GIFT_PROTOCOL, state: 'sent', urn: URN, transferredAt: 5 })).toBe(true)
    expect(isGiftEvent({ type: GIFT_EVENT, v: GIFT_PROTOCOL, state: 'sent', urn: 'x', transferredAt: 5 })).toBe(false)
  })
})
