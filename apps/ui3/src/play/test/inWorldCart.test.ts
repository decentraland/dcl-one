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
    const s = storage()
    const cart = createInWorldCart({ storage: () => s })
    cart.scopeTo(A)
    expect(cart.add(item('1'))).toBe(true)
    expect(cart.add(item('1'))).toBe(true)
    expect(cart.add(item('2'))).toBe(true)
    expect(cart.getSnapshot().map((l) => `${l.itemId}x${l.quantity}`)).toEqual(['1x2', '2x1'])
  })

  it('takes no more of an item than is left of it, nor more units than one purchase buys', () => {
    const s = storage()
    const cart = createInWorldCart({ storage: () => s })
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

  it('takes one more of an item only if what is left of it now allows', () => {
    const s = storage()
    const cart = createInWorldCart({ storage: () => s })
    cart.scopeTo(A)
    cart.add(item('1', { available: 5 }))
    cart.add(item('1', { available: 5 }))
    // The listing now says one is left: the cart's two are already more than that.
    expect(cart.canAdd(item('1', { available: 1 }))).toBe(false)
    expect(cart.add(item('1', { available: 1 }))).toBe(false)
    expect(cart.getSnapshot()[0].quantity).toBe(2)
  })

  it('reads another tab’s write to the account’s cart as it lands, so neither tab undoes the other', () => {
    const s = storage()
    const listeners: Array<(key: string | null) => void> = []
    const watch = (onChange: (key: string | null) => void) => (listeners.push(onChange), () => {})
    const tabA = createInWorldCart({ storage: () => s, watch })
    const tabB = createInWorldCart({ storage: () => s, watch })
    tabA.scopeTo(A)
    tabB.scopeTo(A)
    tabA.add(item('1'))
    tabB.add(item('2'))
    // The browser tells the other tab which key changed.
    for (const l of listeners) l(`dcl-inworld-shop:cart:v2:${A}`)
    expect(tabA.getSnapshot().map((l) => l.itemId)).toEqual(['1', '2'])
    tabA.setQuantity(item('1'), 2)
    const reloaded = createInWorldCart({ storage: () => s, watch })
    reloaded.scopeTo(A)
    expect(reloaded.getSnapshot().map((l) => `${l.itemId}x${l.quantity}`)).toEqual(['1x2', '2x1'])
  })

  it('never goes below one of an item, and removes it on its own', () => {
    const s = storage()
    const cart = createInWorldCart({ storage: () => s })
    cart.scopeTo(A)
    cart.add(item('1'))
    cart.setQuantity(item('1'), 0)
    expect(cart.getSnapshot()[0].quantity).toBe(1)
    cart.remove(item('1'))
    expect(cart.getSnapshot()).toEqual([])
  })

  const OP = '0f0e0d0c-0000-4000-8000-000000000001'
  const bought = (itemId: string, quantity = 1) => [{ contract: '0x' + '11'.repeat(20), itemId, quantity }]

  it('lets go of as many as a purchase bought, and keeps what was added since', () => {
    const s = storage()
    const cart = createInWorldCart({ storage: () => s })
    cart.scopeTo(A)
    cart.add(item('1'))
    cart.add(item('1'))
    cart.add(item('2'))
    cart.applyPurchase(A, OP, bought('1'))
    expect(cart.getSnapshot().map((l) => `${l.itemId}x${l.quantity}`)).toEqual(['1x1', '2x1'])
    cart.applyPurchase(A, '0f0e0d0c-0000-4000-8000-000000000002', [...bought('1'), ...bought('2', 3)])
    expect(cart.getSnapshot()).toEqual([])
  })

  it('takes a purchase out once, however often it is heard, across pages', () => {
    const s = storage()
    const cart = createInWorldCart({ storage: () => s })
    cart.scopeTo(A)
    cart.add(item('1'))
    cart.applyPurchase(A, OP, bought('1'))
    cart.add(item('1'))
    cart.applyPurchase(A, OP, bought('1'))
    expect(cart.getSnapshot().map((l) => `${l.itemId}x${l.quantity}`)).toEqual(['1x1'])
    const reloaded = createInWorldCart({ storage: () => s })
    reloaded.scopeTo(A)
    reloaded.applyPurchase(A, OP, bought('1'))
    expect(reloaded.getSnapshot().map((l) => `${l.itemId}x${l.quantity}`)).toEqual(['1x1'])
  })

  /** A storage that stops keeping anything once `refuse` is set. */
  function flaky() {
    const s = storage()
    const state = { refuse: false }
    return {
      state,
      s,
      storage: (): CartStorage => ({
        getItem: s.getItem,
        setItem: (k, v) => {
          if (state.refuse) throw new Error('full')
          s.setItem(k, v)
        },
        removeItem: (k) => {
          if (state.refuse) throw new Error('full')
          s.removeItem(k)
        }
      })
    }
  }

  it('keeps a purchase and its taking-out together: a write that fails keeps neither, and the replay takes it out once', () => {
    const f = flaky()
    const cart = createInWorldCart({ storage: f.storage })
    cart.scopeTo(A)
    cart.add(item('1'))
    cart.add(item('1'))
    f.state.refuse = true
    cart.applyPurchase(A, OP, bought('1'))
    expect(cart.getSnapshot()[0].quantity).toBe(1)
    // Within the page, what it couldn't keep is what it knows: heard again, nothing more leaves.
    cart.applyPurchase(A, OP, bought('1'))
    expect(cart.getSnapshot()[0].quantity).toBe(1)
    // After a reload, neither was kept: the replay (the shell's status) takes it out, once.
    f.state.refuse = false
    const reloaded = createInWorldCart({ storage: f.storage })
    reloaded.scopeTo(A)
    expect(reloaded.getSnapshot()[0].quantity).toBe(2)
    reloaded.applyPurchase(A, OP, bought('1'))
    expect(reloaded.getSnapshot()[0].quantity).toBe(1)
    const again = createInWorldCart({ storage: f.storage })
    again.scopeTo(A)
    again.applyPurchase(A, OP, bought('1'))
    expect(again.getSnapshot()[0].quantity).toBe(1)
  })

  it('keeps an account’s record in memory when its write fails, so a replay for it takes nothing more', () => {
    const f = flaky()
    const cart = createInWorldCart({ storage: f.storage })
    cart.scopeTo(A)
    cart.add(item('1'))
    cart.add(item('1'))
    cart.scopeTo(B)
    f.state.refuse = true
    cart.applyPurchase(A, OP, bought('1'))
    cart.applyPurchase(A, OP, bought('1'))
    cart.scopeTo(A)
    expect(cart.getSnapshot()[0].quantity).toBe(1)
  })

  it('remembers what was taken out even once the cart is empty', () => {
    const s = storage()
    const cart = createInWorldCart({ storage: () => s })
    cart.scopeTo(A)
    cart.add(item('1'))
    cart.applyPurchase(A, OP, bought('1'))
    expect(cart.getSnapshot()).toEqual([])
    const reloaded = createInWorldCart({ storage: () => s })
    reloaded.scopeTo(A)
    reloaded.add(item('1'))
    reloaded.applyPurchase(A, OP, bought('1'))
    expect(reloaded.getSnapshot().map((l) => `${l.itemId}x${l.quantity}`)).toEqual(['1x1'])
  })

  it('takes a purchase out of its buyer’s cart, whichever account is active', () => {
    const s = storage()
    const cart = createInWorldCart({ storage: () => s })
    cart.scopeTo(A)
    cart.add(item('1'))
    cart.scopeTo(B)
    cart.add(item('1'))
    cart.applyPurchase(A, OP, bought('1'))
    expect(cart.getSnapshot().map((l) => l.itemId)).toEqual(['1'])
    cart.scopeTo(A)
    expect(cart.getSnapshot()).toEqual([])
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
    const key = `dcl-inworld-shop:cart:v2:${A}`
    s.setItem(key, JSON.stringify({ v: 2, lines: [{ contract: 'nope', itemId: '1', quantity: 1 }, { ...item('2'), quantity: 2, addedAt: 1 }, { ...item('2'), quantity: 1, addedAt: 2 }], applied: ['nope', OP] }))
    const cart = createInWorldCart({ storage: () => s })
    cart.scopeTo(A)
    expect(cart.getSnapshot().map((l) => `${l.itemId}x${l.quantity}`)).toEqual(['2x2'])
    // The operation it kept is still taken out; the malformed id was dropped.
    cart.applyPurchase(A, OP, bought('2'))
    expect(cart.getSnapshot()[0].quantity).toBe(2)
    for (const other of [JSON.stringify([{ ...item('2'), quantity: 1, addedAt: 1 }]), JSON.stringify({ v: 1, lines: [], applied: [] }), '{not json']) {
      s.setItem(key, other)
      const fresh = createInWorldCart({ storage: () => s })
      fresh.scopeTo(A)
      expect(fresh.getSnapshot()).toEqual([])
    }
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
