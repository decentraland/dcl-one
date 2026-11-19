import { describe, it, expect, vi } from 'vitest'
import { act } from '@testing-library/react'
import { renderSession, enterAsGuest } from '../harness'
import { answerItemPurchase } from '../../features/itemPurchase/answer'
import type { ItemPurchaseRequestMessage } from '@explorer-core/engine/protocol'

const ITEM = { contract: '0x' + 'ab'.repeat(20), itemId: '7' }
const REQ: ItemPurchaseRequestMessage = { kind: 'itemPurchaseRequest', id: 3, item: ITEM }

// DOMAIN: item purchase — a scene's request is held until its first report, and each report is posted back.
describe('item purchase domain', () => {
  it('holds a request until its first report and posts every report', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    h.driver.emit(REQ)
    expect(h.session().itemPurchase.pending).toEqual(REQ)
    act(() => h.session().itemPurchase.report(3, 'accepted'))
    expect(h.session().itemPurchase.pending).toBeNull()
    expect(h.driver.last('itemPurchaseState')).toEqual({ kind: 'itemPurchaseState', id: 3, state: 'accepted' })
    act(() => h.session().itemPurchase.report(3, 'purchased'))
    expect(h.driver.last('itemPurchaseState')).toEqual({ kind: 'itemPurchaseState', id: 3, state: 'purchased' })
  })

  it('a report on an earlier request leaves a newer one pending', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    h.driver.emit({ ...REQ, id: 4 })
    act(() => h.session().itemPurchase.report(3, 'dismissed'))
    expect(h.session().itemPurchase.pending?.id).toBe(4)
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

  it('accepts at once, then reports how the purchase ended', async () => {
    const report = vi.fn()
    let end!: (outcome: 'purchased') => void
    const purchase = vi.fn(() => new Promise<'purchased'>((resolve) => (end = resolve)))
    answerItemPurchase(REQ, report, purchase)
    expect(purchase).toHaveBeenCalledWith(ITEM)
    expect(report.mock.calls).toEqual([[3, 'accepted']])
    end('purchased')
    await Promise.resolve()
    expect(report.mock.calls).toEqual([
      [3, 'accepted'],
      [3, 'purchased']
    ])
  })

  it('fails a request that names no purchasable item without opening anything', () => {
    const report = vi.fn()
    const purchase = vi.fn()
    answerItemPurchase({ ...REQ, item: null }, report, purchase)
    expect(purchase).not.toHaveBeenCalled()
    expect(report.mock.calls).toEqual([
      [3, 'accepted'],
      [3, 'failed']
    ])
  })

  it('fails when the purchase throws', async () => {
    const report = vi.fn()
    vi.spyOn(console, 'warn').mockImplementation(() => {})
    answerItemPurchase(REQ, report, () => Promise.reject(new Error('no shell')))
    await Promise.resolve()
    await Promise.resolve()
    expect(report).toHaveBeenLastCalledWith(3, 'failed')
  })
})
