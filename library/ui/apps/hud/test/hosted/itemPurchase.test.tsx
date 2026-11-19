import { describe, it, expect, vi, afterEach } from 'vitest'
import { waitFor } from '@testing-library/react'
import { renderSession, enterAsGuest, FakeDriver } from '../harness'
import { answerItemPurchase, type PurchaseSceneItem } from '../../features/itemPurchase/answer'
import type { ItemPurchaseRequestMessage } from '@explorer-core/engine/protocol'

const ITEM = { contract: '0x' + 'ab'.repeat(20), itemId: '7' }
const REQ: ItemPurchaseRequestMessage = { kind: 'itemPurchaseRequest', id: 3, item: ITEM }

const reports = (driver: FakeDriver): unknown[] => driver.sent.filter((m) => m.kind === 'itemPurchaseState')

afterEach(() => vi.restoreAllMocks())

// DOMAIN: item purchase — the session answers a scene's request on receipt, with the host's purchase.
describe('item purchase domain', () => {
  it('answers unavailable when the host cannot buy', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    h.driver.emit(REQ)
    expect(reports(h.driver)).toEqual([{ kind: 'itemPurchaseState', id: 3, state: 'unavailable' }])
  })

  it('accepts with the host\'s purchase and reports how it ended', async () => {
    let end!: (outcome: 'purchased') => void
    const purchase = vi.fn<PurchaseSceneItem>(() => new Promise((resolve) => (end = resolve)))
    const h = renderSession(undefined, new FakeDriver(), { purchaseSceneItem: purchase })
    await enterAsGuest(h)
    h.driver.emit(REQ)
    expect(purchase).toHaveBeenCalledWith(ITEM)
    expect(reports(h.driver)).toEqual([{ kind: 'itemPurchaseState', id: 3, state: 'accepted' }])
    end('purchased')
    await waitFor(() => expect(h.driver.last('itemPurchaseState')).toEqual({ kind: 'itemPurchaseState', id: 3, state: 'purchased' }))
  })
})

describe('answering an item purchase request', () => {
  it('is unavailable where the host cannot buy, whatever the item', () => {
    const report = vi.fn()
    answerItemPurchase(REQ, report, undefined)
    answerItemPurchase({ ...REQ, id: 4, item: null }, report, undefined)
    expect(report.mock.calls).toEqual([
      [3, 'unavailable'],
      [4, 'unavailable']
    ])
  })

  it('fails a request that names no purchasable item without opening anything', () => {
    const report = vi.fn()
    const purchase = vi.fn<PurchaseSceneItem>()
    answerItemPurchase({ ...REQ, item: null }, report, purchase)
    expect(purchase).not.toHaveBeenCalled()
    expect(report.mock.calls).toEqual([
      [3, 'accepted'],
      [3, 'failed']
    ])
  })

  it.each([
    ['rejects', (): Promise<never> => Promise.reject(new Error('no shell'))],
    [
      'throws before returning a promise',
      (): never => {
        throw new Error('no shell')
      }
    ]
  ])('fails when the purchase %s', async (_, purchase) => {
    const report = vi.fn()
    vi.spyOn(console, 'warn').mockImplementation(() => {})
    answerItemPurchase(REQ, report, purchase)
    await waitFor(() => expect(report).toHaveBeenLastCalledWith(3, 'failed'))
    expect(report.mock.calls).toEqual([
      [3, 'accepted'],
      [3, 'failed']
    ])
  })
})
