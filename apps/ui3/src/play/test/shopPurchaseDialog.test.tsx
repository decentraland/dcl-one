// The purchase dialog's steps (features/shop/ui/ShopPurchaseDialog) and its host over the app
// (shell/shop/overlay): what each step says and offers, and where the keyboard goes.
import { describe, it, expect, vi, afterEach } from 'vitest'
import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react'
import { ShopPurchaseDialog, type ShopPurchaseStep } from '../features/shop/ui'
import { ARM_MS } from '../features/shop/ui/ShopPurchaseDialog'
import { createShopOverlay } from '../shell/shop/overlay'

const item = { name: 'Golden Armor', creator: 'Aurora', rarity: 'epic', credits: 500 }
const name = { kind: 'name' as const, name: 'Mojito', creator: '', rarity: '', credits: 94 }

function show(view: ShopPurchaseStep) {
  const handlers = { onClose: vi.fn(), onConfirm: vi.fn(), onBuyCredits: vi.fn(), onBackpack: vi.fn(), onAssign: vi.fn(), onPreview: vi.fn(), onRetry: vi.fn(), onSignIn: vi.fn() }
  render(<ShopPurchaseDialog view={view} {...handlers} />)
  return handlers
}

describe('the purchase dialog', () => {
  it('Buy Item: the balance, the item and its price, then CANCEL or BUY ITEM', () => {
    vi.useFakeTimers()
    const h = show({ step: 'confirm', items: [item], balance: 700 })
    const dialog = screen.getByRole('dialog', { name: 'Buy Item' })
    expect(dialog.textContent).toContain('Your Credits Balance:700')
    expect(dialog.textContent).toContain('Golden Armor')
    expect(dialog.textContent).toContain('Aurora')
    expect(dialog.textContent).toContain('500')
    // A click the moment it appears (meant for what was there before) spends nothing.
    fireEvent.click(screen.getByRole('button', { name: 'Buy item' }))
    expect(h.onConfirm).not.toHaveBeenCalled()
    act(() => void vi.advanceTimersByTime(ARM_MS))
    fireEvent.click(screen.getByRole('button', { name: 'Buy item' }))
    vi.useRealTimers()
    expect(h.onConfirm).toHaveBeenCalledTimes(1)
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }))
    expect(h.onClose).toHaveBeenCalled()
  })

  it('Insufficient Funds: how many credits are missing, and BUY CREDITS', () => {
    const h = show({ step: 'insufficient', items: [item], balance: 200, need: 300 })
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
    const h = show({ step: 'success', items: [item] })
    expect(screen.getByRole('dialog', { name: 'Purchase successful!' }).textContent).toContain('New')
    fireEvent.click(screen.getByRole('button', { name: 'Go to backpack' }))
    expect(h.onBackpack).toHaveBeenCalled()
    fireEvent.click(screen.getByRole('button', { name: 'Continue shopping' }))
    expect(h.onClose).toHaveBeenCalled()
  })

  it('the Error modal: what happened, then CANCEL and TRY AGAIN when it may be tried again', () => {
    const h = show({ step: 'failed', message: 'You declined the signature.', retry: true })
    expect(screen.getByRole('dialog', { name: 'Oops! Something went wrong' })).toBeInTheDocument()
    expect(screen.getByRole('alert').textContent).toBe("We couldn't complete your purchase this time. You declined the signature.")
    fireEvent.click(screen.getByRole('button', { name: 'Try again' }))
    expect(h.onRetry).toHaveBeenCalled()
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }))
    expect(h.onClose).toHaveBeenCalled()
  })

  it('the Error modal offers only CLOSE when it can’t be tried again', () => {
    show({ step: 'failed', message: 'The transaction failed on-chain. Nothing was bought.', retry: false })
    expect(screen.getByRole('button', { name: 'Close' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Try again' })).toBeNull()
  })

  it('Buy N Items: every unit a row, counted in the title, the list scrolling past two', () => {
    const items = [item, { ...item, name: 'Silver Boots' }, { ...item, name: 'Silver Boots' }]
    show({ step: 'confirm', items, balance: 7000 })
    const dialog = screen.getByRole('dialog', { name: 'Buy 3 Items' })
    expect(screen.getByRole('list', { name: '3 items' }).children).toHaveLength(3)
    expect(dialog.textContent).toContain('Silver Boots')
    expect(screen.getByRole('button', { name: 'Buy items' })).toBeInTheDocument()
  })

  it('lists two items as rows, with nothing to scroll', () => {
    show({ step: 'confirm', items: [item, item], balance: 7000 })
    expect(screen.getByRole('dialog', { name: 'Buy 2 Items' })).toBeInTheDocument()
    expect(screen.queryByRole('list')).toBeNull()
  })

  it('Insufficient Funds for several items: what is missing for all of them', () => {
    show({ step: 'insufficient', items: [item, item, item], balance: 200, need: 1300 })
    expect(screen.getByRole('dialog', { name: 'Insufficient Funds' }).textContent).toContain('You will need to buy 1,300 Credits to purchase these items.')
  })

  it('counts the units while loading and processing', () => {
    show({ step: 'loading', count: 3 })
    expect(screen.getByRole('dialog', { name: 'Buy 3 Items' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Buy items' })).toBeDisabled()
  })

  it('Purchase successful: three of what was bought in view, the rest a scroll away, each tagged NEW', () => {
    show({ step: 'success', items: [item, item, item] })
    expect(screen.queryByRole('list')).toBeNull()
    cleanup()
    show({ step: 'success', items: [item, item, item, item, item] })
    const row = screen.getByRole('list', { name: '5 items bought' })
    expect(row.querySelectorAll('[data-rarity]')).toHaveLength(5)
    expect(screen.getAllByText('New')).toHaveLength(5)
  })

  it('Buy Name: the balance, the NAME and its price, then CANCEL or BUY NAME', () => {
    vi.useFakeTimers()
    const h = show({ step: 'confirm', items: [name], balance: 700 })
    const dialog = screen.getByRole('dialog', { name: 'Buy Name' })
    expect(dialog.textContent).toContain('Your Credits Balance:700')
    expect(dialog.textContent).toContain('Mojito')
    expect(dialog.textContent).toContain('Decentraland Name')
    expect(dialog.textContent).toContain('94')
    // A NAME has no rarity.
    expect(dialog.textContent).not.toContain('base')
    act(() => void vi.advanceTimersByTime(ARM_MS))
    fireEvent.click(screen.getByRole('button', { name: 'Buy name' }))
    vi.useRealTimers()
    expect(h.onConfirm).toHaveBeenCalledTimes(1)
  })

  it('Insufficient Funds for a NAME: what is missing to purchase this Name', () => {
    const h = show({ step: 'insufficient', items: [name], balance: 24, need: 70 })
    expect(screen.getByRole('dialog', { name: 'Insufficient Funds' }).textContent).toContain('You will need to buy 70 Credits to purchase this Name.')
    fireEvent.click(screen.getByRole('button', { name: 'Buy credits' }))
    expect(h.onBuyCredits).toHaveBeenCalled()
  })

  it('names a NAME while loading and processing', () => {
    show({ step: 'loading', subject: 'name' })
    expect(screen.getByRole('dialog', { name: 'Buy Name' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Buy name' })).toBeDisabled()
    cleanup()
    show({ step: 'processing', balance: 700, subject: 'name' })
    expect(screen.getByRole('dialog', { name: 'Buy Name' })).toBeInTheDocument()
  })

  it('Purchase successful for a NAME: the NAME tagged NEW, CONTINUE SHOPPING or ASSIGN TO AVATAR', () => {
    const h = show({ step: 'success', items: [name] })
    const dialog = screen.getByRole('dialog', { name: 'Purchase successful!' })
    expect(dialog.textContent).toContain('New')
    expect(screen.queryByRole('button', { name: 'Go to backpack' })).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'Assign to avatar' }))
    expect(h.onAssign).toHaveBeenCalled()
    expect(h.onBackpack).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole('button', { name: 'Continue shopping' }))
    expect(h.onClose).toHaveBeenCalled()
  })

  describe('a simulated NAME purchase', () => {
    const AHEAD = 'Simulation: no Credits will be spent, and this NAME won’t be registered or reserved.'
    const DONE = 'This was a simulation: no Credits were spent, and this NAME wasn’t registered or reserved.'

    it('says so before it runs, and its button simulates', () => {
      vi.useFakeTimers()
      const h = show({ step: 'confirm', items: [name], balance: 700, simulation: true })
      const dialog = screen.getByRole('dialog', { name: 'Buy Name' })
      expect(within(dialog).getByRole('note')).toHaveTextContent(AHEAD)
      expect(screen.queryByRole('button', { name: 'Buy name' })).toBeNull()
      act(() => void vi.advanceTimersByTime(ARM_MS))
      fireEvent.click(screen.getByRole('button', { name: 'Simulate purchase' }))
      vi.useRealTimers()
      expect(h.onConfirm).toHaveBeenCalledTimes(1)
    })

    it('says so while it loads, short of Credits, and while it runs', () => {
      show({ step: 'loading', subject: 'name', simulation: true })
      expect(screen.getByRole('note')).toHaveTextContent(AHEAD)
      expect(screen.getByRole('button', { name: 'Simulate purchase' })).toBeDisabled()
      cleanup()
      show({ step: 'insufficient', items: [name], balance: 24, need: 70, simulation: true })
      expect(screen.getByRole('note')).toHaveTextContent(AHEAD)
      cleanup()
      show({ step: 'processing', balance: 700, subject: 'name', simulation: true })
      expect(screen.getByRole('note')).toHaveTextContent(AHEAD)
      expect(screen.getByRole('status', { name: 'Simulating your purchase' })).toBeInTheDocument()
    })

    it('ends as a simulation, not a purchase: no NEW tag, and PREVIEW ON AVATAR instead of ASSIGN TO AVATAR', () => {
      const h = show({ step: 'success', items: [name], simulation: true })
      expect(screen.queryByRole('dialog', { name: 'Purchase successful!' })).toBeNull()
      const dialog = screen.getByRole('dialog', { name: 'Simulation complete' })
      expect(within(dialog).getByRole('note')).toHaveTextContent(DONE)
      expect(dialog.textContent).not.toContain('New')
      expect(screen.queryByRole('button', { name: 'Assign to avatar' })).toBeNull()
      fireEvent.click(screen.getByRole('button', { name: 'Preview on avatar' }))
      expect(h.onPreview).toHaveBeenCalledTimes(1)
      expect(h.onAssign).not.toHaveBeenCalled()
    })

    it('says so when it fails', () => {
      show({ step: 'failed', message: 'Your sign-in changed meanwhile. Open the NAME again.', retry: false, simulation: true })
      const dialog = screen.getByRole('dialog', { name: 'Oops! Something went wrong' })
      expect(within(dialog).getByRole('note')).toHaveTextContent(DONE)
      expect(within(dialog).getByRole('alert')).toHaveTextContent("We couldn't complete the simulation this time.")
    })

    it('is said only for a simulation', () => {
      show({ step: 'confirm', items: [name], balance: 700 })
      expect(screen.queryByRole('note')).toBeNull()
      cleanup()
      show({ step: 'success', items: [name] })
      expect(screen.queryByRole('note')).toBeNull()
      expect(screen.getByRole('dialog', { name: 'Purchase successful!' }).textContent).toContain('New')
    })
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
    await act(async () => ui.show({ step: 'confirm', items: [item], balance: 700 }, { onClose, onConfirm }))
    expect(document.activeElement?.textContent).toBe('Cancel')
    await act(async () => ui.show({ step: 'success', items: [item] }, { onClose, onBackpack: vi.fn() }))
    expect(document.activeElement?.textContent).toBe('Continue shopping')
    await act(async () => ui.show({ step: 'confirm', items: [item], balance: 700 }, { onClose, onConfirm }))
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
    await act(async () => pointer.show({ step: 'confirm', items: [item], balance: 700 }, { onClose: vi.fn(), onConfirm: vi.fn() }, { pointer: true }))
    expect(focus).toHaveBeenLastCalledWith()
    await act(async () => pointer.hide())
    document.body.innerHTML = ''
    for (const how of [{ pointer: false }, undefined]) {
      const keyboard = createShopOverlay(vi.fn())
      await act(async () => keyboard.show({ step: 'confirm', items: [item], balance: 700 }, { onClose: vi.fn(), onConfirm: vi.fn() }, how))
      expect(focus).toHaveBeenLastCalledWith()
      await act(async () => keyboard.hide())
      document.body.innerHTML = ''
    }
    focus.mockRestore()
  })

  it('takes the focus in again when a view of the same step drops the focused button', async () => {
    const ui = createShopOverlay(vi.fn())
    await act(async () => ui.show({ step: 'failed', message: 'The wallet could not sign.', retry: true }, { onClose: vi.fn(), onRetry: vi.fn() }))
    screen.getByRole('button', { name: 'Try again' }).focus()
    await act(async () => ui.show({ step: 'failed', message: 'This purchase didn’t go through.', retry: false }, { onClose: vi.fn() }))
    expect(screen.queryByRole('button', { name: 'Try again' })).toBeNull()
    expect(document.activeElement?.textContent).toBe('Close')
  })

  it('leaves the page under it, the app’s frame included, inert while it shows', async () => {
    const app = document.createElement('div')
    document.body.appendChild(app)
    const ui = createShopOverlay(vi.fn())
    await act(async () => ui.show({ step: 'confirm', items: [item], balance: 700 }, { onClose: vi.fn(), onConfirm: vi.fn() }))
    expect(app.inert).toBe(true)
    await act(async () => ui.hide())
    expect(app.inert).toBe(false)
  })

  it('keeps Escape from closing a purchase that is processing', async () => {
    const ui = createShopOverlay(vi.fn())
    const onClose = vi.fn()
    await act(async () => ui.show({ step: 'processing', balance: 20 }, { onClose }))
    fireEvent.keyDown(screen.getByRole('dialog'), { key: 'Escape' })
    expect(onClose).not.toHaveBeenCalled()
  })

  it('reaches a success row that scrolls with Tab, after the first focus lands on CONTINUE SHOPPING', async () => {
    const ui = createShopOverlay(vi.fn())
    const many = Array.from({ length: 5 }, (_, i) => ({ ...item, name: `Item ${i + 1}` }))
    await act(async () => ui.show({ step: 'success', items: many }, { onClose: vi.fn(), onBackpack: vi.fn() }))
    const row = screen.getByRole('list', { name: '5 items bought' })
    expect(screen.getAllByRole('listitem').map((el) => el.getAttribute('aria-label'))).toEqual(['Item 1', 'Item 2', 'Item 3', 'Item 4', 'Item 5'])
    expect(document.activeElement?.textContent).toBe('Continue shopping')
    const visited: (string | null)[] = []
    for (let i = 0; i < 3; i++) {
      fireEvent.keyDown(document.activeElement!, { key: 'Tab' })
      visited.push(document.activeElement === row ? 'row' : document.activeElement?.textContent ?? null)
    }
    // The order is the dialog's: the row (it comes first), then the buttons again.
    expect(visited).toContain('row')
    expect(new Set(visited)).toEqual(new Set(['row', 'Continue shopping', 'Go to backpack']))
  })

  it('keeps Tab within the dialog', async () => {
    const ui = createShopOverlay(vi.fn())
    await act(async () => ui.show({ step: 'confirm', items: [item], balance: 700 }, { onClose: vi.fn(), onConfirm: vi.fn() }))
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
