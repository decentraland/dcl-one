// The in-world Shop's cart (features/shop/cartStore): what it keeps, how much of each, per account.
import { describe, expect, it, vi } from 'vitest'
import { createInWorldCart, type CartLock, type CartStorage, type InWorldCartInput } from '../features/shop/cartStore'
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
  it('keeps an item once, one more of it each time it is added', async () => {
    const s = storage()
    const cart = createInWorldCart({ storage: () => s })
    cart.scopeTo(A)
    expect(await cart.add(item('1'))).toBe(true)
    expect(await cart.add(item('1'))).toBe(true)
    expect(await cart.add(item('2'))).toBe(true)
    expect(cart.getSnapshot().map((l) => `${l.itemId}x${l.quantity}`)).toEqual(['1x2', '2x1'])
  })

  it('takes no more of an item than is left of it, nor more units than one purchase buys', async () => {
    const s = storage()
    const cart = createInWorldCart({ storage: () => s })
    cart.scopeTo(A)
    expect(await cart.add(item('1', { available: 1 }))).toBe(true)
    expect(await cart.add(item('1', { available: 1 }))).toBe(false)
    await cart.setQuantity(item('1'), 5)
    expect(cart.getSnapshot()[0].quantity).toBe(1)
    await cart.add(item('2', { available: 100 }))
    await cart.setQuantity(item('2'), 100)
    expect(cart.getSnapshot()[1].quantity).toBe(MAX_PURCHASE_UNITS - 1)
    expect(cart.canAdd(item('3'))).toBe(false)
    expect(await cart.add(item('3'))).toBe(false)
  })

  it('takes one more of an item only if what is left of it now allows', async () => {
    const s = storage()
    const cart = createInWorldCart({ storage: () => s })
    cart.scopeTo(A)
    await cart.add(item('1', { available: 5 }))
    await cart.add(item('1', { available: 5 }))
    // The listing now says one is left: the cart's two are already more than that.
    expect(cart.canAdd(item('1', { available: 1 }))).toBe(false)
    expect(await cart.add(item('1', { available: 1 }))).toBe(false)
    expect(cart.getSnapshot()[0].quantity).toBe(2)
  })

  /** Web Locks as the origin's tabs share them: one holder at a time, the rest in order. */
  function sharedLock(): CartLock {
    let chain: Promise<unknown> = Promise.resolve()
    return <T,>(fn: () => T): Promise<T> => {
      const run = chain.then(fn)
      chain = run.catch(() => {})
      return run
    }
  }

  it('never writes one tab’s change over another’s, even when both start before either has written', async () => {
    const shared = storage()
    const lock = sharedLock()
    const listeners: Array<(key: string | null) => void> = []
    const watch = (onChange: (key: string | null) => void) => (listeners.push(onChange), () => {})
    let tabB: ReturnType<typeof createInWorldCart> | null = null
    let pendingB: Promise<boolean> | null = null
    // Tab A's first write lets tab B add its own item first, the moment before A's lands.
    const storageA: CartStorage = {
      getItem: shared.getItem,
      removeItem: shared.removeItem,
      setItem: (k, v) => {
        if (tabB && !pendingB) pendingB = tabB.add(item('2'))
        shared.setItem(k, v)
      }
    }
    const tabA = createInWorldCart({ storage: () => storageA, watch, lock })
    tabB = createInWorldCart({ storage: () => shared, watch, lock })
    tabA.scopeTo(A)
    tabB.scopeTo(A)
    expect(await tabA.add(item('1'))).toBe(true)
    expect(await pendingB).toBe(true)
    const reloaded = createInWorldCart({ storage: () => shared, watch, lock })
    reloaded.scopeTo(A)
    expect(reloaded.getSnapshot().map((l) => l.itemId)).toEqual(['1', '2'])
    // The browser tells each tab which key the other wrote: both show what is kept.
    for (const l of listeners) l(`dcl-inworld-shop:cart:v2:${A}`)
    expect(tabA.getSnapshot().map((l) => l.itemId)).toEqual(['1', '2'])
    await tabA.setQuantity(item('1'), 2)
    const again = createInWorldCart({ storage: () => shared, watch, lock })
    again.scopeTo(A)
    expect(again.getSnapshot().map((l) => `${l.itemId}x${l.quantity}`)).toEqual(['1x2', '2x1'])
  })

  it('replays a change storage didn’t take on what another tab wrote since, never writing over it', async () => {
    const shared = storage()
    const lock = sharedLock()
    const refuse = { a: false }
    // Tab A's writes fail while `refuse.a`; tab B's always land. Storage events are never delivered.
    const storageA: CartStorage = {
      getItem: shared.getItem,
      setItem: (k, v) => {
        if (refuse.a) throw new Error('full')
        shared.setItem(k, v)
      },
      removeItem: shared.removeItem
    }
    const tabA = createInWorldCart({ storage: () => storageA, lock, watch: () => () => {} })
    const tabB = createInWorldCart({ storage: () => shared, lock, watch: () => () => {} })
    tabA.scopeTo(A)
    tabB.scopeTo(A)
    refuse.a = true
    await tabA.add(item('1'))
    expect(tabA.getSnapshot().map((l) => l.itemId)).toEqual(['1'])
    refuse.a = false
    await tabB.add(item('2'))
    // A changes the cart again before hearing of B's write: it reads B's, its own untaken change on it.
    await tabA.add(item('3'))
    const reloaded = createInWorldCart({ storage: () => shared, lock, watch: () => () => {} })
    reloaded.scopeTo(A)
    expect(reloaded.getSnapshot().map((l) => l.itemId).sort()).toEqual(['1', '2', '3'])
  })

  it('shows and keeps a change storage didn’t take, after an earlier one it didn’t take was undone', async () => {
    const shared = storage()
    const refuse = { on: false }
    const flakyStorage: CartStorage = {
      getItem: shared.getItem,
      setItem: (k, v) => {
        if (refuse.on) throw new Error('full')
        shared.setItem(k, v)
      },
      removeItem: (k) => {
        if (refuse.on) throw new Error('full')
        shared.removeItem(k)
      }
    }
    const cart = createInWorldCart({ storage: () => flakyStorage, watch: () => () => {} })
    cart.scopeTo(A)
    refuse.on = true
    await cart.add(item('1'))
    expect(cart.getSnapshot().map((l) => l.itemId)).toEqual(['1'])
    refuse.on = false
    await cart.remove(item('1'))
    expect(cart.getSnapshot()).toEqual([])
    // A second change storage doesn't take, as many pending as the first had: its own shows.
    refuse.on = true
    await cart.add(item('2'))
    expect(cart.getSnapshot().map((l) => l.itemId)).toEqual(['2'])
    refuse.on = false
    await cart.add(item('3'))
    expect(cart.getSnapshot().map((l) => l.itemId)).toEqual(['2', '3'])
    const reloaded = createInWorldCart({ storage: () => shared, watch: () => () => {} })
    reloaded.scopeTo(A)
    expect(reloaded.getSnapshot().map((l) => l.itemId)).toEqual(['2', '3'])
  })

  it('takes a purchase out once while its write waits on storage, over another tab’s writes', async () => {
    const shared = storage()
    const lock = sharedLock()
    const refuse = { a: false }
    const storageA: CartStorage = {
      getItem: shared.getItem,
      setItem: (k, v) => {
        if (refuse.a) throw new Error('full')
        shared.setItem(k, v)
      },
      removeItem: shared.removeItem
    }
    const tabA = createInWorldCart({ storage: () => storageA, lock, watch: () => () => {} })
    const tabB = createInWorldCart({ storage: () => shared, lock, watch: () => () => {} })
    tabA.scopeTo(A)
    tabB.scopeTo(A)
    await tabA.add(item('1'))
    await tabA.add(item('1'))
    refuse.a = true
    await tabA.applyPurchase(A, OP, bought('1'))
    await tabA.applyPurchase(A, OP, bought('1'))
    refuse.a = false
    await tabB.add(item('2'))
    await tabA.add(item('3'))
    const reloaded = createInWorldCart({ storage: () => shared, lock, watch: () => () => {} })
    reloaded.scopeTo(A)
    expect(reloaded.getSnapshot().map((l) => `${l.itemId}x${l.quantity}`).sort()).toEqual(['1x1', '2x1', '3x1'])
    // And kept as taken out: heard again after the reload, nothing more leaves.
    await reloaded.applyPurchase(A, OP, bought('1'))
    expect(reloaded.getSnapshot().find((l) => l.itemId === '1')?.quantity).toBe(1)
  })

  it('takes a purchase out under the same lock as the cart’s other changes', async () => {
    const shared = storage()
    const lock = sharedLock()
    const tabA = createInWorldCart({ storage: () => shared, lock, watch: () => () => {} })
    const tabB = createInWorldCart({ storage: () => shared, lock, watch: () => () => {} })
    tabA.scopeTo(A)
    tabB.scopeTo(A)
    await tabA.add(item('1'))
    await tabA.add(item('1'))
    // Both in flight together: a purchase of one heard in tab A, an addition in tab B.
    await Promise.all([tabA.applyPurchase(A, OP, bought('1')), tabB.add(item('2'))])
    const reloaded = createInWorldCart({ storage: () => shared, lock, watch: () => () => {} })
    reloaded.scopeTo(A)
    expect(reloaded.getSnapshot().map((l) => `${l.itemId}x${l.quantity}`)).toEqual(['1x1', '2x1'])
  })

  it('never goes below one of an item, and removes it on its own', async () => {
    const s = storage()
    const cart = createInWorldCart({ storage: () => s })
    cart.scopeTo(A)
    await cart.add(item('1'))
    await cart.setQuantity(item('1'), 0)
    expect(cart.getSnapshot()[0].quantity).toBe(1)
    await cart.remove(item('1'))
    expect(cart.getSnapshot()).toEqual([])
  })

  const OP = '0f0e0d0c-0000-4000-8000-000000000001'
  const bought = (itemId: string, quantity = 1) => [{ contract: '0x' + '11'.repeat(20), itemId, quantity }]

  it('lets go of as many as a purchase bought, and keeps what was added since', async () => {
    const s = storage()
    const cart = createInWorldCart({ storage: () => s })
    cart.scopeTo(A)
    await cart.add(item('1'))
    await cart.add(item('1'))
    await cart.add(item('2'))
    await cart.applyPurchase(A, OP, bought('1'))
    expect(cart.getSnapshot().map((l) => `${l.itemId}x${l.quantity}`)).toEqual(['1x1', '2x1'])
    await cart.applyPurchase(A, '0f0e0d0c-0000-4000-8000-000000000002', [...bought('1'), ...bought('2', 3)])
    expect(cart.getSnapshot()).toEqual([])
  })

  it('takes a purchase out once, however often it is heard, across pages', async () => {
    const s = storage()
    const cart = createInWorldCart({ storage: () => s })
    cart.scopeTo(A)
    await cart.add(item('1'))
    await cart.applyPurchase(A, OP, bought('1'))
    await cart.add(item('1'))
    await cart.applyPurchase(A, OP, bought('1'))
    expect(cart.getSnapshot().map((l) => `${l.itemId}x${l.quantity}`)).toEqual(['1x1'])
    const reloaded = createInWorldCart({ storage: () => s })
    reloaded.scopeTo(A)
    await reloaded.applyPurchase(A, OP, bought('1'))
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

  it('keeps a purchase and its taking-out together: a write that fails keeps neither, and the replay takes it out once', async () => {
    const f = flaky()
    const cart = createInWorldCart({ storage: f.storage })
    cart.scopeTo(A)
    await cart.add(item('1'))
    await cart.add(item('1'))
    f.state.refuse = true
    await cart.applyPurchase(A, OP, bought('1'))
    expect(cart.getSnapshot()[0].quantity).toBe(1)
    // Within the page, what it couldn't keep is what it knows: heard again, nothing more leaves.
    await cart.applyPurchase(A, OP, bought('1'))
    expect(cart.getSnapshot()[0].quantity).toBe(1)
    // After a reload, neither was kept: the replay (the shell's status) takes it out, once.
    f.state.refuse = false
    const reloaded = createInWorldCart({ storage: f.storage })
    reloaded.scopeTo(A)
    expect(reloaded.getSnapshot()[0].quantity).toBe(2)
    await reloaded.applyPurchase(A, OP, bought('1'))
    expect(reloaded.getSnapshot()[0].quantity).toBe(1)
    const again = createInWorldCart({ storage: f.storage })
    again.scopeTo(A)
    await again.applyPurchase(A, OP, bought('1'))
    expect(again.getSnapshot()[0].quantity).toBe(1)
  })

  it('keeps an account’s record in memory when its write fails, so a replay for it takes nothing more', async () => {
    const f = flaky()
    const cart = createInWorldCart({ storage: f.storage })
    cart.scopeTo(A)
    await cart.add(item('1'))
    await cart.add(item('1'))
    cart.scopeTo(B)
    f.state.refuse = true
    await cart.applyPurchase(A, OP, bought('1'))
    await cart.applyPurchase(A, OP, bought('1'))
    cart.scopeTo(A)
    expect(cart.getSnapshot()[0].quantity).toBe(1)
  })

  it('remembers what was taken out even once the cart is empty', async () => {
    const s = storage()
    const cart = createInWorldCart({ storage: () => s })
    cart.scopeTo(A)
    await cart.add(item('1'))
    await cart.applyPurchase(A, OP, bought('1'))
    expect(cart.getSnapshot()).toEqual([])
    const reloaded = createInWorldCart({ storage: () => s })
    reloaded.scopeTo(A)
    await reloaded.add(item('1'))
    await reloaded.applyPurchase(A, OP, bought('1'))
    expect(reloaded.getSnapshot().map((l) => `${l.itemId}x${l.quantity}`)).toEqual(['1x1'])
  })

  it('takes a purchase out of its buyer’s cart, whichever account is active', async () => {
    const s = storage()
    const cart = createInWorldCart({ storage: () => s })
    cart.scopeTo(A)
    await cart.add(item('1'))
    cart.scopeTo(B)
    await cart.add(item('1'))
    await cart.applyPurchase(A, OP, bought('1'))
    expect(cart.getSnapshot().map((l) => l.itemId)).toEqual(['1'])
    cart.scopeTo(A)
    expect(cart.getSnapshot()).toEqual([])
  })

  it('keeps each account’s cart apart, across pages', async () => {
    const s = storage()
    const first = createInWorldCart({ storage: () => s })
    first.scopeTo(A)
    await first.add(item('1'))
    first.scopeTo(B)
    expect(first.getSnapshot()).toEqual([])
    await first.add(item('2'))
    const reloaded = createInWorldCart({ storage: () => s })
    reloaded.scopeTo(A)
    expect(reloaded.getSnapshot().map((l) => l.itemId)).toEqual(['1'])
    reloaded.scopeTo(B)
    expect(reloaded.getSnapshot().map((l) => l.itemId)).toEqual(['2'])
  })

  it('makes a signed-out cart the account’s once one signs in', async () => {
    const s = storage()
    const cart = createInWorldCart({ storage: () => s })
    await cart.add(item('1'))
    cart.scopeTo(A)
    // Taken over under the cart's lock.
    await vi.waitFor(() => expect(cart.getSnapshot().map((l) => l.itemId)).toEqual(['1']))
    cart.scopeTo(null)
    expect(cart.getSnapshot()).toEqual([])
  })

  it('skips what storage holds in another shape', async () => {
    const s = storage()
    const key = `dcl-inworld-shop:cart:v2:${A}`
    s.setItem(key, JSON.stringify({ v: 2, lines: [{ contract: 'nope', itemId: '1', quantity: 1 }, { ...item('2'), quantity: 2, addedAt: 1 }, { ...item('2'), quantity: 1, addedAt: 2 }], applied: ['nope', OP] }))
    const cart = createInWorldCart({ storage: () => s })
    cart.scopeTo(A)
    expect(cart.getSnapshot().map((l) => `${l.itemId}x${l.quantity}`)).toEqual(['2x2'])
    // The operation it kept is still taken out; the malformed id was dropped.
    await cart.applyPurchase(A, OP, bought('2'))
    expect(cart.getSnapshot()[0].quantity).toBe(2)
    for (const other of [JSON.stringify([{ ...item('2'), quantity: 1, addedAt: 1 }]), JSON.stringify({ v: 1, lines: [], applied: [] }), '{not json']) {
      s.setItem(key, other)
      const fresh = createInWorldCart({ storage: () => s })
      fresh.scopeTo(A)
      expect(fresh.getSnapshot()).toEqual([])
    }
  })

  it('takes a signed-out cart over within the cap, and leaves what doesn’t fit in it', async () => {
    const s = storage()
    const cart = createInWorldCart({ storage: () => s, watch: () => () => {} })
    cart.scopeTo(A)
    await cart.add(item('1', { available: 100 }))
    await cart.setQuantity(item('1'), 20)
    cart.scopeTo(null)
    await cart.add(item('2', { available: 100 }))
    await cart.setQuantity(item('2'), 20)
    cart.scopeTo(A)
    await vi.waitFor(() => expect(cart.getSnapshot().map((l) => `${l.itemId}x${l.quantity}`)).toEqual(['1x20', '2x10']))
    // The ten that didn't fit are still the signed-out cart's.
    cart.scopeTo(null)
    expect(cart.getSnapshot().map((l) => `${l.itemId}x${l.quantity}`)).toEqual(['2x10'])
  })

  /** A shared storage whose writes to the keys `refused` names throw. */
  function guarded() {
    const shared = storage()
    const refused = new Set<string>()
    const make = (): CartStorage => ({
      getItem: shared.getItem,
      setItem: (k, v) => {
        if (refused.has(k)) throw new Error('full')
        shared.setItem(k, v)
      },
      removeItem: (k) => {
        if (refused.has(k)) throw new Error('full')
        shared.removeItem(k)
      }
    })
    return { shared, refused, make }
  }
  const ACCOUNT_KEY = `dcl-inworld-shop:cart:v2:${A}`
  const GUEST_KEY = 'dcl-inworld-shop:cart:v2:guest'
  const units = (cart: { getSnapshot(): readonly { itemId: string; quantity: number }[] }) => cart.getSnapshot().map((l) => `${l.itemId}x${l.quantity}`)

  it('leaves a signed-out cart as it is until the account’s cart that took it over is written', async () => {
    const g = guarded()
    const cart = createInWorldCart({ storage: g.make, watch: () => () => {} })
    await cart.add(item('1'))
    g.refused.add(ACCOUNT_KEY)
    cart.scopeTo(A)
    // The account's write didn't land: a reload still finds the signed-out cart, and the account has none of it.
    await vi.waitFor(() => expect(JSON.parse(g.shared.getItem(GUEST_KEY) ?? '{}').transfer).toBeTruthy())
    const meanwhile = createInWorldCart({ storage: () => g.shared, watch: () => () => {} })
    expect(units(meanwhile)).toEqual(['1x1'])
    // The next write that lands takes it over, once.
    g.refused.clear()
    await cart.add(item('2'))
    expect(units(cart).sort()).toEqual(['1x1', '2x1'])
    const reloaded = createInWorldCart({ storage: () => g.shared, watch: () => () => {} })
    expect(reloaded.getSnapshot()).toEqual([])
    reloaded.scopeTo(A)
    expect(units(reloaded).sort()).toEqual(['1x1', '2x1'])
  })

  it('takes a signed-out cart over once when signing in is tried again after a refused write', async () => {
    const g = guarded()
    const cart = createInWorldCart({ storage: g.make, watch: () => () => {} })
    await cart.add(item('1'))
    g.refused.add(ACCOUNT_KEY)
    cart.scopeTo(A)
    await vi.waitFor(() => expect(g.shared.getItem(GUEST_KEY)).toContain('transfer'))
    g.refused.clear()
    cart.scopeTo(null)
    cart.scopeTo(A)
    await vi.waitFor(() => expect(units(cart)).toEqual(['1x1']))
    const reloaded = createInWorldCart({ storage: () => g.shared, watch: () => () => {} })
    reloaded.scopeTo(A)
    expect(units(reloaded)).toEqual(['1x1'])
  })

  it('takes a signed-out cart over once when another tab took it over first', async () => {
    const g = guarded()
    const lock = sharedLock()
    const tabA = createInWorldCart({ storage: g.make, lock, watch: () => () => {} })
    const tabB = createInWorldCart({ storage: () => g.shared, lock, watch: () => () => {} })
    await tabA.add(item('1'))
    // Tab A can't write the account's cart; tab B signs in too, and takes the signed-out cart over.
    g.refused.add(ACCOUNT_KEY)
    tabA.scopeTo(A)
    tabB.scopeTo(A)
    await vi.waitFor(() => expect(units(tabB)).toEqual(['1x1']))
    expect(g.shared.getItem(GUEST_KEY)).toBeNull()
    g.refused.clear()
    await tabA.add(item('2'))
    const reloaded = createInWorldCart({ storage: () => g.shared, lock, watch: () => () => {} })
    reloaded.scopeTo(A)
    expect(units(reloaded).sort()).toEqual(['1x1', '2x1'])
  })

  it('lets go of a transfer the account already took, when letting go of it was refused', async () => {
    const g = guarded()
    const cart = createInWorldCart({ storage: g.make, watch: () => () => {} })
    await cart.add(item('1'))
    await cart.add(item('1'))
    // The account takes the two; the signed-out cart can't let go of them yet.
    const write = g.make()
    let accountWritten = false
    const failGuestAfterAccount: CartStorage = {
      getItem: write.getItem,
      setItem: (k, v) => {
        if (k === GUEST_KEY && accountWritten) throw new Error('full')
        write.setItem(k, v)
        if (k === ACCOUNT_KEY) accountWritten = true
      },
      removeItem: (k) => {
        if (k === GUEST_KEY && accountWritten) throw new Error('full')
        write.removeItem(k)
      }
    }
    const signing = createInWorldCart({ storage: () => failGuestAfterAccount, watch: () => () => {} })
    signing.scopeTo(A)
    await vi.waitFor(() => expect(units(signing)).toEqual(['1x2']))
    expect(units(createInWorldCart({ storage: () => g.shared, watch: () => () => {} }))).toEqual(['1x2'])
    // Signing in again finds the transfer taken: it lets go of it, adding nothing.
    const again = createInWorldCart({ storage: () => g.shared, watch: () => () => {} })
    again.scopeTo(A)
    await vi.waitFor(() => expect(g.shared.getItem(GUEST_KEY)).toBeNull())
    expect(units(again)).toEqual(['1x2'])
  })

  it('never lets another account take a transfer one account took, when letting go of it was refused', async () => {
    const g = guarded()
    const lock = sharedLock()
    const first = createInWorldCart({ storage: g.make, lock, watch: () => () => {} })
    await first.add(item('1'))
    // A takes it, and the signed-out cart can't let go of it.
    let taken = false
    const refuseGuestOnceTaken: CartStorage = {
      getItem: g.shared.getItem,
      setItem: (k, v) => {
        if (k === GUEST_KEY && taken) throw new Error('full')
        g.shared.setItem(k, v)
        if (k === ACCOUNT_KEY) taken = true
      },
      removeItem: (k) => {
        if (k === GUEST_KEY && taken) throw new Error('full')
        g.shared.removeItem(k)
      }
    }
    const tabA = createInWorldCart({ storage: () => refuseGuestOnceTaken, lock, watch: () => () => {} })
    tabA.scopeTo(A)
    await vi.waitFor(() => expect(units(tabA)).toEqual(['1x1']))
    expect(JSON.parse(g.shared.getItem(GUEST_KEY) ?? '{}').transfer).toMatchObject({ to: A })
    // Storage works again; B signs in: the transfer is A's, so B takes nothing, and the signed-out cart lets go.
    const tabB = createInWorldCart({ storage: () => g.shared, lock, watch: () => () => {} })
    tabB.scopeTo(B)
    await vi.waitFor(() => expect(g.shared.getItem(GUEST_KEY)).toBeNull())
    expect(units(tabB)).toEqual([])
    const reloaded = createInWorldCart({ storage: () => g.shared, lock, watch: () => () => {} })
    reloaded.scopeTo(A)
    expect(units(reloaded)).toEqual(['1x1'])
  })

  it('lets another account take a transfer the first never took', async () => {
    const g = guarded()
    const lock = sharedLock()
    const cart = createInWorldCart({ storage: g.make, lock, watch: () => () => {} })
    await cart.add(item('1'))
    g.refused.add(ACCOUNT_KEY)
    cart.scopeTo(A)
    await vi.waitFor(() => expect(JSON.parse(g.shared.getItem(GUEST_KEY) ?? '{}').transfer).toMatchObject({ to: A }))
    g.refused.clear()
    const other = createInWorldCart({ storage: () => g.shared, lock, watch: () => () => {} })
    other.scopeTo(B)
    await vi.waitFor(() => expect(units(other)).toEqual(['1x1']))
    expect(g.shared.getItem(GUEST_KEY)).toBeNull()
    const reloaded = createInWorldCart({ storage: () => g.shared, lock, watch: () => () => {} })
    reloaded.scopeTo(A)
    expect(units(reloaded)).toEqual([])
  })

  it('writes under the origin’s Web Lock when the browser has one', async () => {
    const request = vi.fn((_name: string, fn: () => unknown) => Promise.resolve(fn()))
    Object.defineProperty(navigator, 'locks', { value: { request }, configurable: true })
    try {
      const s = storage()
      const cart = createInWorldCart({ storage: () => s, watch: () => () => {} })
      cart.scopeTo(A)
      expect(await cart.add(item('1'))).toBe(true)
      expect(request).toHaveBeenCalledWith('dcl-inworld-shop:cart', expect.any(Function))
    } finally {
      Reflect.deleteProperty(navigator, 'locks')
    }
  })

  it('shows another tab’s write as the window hears of it', async () => {
    const s = storage()
    const cart = createInWorldCart({ storage: () => s })
    cart.scopeTo(A)
    const other = createInWorldCart({ storage: () => s, watch: () => () => {} })
    other.scopeTo(A)
    await other.add(item('1'))
    expect(cart.getSnapshot()).toEqual([])
    window.dispatchEvent(new StorageEvent('storage', { key: `dcl-inworld-shop:cart:v2:${A}` }))
    expect(cart.getSnapshot().map((l) => l.itemId)).toEqual(['1'])
  })

  it('stays in memory when storage refuses it', async () => {
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
    expect(await cart.add(item('1'))).toBe(true)
    expect(cart.getSnapshot()).toHaveLength(1)
  })
})
