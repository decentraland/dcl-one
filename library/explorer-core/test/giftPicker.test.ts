// The gift picker's rules (features/gifting/picker).
import { describe, expect, it } from 'vitest'
import type { GiftableItem } from '../engine/protocol'
import { bestToken, giftRows, transferable } from '../features/gifting/picker'

const ITEM = 'urn:decentraland:matic:collections-v2:0xabc:0'
const item = (amount: number, urn = ITEM): GiftableItem => ({
  urn,
  name: 'Hat',
  rarity: 'epic',
  category: 'hat',
  amount,
  tokens: Array.from({ length: amount }, (_, i) => ({ urn: `${urn}:${i + 1}`, tokenId: String(i + 1), transferredAt: 100 + i }))
})

describe('giftRows', () => {
  it('lists an item with its copies', () => {
    expect(giftRows([item(2)], [], [])).toEqual([{ item: item(2), amount: 2, equipped: false, giftable: true }])
  })

  it("doesn't count a pending gift, and drops an item with no copy left", () => {
    expect(giftRows([item(2)], [], [{ urn: `${ITEM}:2`, transferredAt: 101 }])[0]?.amount).toBe(1)
    expect(giftRows([item(1)], [], [{ urn: `${ITEM}:1`, transferredAt: 100 }])).toEqual([])
  })

  it('counts a pending gift again once the token changed hands since', () => {
    expect(giftRows([item(1)], [], [{ urn: `${ITEM}:1`, transferredAt: 50 }])[0]?.amount).toBe(1)
  })

  it('keeps an equipped single copy, not giftable; with a spare copy it can be gifted', () => {
    expect(giftRows([item(1)], [`${ITEM}:1`], [])[0]).toMatchObject({ equipped: true, giftable: false })
    expect(giftRows([item(2)], [`${ITEM}:1`], [])[0]).toMatchObject({ equipped: true, giftable: true })
  })
})

describe('bestToken', () => {
  it('sends the newest-transferred copy that is neither worn nor pending', () => {
    expect(bestToken(item(3), [], [])?.tokenId).toBe('3')
    expect(bestToken(item(3), [`${ITEM}:3`], [])?.tokenId).toBe('2')
    expect(bestToken(item(3), [`${ITEM}:3`], [{ urn: `${ITEM}:2`, transferredAt: 101 }])?.tokenId).toBe('1')
    expect(bestToken(item(1), [`${ITEM}:1`], [])).toBeNull()
  })
})

describe('transferable', () => {
  it('admits a Polygon collection token only', () => {
    expect(transferable(`${ITEM}:1`)).toBe(true)
    expect(transferable('urn:decentraland:amoy:collections-v2:0xabc:0:1')).toBe(true)
    expect(transferable('urn:decentraland:ethereum:collections-v1:dcl_launch:hat:7')).toBe(false)
    expect(transferable(ITEM)).toBe(false)
  })
})
