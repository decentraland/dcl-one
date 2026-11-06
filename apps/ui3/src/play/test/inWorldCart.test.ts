// The in-world Shop's cart (features/shop/cartStore): what it keeps, how much of each, per account.
import { describe, expect, it } from 'vitest'
import { createInWorldCart, type CartStorage, type InWorldCartInput } from '../features/shop/cartStore'
import { MAX_PURCHASE_UNITS } from '../lib/shopProtocol'

const A = '0x' + 'aa'.repeat(20)
const B = '0x' + 'bb'.repeat(20)
const item = (itemId: string, extra: Partial<InWorldCartInput> = {}): InWorldCartInput => ({
  contract: '0x' + '11'.repeat(20),
  itemId,
  name: `Item ${itemId}`,
  rarity: 'epic',
  category: 'upper_body',
  creator: '0x' + '22'.repeat(20),
  credits: 500,
  available: 10,
  ...extra
})

function storage(): CartStorage & { map: Map<string, string> } {
  const map = new Map<string, string>()
  return { map, getItem: (k) => map.get(k) ?? null, setItem: (k, v) => void map.set(k, v), removeItem: (k) => void map.delete(k) }
}

describe('the in-world cart', () => {
  it('keeps an item once, one more of it each time it is added', () => {
    const cart = createInWorldCart({ storage: storage })
    cart.scopeTo(A)
    expect(cart.add(item('1'))).toBe(true)
    expect(cart.add(item('1'))).toBe(true)
    expect(cart.add(item('2'))).toBe(true)
    expect(cart.getSnapshot().map((l) => `${l.itemId}x${l.quantity}`)).toEqual(['1x2', '2x1'])
  })

  it('takes no more of an item than is left of it, nor more units than one purchase buys', () => {
    const cart = createInWorldCart({ storage: storage })
    cart.scopeTo(A)
    expect(cart.add(item('1', { available: 1 }))).toBe(true)
    expect(cart.add(item('1', { available: 1 }))).toBe(false)
    cart.setQuantity(item('1'), 5)
    expect(cart.getSnapshot()[0].quantity).toBe(1)
    cart.add(item('2', { available: 100 }))
    cart.setQuantity(item('2'), 100)
    expect(cart.getSnapshot()[1].quantity).toBe(MAX_PURCHASE_UNITS - 1)
    expect(cart.canAdd(item('3'))).toBe(false)
    expect(cart.add(item('3'))).toBe(false)
  })

  it('never goes below one of an item, and removes it on its own', () => {
    const cart = createInWorldCart({ storage: storage })
    cart.scopeTo(A)
    cart.add(item('1'))
    cart.setQuantity(item('1'), 0)
    expect(cart.getSnapshot()[0].quantity).toBe(1)
    cart.remove(item('1'))
    expect(cart.getSnapshot()).toEqual([])
  })

  it('lets go of what a purchase bought, and only that', () => {
    const cart = createInWorldCart({ storage: storage })
    cart.scopeTo(A)
    cart.add(item('1'))
    cart.add(item('2'))
    cart.removeBought([{ contract: '0x' + '11'.repeat(20), itemId: '1' }])
    expect(cart.getSnapshot().map((l) => l.itemId)).toEqual(['2'])
  })

  it('keeps each account’s cart apart, across pages', () => {
    const s = storage()
    const first = createInWorldCart({ storage: () => s })
    first.scopeTo(A)
    first.add(item('1'))
    first.scopeTo(B)
    expect(first.getSnapshot()).toEqual([])
    first.add(item('2'))
    const reloaded = createInWorldCart({ storage: () => s })
    reloaded.scopeTo(A)
    expect(reloaded.getSnapshot().map((l) => l.itemId)).toEqual(['1'])
    reloaded.scopeTo(B)
    expect(reloaded.getSnapshot().map((l) => l.itemId)).toEqual(['2'])
  })

  it('makes a signed-out cart the account’s once one signs in', () => {
    const s = storage()
    const cart = createInWorldCart({ storage: () => s })
    cart.add(item('1'))
    cart.scopeTo(A)
    expect(cart.getSnapshot().map((l) => l.itemId)).toEqual(['1'])
    cart.scopeTo(null)
    expect(cart.getSnapshot()).toEqual([])
  })

  it('skips what storage holds in another shape', () => {
    const s = storage()
    s.setItem(`dcl-inworld-shop:cart:v1:${A}`, JSON.stringify([{ contract: 'nope', itemId: '1', quantity: 1 }, { ...item('2'), quantity: 2, addedAt: 1 }, { ...item('2'), quantity: 1, addedAt: 2 }]))
    const cart = createInWorldCart({ storage: () => s })
    cart.scopeTo(A)
    expect(cart.getSnapshot().map((l) => `${l.itemId}x${l.quantity}`)).toEqual(['2x2'])
  })

  it('stays in memory when storage refuses it', () => {
    const cart = createInWorldCart({
      storage: () => ({
        getItem: () => null,
        setItem: () => {
          throw new Error('full')
        },
        removeItem: () => {}
      })
    })
    cart.scopeTo(A)
    expect(cart.add(item('1'))).toBe(true)
    expect(cart.getSnapshot()).toHaveLength(1)
  })
})
