// The purchase dialog's steps (features/shop/ui/ShopPurchaseDialog) and its host over the app
// (shell/shop/overlay): what each step says and offers, and where the keyboard goes.
import { describe, it, expect, vi, afterEach } from 'vitest'
import { act, fireEvent, render, screen } from '@testing-library/react'
import { ShopPurchaseDialog, type ShopPurchaseStep } from '../features/shop/ui'
import { createShopOverlay } from '../shell/shop/overlay'

const item = { name: 'Golden Armor', creator: 'Aurora', rarity: 'epic', credits: 500 }

function show(view: ShopPurchaseStep) {
  const handlers = { onClose: vi.fn(), onConfirm: vi.fn(), onBuyCredits: vi.fn(), onBackpack: vi.fn(), onRetry: vi.fn(), onSignIn: vi.fn() }
  render(<ShopPurchaseDialog view={view} {...handlers} />)
  return handlers
}

describe('the purchase dialog', () => {
  it('Buy Item: the balance, the item and its price, then CANCEL or BUY ITEM', () => {
    const h = show({ step: 'confirm', item, balance: 700 })
    const dialog = screen.getByRole('dialog', { name: 'Buy Item' })
    expect(dialog.textContent).toContain('Your Credits Balance:700')
    expect(dialog.textContent).toContain('Golden Armor')
    expect(dialog.textContent).toContain('Aurora')
    expect(dialog.textContent).toContain('500')
    fireEvent.click(screen.getByRole('button', { name: 'Buy item' }))
    expect(h.onConfirm).toHaveBeenCalled()
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }))
    expect(h.onClose).toHaveBeenCalled()
  })

  it('Insufficient Funds: how many credits are missing, and BUY CREDITS', () => {
    const h = show({ step: 'insufficient', item, balance: 200, need: 300 })
    expect(screen.getByRole('dialog', { name: 'Insufficient Funds' }).textContent).toContain('You will need to buy 300 Credits to purchase this item.')
    fireEvent.click(screen.getByRole('button', { name: 'Buy credits' }))
    expect(h.onBuyCredits).toHaveBeenCalled()
  })

  it('Processing: a spinner and nothing to press', () => {
    show({ step: 'processing', balance: 20 })
    expect(screen.getByRole('dialog', { name: 'Buy Item' }).getAttribute('aria-busy')).toBe('true')
    expect((screen.getByRole('button', { name: 'Processing...' }) as HTMLButtonElement).disabled).toBe(true)
    expect(screen.queryByRole('button', { name: 'Cancel' })).toBeNull()
  })

  it('Purchase successful: the item tagged NEW, CONTINUE SHOPPING or GO TO BACKPACK', () => {
    const h = show({ step: 'success', item })
    expect(screen.getByRole('dialog', { name: 'Purchase successful!' }).textContent).toContain('New')
    fireEvent.click(screen.getByRole('button', { name: 'Go to backpack' }))
    expect(h.onBackpack).toHaveBeenCalled()
    fireEvent.click(screen.getByRole('button', { name: 'Continue shopping' }))
    expect(h.onClose).toHaveBeenCalled()
  })

  it('a failure says what happened, and offers TRY AGAIN only when it may', () => {
    const h = show({ step: 'failed', message: 'You declined the signature.', retry: true })
    expect(screen.getByRole('alert').textContent).toBe('You declined the signature.')
    fireEvent.click(screen.getByRole('button', { name: 'Try again' }))
    expect(h.onRetry).toHaveBeenCalled()
  })

  it('asks a guest to sign in', () => {
    const h = show({ step: 'signIn' })
    fireEvent.click(screen.getByRole('button', { name: 'Sign in' }))
    expect(h.onSignIn).toHaveBeenCalled()
  })
})

describe('the purchase dialog over the app', () => {
  afterEach(() => {
    document.body.innerHTML = ''
  })

  it('focuses the action that spends nothing, cancels on Escape and gives the focus back once closed', async () => {
    const restore = vi.fn()
    const ui = createShopOverlay(restore)
    const onClose = vi.fn()
    const onConfirm = vi.fn()
    await act(async () => ui.show({ step: 'confirm', item, balance: 700 }, { onClose, onConfirm }))
    expect(document.activeElement?.textContent).toBe('Cancel')
    await act(async () => ui.show({ step: 'success', item }, { onClose, onBackpack: vi.fn() }))
    expect(document.activeElement?.textContent).toBe('Continue shopping')
    await act(async () => ui.show({ step: 'confirm', item, balance: 700 }, { onClose, onConfirm }))
    expect(onConfirm).not.toHaveBeenCalled()
    fireEvent.keyDown(document.activeElement!, { key: 'Escape' })
    expect(onClose).toHaveBeenCalled()
    await act(async () => ui.hide())
    expect(screen.queryByRole('dialog')).toBeNull()
    expect(restore).toHaveBeenCalled()
  })

  it('rings the first focus unless a pointer opened it, and leaves later steps to the browser', async () => {
    const focus = vi.spyOn(HTMLElement.prototype, 'focus')
    const pointer = createShopOverlay(vi.fn())
    await act(async () => pointer.show({ step: 'loading' }, { onClose: vi.fn() }, { pointer: true }))
    expect(focus).toHaveBeenLastCalledWith({ focusVisible: false })
    await act(async () => pointer.show({ step: 'confirm', item, balance: 700 }, { onClose: vi.fn(), onConfirm: vi.fn() }, { pointer: true }))
    expect(focus).toHaveBeenLastCalledWith()
    await act(async () => pointer.hide())
    document.body.innerHTML = ''
    for (const how of [{ pointer: false }, undefined]) {
      const keyboard = createShopOverlay(vi.fn())
      await act(async () => keyboard.show({ step: 'confirm', item, balance: 700 }, { onClose: vi.fn(), onConfirm: vi.fn() }, how))
      expect(focus).toHaveBeenLastCalledWith()
      await act(async () => keyboard.hide())
      document.body.innerHTML = ''
    }
    focus.mockRestore()
  })

  it('keeps Escape from closing a purchase that is processing', async () => {
    const ui = createShopOverlay(vi.fn())
    const onClose = vi.fn()
    await act(async () => ui.show({ step: 'processing', balance: 20 }, { onClose }))
    fireEvent.keyDown(screen.getByRole('dialog'), { key: 'Escape' })
    expect(onClose).not.toHaveBeenCalled()
  })

  it('keeps Tab within the dialog', async () => {
    const ui = createShopOverlay(vi.fn())
    await act(async () => ui.show({ step: 'confirm', item, balance: 700 }, { onClose: vi.fn(), onConfirm: vi.fn() }))
    const buy = screen.getByRole('button', { name: 'Buy item' })
    const cancel = screen.getByRole('button', { name: 'Cancel' })
    expect(document.activeElement).toBe(cancel)
    fireEvent.keyDown(cancel, { key: 'Tab' })
    expect(document.activeElement).toBe(buy)
    fireEvent.keyDown(buy, { key: 'Tab' })
    expect(document.activeElement).toBe(cancel)
    fireEvent.keyDown(cancel, { key: 'Tab', shiftKey: true })
    expect(document.activeElement).toBe(buy)
  })
})
