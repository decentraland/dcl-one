import { describe, expect, it, vi } from 'vitest'
import type { Eip1193Provider } from '../../data/auth/wallet'
import { createDialogSlot, promptingWallet, type DialogOwner, type ShellPurchaseView } from '../shell/shop/purchase'

/** A wallet whose every request waits for `answer()` / `refuse()`, as a buyer deciding in their wallet would. */
function heldWallet() {
  let answer: (v: unknown) => void = () => {}
  let refuse: (e: unknown) => void = () => {}
  const wallet: Eip1193Provider = {
    request: vi.fn(
      () =>
        new Promise((resolve, reject) => {
          answer = resolve
          refuse = reject
        })
    )
  }
  return { wallet, answer: (v: unknown) => answer(v), refuse: (e: unknown) => refuse(e) }
}

function setup() {
  const shown: ShellPurchaseView[] = []
  const slot = createDialogSlot({ show: (view) => void shown.push(view), hide: () => {} })
  const op: DialogOwner = { state: 'processing' }
  const processing: ShellPurchaseView = { step: 'processing', balance: 700 }
  slot.show(op, processing, { onClose: () => {} })
  return { shown, slot, op, processing }
}

const SIGN = { method: 'eth_signTypedData_v4', params: ['0xabc', '{}'] }
const tick = () => new Promise((r) => setTimeout(r, 0))

describe('the wallet prompt in the in-world purchase', () => {
  it('says to confirm in the wallet while it asks, then shows the step it showed before, with the answer untouched', async () => {
    const { shown, slot, op, processing } = setup()
    const held = heldWallet()
    const signing = promptingWallet(slot, op, held.wallet, 'web3').request(SIGN)
    await tick()
    expect(slot.shown()?.view).toEqual({ step: 'wallet' })
    held.answer('0xsig')
    await expect(signing).resolves.toBe('0xsig')
    expect(slot.shown()?.view).toBe(processing)
    expect(shown.map((v) => v.step)).toEqual(['processing', 'wallet', 'processing'])
    expect(held.wallet.request).toHaveBeenCalledWith(SIGN)
  })

  it('never draws the step before over a newer one shown while the wallet asked (pending, failed)', async () => {
    for (const newer of [{ step: 'pending' }, { step: 'failed', message: 'It failed.', retry: false }] as ShellPurchaseView[]) {
      const { slot, op } = setup()
      const held = heldWallet()
      const signing = promptingWallet(slot, op, held.wallet, 'web3').request(SIGN)
      await tick()
      slot.show(op, newer, { onClose: () => {} })
      held.answer('0xsig')
      await signing
      expect(slot.shown()?.view).toBe(newer)
    }
  })

  it('never draws over another purchase that took the dialog while the wallet asked', async () => {
    const { slot, op } = setup()
    const held = heldWallet()
    const signing = promptingWallet(slot, op, held.wallet, 'web3').request(SIGN)
    await tick()
    const other: DialogOwner = { state: 'reviewing' }
    const theirs: ShellPurchaseView = { step: 'loading' }
    slot.show(other, theirs, { onClose: () => {} })
    held.answer('0xsig')
    await signing
    expect(slot.shown()).toMatchObject({ owner: other, view: theirs })
  })

  it('passes the wallet’s refusal through as it came, and still restores the step before', async () => {
    const { slot, op, processing } = setup()
    const held = heldWallet()
    const declined = Object.assign(new Error('User rejected the request.'), { code: 4001 })
    const signing = promptingWallet(slot, op, held.wallet, 'web3').request(SIGN)
    await tick()
    held.refuse(declined)
    await expect(signing).rejects.toBe(declined)
    expect(slot.shown()?.view).toBe(processing)
  })

  it('shows nothing for an in-app wallet, for a read, or while the purchase isn’t the one shown', async () => {
    const { shown, slot, op } = setup()
    const managed = heldWallet()
    expect(promptingWallet(slot, op, managed.wallet, 'managed')).toBe(managed.wallet)

    const reads: Eip1193Provider = { request: vi.fn(async () => ['0xbuyer']) }
    await promptingWallet(slot, op, reads, 'web3').request({ method: 'eth_accounts' })

    const other: DialogOwner = { state: 'processing' }
    const held = heldWallet()
    const signing = promptingWallet(slot, other, held.wallet, 'web3').request(SIGN)
    await tick()
    held.answer('0xsig')
    await signing
    expect(shown.map((v) => v.step)).toEqual(['processing'])
  })
})
