// The purchase dialog's steps (library/ui/apps/shop/ui/ShopPurchaseDialog) and its host over the app
// (shell/shop/overlay): what each step says and offers, and where the keyboard goes.
import { describe, it, expect, vi, afterEach } from 'vitest'
import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react'
import { ShopPurchaseDialog, type ShopPurchaseStep } from '@dcl/one-ui/apps/shop/ui'
import { ARM_MS } from '@dcl/one-ui/apps/hud/design'
import { createShopOverlay } from '../shell/shop/overlay'

const item = { name: 'Golden Armor', creator: 'Aurora', rarity: 'epic', credits: 500 }
const name = { kind: 'name' as const, name: 'Mojito', creator: '', rarity: '', credits: 94 }

function show(view: ShopPurchaseStep) {
  const handlers = {
    onClose: vi.fn(),
    onConfirm: vi.fn(),
    onBuyCredits: vi.fn(),
    onBackpack: vi.fn(),
    onPreview: vi.fn(),
    onRetry: vi.fn(),
    onSwitchNetwork: vi.fn(),
    onSignIn: vi.fn(),
    onPay: vi.fn(),
    onAuthorize: vi.fn(),
    onCheckAgain: vi.fn()
  }
  render(<ShopPurchaseDialog view={view} {...handlers} />)
  return handlers
}

describe('the purchase dialog', () => {
  it('Buy Item: the balance, the item and its price, then CANCEL or BUY ITEM', () => {
    vi.useFakeTimers()
    const h = show({ step: 'confirm', items: [item], balance: 700 })
    const dialog = screen.getByRole('dialog', { name: 'Buy Item' })
    expect(dialog.textContent).toContain('Your Balance:700')
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

  it('the Error modal offers CANCEL and SWITCH TO … AND RETRY, in place of TRY AGAIN, when the wallet must move', () => {
    const h = show({ step: 'failed', message: 'Your wallet has to send it on Polygon.', retry: true, switchTo: 'Polygon' })
    expect(screen.queryByRole('button', { name: 'Try again' })).toBeNull()
    fireEvent.click(screen.getByRole('button', { name: 'Switch to Polygon and retry' }))
    expect(h.onSwitchNetwork).toHaveBeenCalledTimes(1)
    expect(h.onRetry).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }))
    expect(h.onClose).toHaveBeenCalled()
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

  it('says which payment of several it is processing, and nothing for one', () => {
    show({ step: 'processing', balance: null, count: 2, progress: { at: 2, of: 3 } })
    expect(screen.getByRole('dialog', { name: 'Buy 2 Items' }).textContent).toContain('Payment 2 of 3')
    cleanup()
    show({ step: 'processing', balance: null, count: 2, progress: { at: 1, of: 1 } })
    expect(screen.getByRole('dialog', { name: 'Buy 2 Items' }).textContent).not.toContain('Payment')
  })

  it('counts the units while loading and processing', () => {
    show({ step: 'loading', count: 3 })
    expect(screen.getByRole('dialog', { name: 'Buy 3 Items' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Buy items' })).toBeDisabled()
  })

  it('Purchase successful: two rows of three of what was bought in view, the rest a scroll away, each tagged NEW', () => {
    const named = (n: number) => Array.from({ length: n }, (_, i) => ({ ...item, name: `Item ${i + 1}` }))
    show({ step: 'success', items: named(6) })
    expect(screen.queryByRole('list')).toBeNull()
    expect(screen.getAllByText('New')).toHaveLength(6)
    cleanup()
    show({ step: 'success', items: named(7) })
    const list = screen.getByRole('list', { name: '7 items bought' })
    expect(list.querySelectorAll('[data-rarity]')).toHaveLength(7)
    expect(screen.getAllByText('New')).toHaveLength(7)
  })

  it('Purchase successful, scrolling: a card of several units says how many in its label', () => {
    const keyed = Array.from({ length: 7 }, (_, i) => ({ ...item, name: `Item ${i + 1}`, key: `k${i}` }))
    show({ step: 'success', items: [keyed[0], ...keyed] })
    expect(screen.getByRole('listitem', { name: 'Item 1 x2' })).toBeInTheDocument()
    expect(screen.getByRole('listitem', { name: 'Item 2' })).toBeInTheDocument()
  })

  it('Purchase successful: units of one item as one card with their amount, a smart wearable with its badge', () => {
    show({ step: 'success', items: [{ ...item, smart: true, key: 'a' }, { ...item, smart: true, key: 'a' }, { ...item, name: 'Silver Boots', key: 'b' }] })
    const dialog = screen.getByRole('dialog', { name: 'Purchase successful!' })
    expect(dialog.querySelectorAll('[data-rarity]')).toHaveLength(2)
    expect(dialog.textContent).toContain('x2')
    expect(within(dialog).getAllByText('SMART')).toHaveLength(1)
  })

  it('Purchase successful: two items drawn alike stay two cards, and so do items with no identity', () => {
    show({ step: 'success', items: [{ ...item, key: 'a' }, { ...item, key: 'b' }, item, item] })
    const dialog = screen.getByRole('dialog', { name: 'Purchase successful!' })
    expect(dialog.querySelectorAll('[data-rarity]')).toHaveLength(4)
    expect(dialog.textContent).not.toContain('x2')
  })

  it('shows the creator’s picture beside their name when it is known', () => {
    show({ step: 'confirm', items: [{ ...item, creatorFace: 'https://example.com/face.png' }], balance: 700 })
    expect(screen.getByRole('dialog', { name: 'Buy Item' }).querySelector('img[src="https://example.com/face.png"]')).not.toBeNull()
  })

  it('Confirm in your wallet: what to do, and nothing to press while the wallet asks', () => {
    show({ step: 'wallet' })
    const dialog = screen.getByRole('dialog', { name: 'Confirm in your wallet' })
    expect(dialog.textContent).toContain('Sign the transaction in your wallet to complete the purchase. Keep this window open.')
    expect(within(dialog).getByRole('status', { name: 'Waiting for your wallet' })).toBeInTheDocument()
    expect(within(dialog).queryByRole('button')).toBeNull()
  })

  it('Confirm in your wallet keeps the way out of the step it covers: its CANCEL leaves it', () => {
    const h = show({ step: 'wallet', close: 'Cancel' })
    const dialog = screen.getByRole('dialog', { name: 'Confirm in your wallet' })
    fireEvent.click(within(dialog).getByRole('button', { name: 'Cancel' }))
    expect(h.onClose).toHaveBeenCalledTimes(1)
  })

  it('Insufficient Funds where MANA is offered: neither is enough, both ways shown off, and BUY CREDITS', () => {
    const MANA = 10n ** 18n
    const h = show({ step: 'insufficient', items: [item, item], balance: 200, need: 800, mana: { price: 100n * MANA, balance: 15n * MANA } })
    const dialog = screen.getByRole('dialog', { name: 'Insufficient Funds' })
    expect(dialog.textContent).toContain('Your Balance:20015')
    expect(dialog.textContent).toContain('Your Credits and MANA are not enough for these items. Buy Credits or add MANA to continue.')
    const boxes = within(dialog).getAllByRole('radio') as HTMLInputElement[]
    expect(boxes.map((b) => [b.disabled, b.checked])).toEqual([
      [true, false],
      [true, false]
    ])
    expect(dialog.textContent).toContain('Total: 2 Items1,000')
    fireEvent.click(screen.getByRole('button', { name: 'Buy credits' }))
    expect(h.onBuyCredits).toHaveBeenCalled()
  })

  it('Buy Name: the balance, the NAME and its price, then CANCEL or BUY NAME', () => {
    vi.useFakeTimers()
    const h = show({ step: 'confirm', items: [name], balance: 700 })
    const dialog = screen.getByRole('dialog', { name: 'Buy Name' })
    expect(dialog.textContent).toContain('Your Balance:700')
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

  it('Purchase successful for a NAME: the NAME tagged NEW and named, then CONTINUE SHOPPING and nothing past it', () => {
    const h = show({ step: 'success', items: [name] })
    const dialog = screen.getByRole('dialog', { name: 'Purchase successful!' })
    expect(dialog.textContent).toContain('New')
    expect(within(dialog).getByRole('img', { name: 'Mojito.dcl.eth' })).toBeInTheDocument()
    // Only the simulation is bought here: no button that would do nothing, and no backpack for a NAME.
    expect(screen.queryByRole('button', { name: 'Assign to avatar' })).toBeNull()
    expect(screen.queryByRole('button', { name: 'Go to backpack' })).toBeNull()
    fireEvent.click(screen.getByRole('button', { name: 'Continue shopping' }))
    expect(h.onClose).toHaveBeenCalled()
    expect(h.onBackpack).not.toHaveBeenCalled()
  })

  describe('a simulated NAME purchase', () => {
    const AHEAD = 'Simulation: no Credits will be spent, and this NAME won’t be registered or reserved.'
    const DONE = 'This was a simulation: no Credits were spent, and this NAME wasn’t registered or reserved.'

    it('says so before it runs, and its button simulates', () => {
      vi.useFakeTimers()
      const h = show({ step: 'confirm', items: [name], balance: 700, simulation: true })
      const dialog = screen.getByRole('dialog', { name: 'Buy Name' })
      expect(within(dialog).getByRole('note')).toHaveTextContent(AHEAD)
      // The row names the NAME beside its tile, so the tile itself stays out of the way.
      expect(within(dialog).getByText('Mojito')).toBeInTheDocument()
      expect(within(dialog).queryByRole('img', { name: 'Mojito.dcl.eth' })).toBeNull()
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

    it('ends as a simulation, not a purchase: the NAME it ran for, no NEW tag, and PREVIEW ON AVATAR', () => {
      const h = show({ step: 'success', items: [name], simulation: true })
      expect(screen.queryByRole('dialog', { name: 'Purchase successful!' })).toBeNull()
      const dialog = screen.getByRole('dialog', { name: 'Simulation complete' })
      expect(within(dialog).getByRole('note')).toHaveTextContent('This was a simulation: no Credits were spent, and Mojito wasn’t registered or reserved.')
      expect(within(dialog).getByRole('img', { name: 'Mojito.dcl.eth' })).toBeInTheDocument()
      expect(dialog.textContent).not.toContain('New')
      expect(screen.queryByRole('button', { name: 'Assign to avatar' })).toBeNull()
      fireEvent.click(screen.getByRole('button', { name: 'Preview on avatar' }))
      expect(h.onPreview).toHaveBeenCalledTimes(1)
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

  describe('Buy Item with its payment options', () => {
    const MANA = 10n ** 18n
    const credits = { price: 500, balance: 700, pays: true }
    const mana = { price: 5000n * MANA, balance: 7000n * MANA, pays: true }
    const box = (name: RegExp) => screen.getByRole('radio', { name }) as HTMLInputElement
    const option = (name: RegExp) => box(name).closest('label')?.textContent
    const buy = (label = 'Buy item') => screen.getByRole('button', { name: label }) as HTMLButtonElement
    const arm = () => act(() => void vi.advanceTimersByTime(ARM_MS))

    afterEach(() => vi.useRealTimers())

    it('shows the balance, the item, an option per currency with what it pays, the Credits picked first, and BUY pays with the one picked', () => {
      vi.useFakeTimers()
      const h = show({ step: 'method', items: [item], credits, mana })
      const dialog = screen.getByRole('dialog', { name: 'Buy Item' })
      expect(dialog.textContent).toContain('Your Balance:7007,000')
      expect(dialog.textContent).toContain('Golden Armor')
      expect(screen.getByRole('radiogroup', { name: 'Payment method' })).toBeInTheDocument()
      expect(option(/^Credits/)).toBe('CreditsCredits Balance:700500')
      expect(option(/^MANA/)).toBe('MANAMANA Balance:7,0005,0001 Credit = 10 MANA')
      // A single item has no total under its options.
      expect(dialog.textContent).not.toContain('Total:')
      expect(box(/^Credits/).checked).toBe(true)
      fireEvent.click(box(/^MANA/))
      expect(box(/^MANA/).checked).toBe(true)
      expect(box(/^Credits/).checked).toBe(false)
      // Armed as BUY ITEM: a click the moment it appears pays nothing.
      fireEvent.click(buy())
      expect(h.onPay).not.toHaveBeenCalled()
      arm()
      fireEvent.click(buy())
      expect(h.onPay).toHaveBeenCalledTimes(1)
      expect(h.onPay).toHaveBeenCalledWith('mana')
      fireEvent.click(screen.getByRole('button', { name: 'Cancel' }))
      expect(h.onClose).toHaveBeenCalled()
    })

    it('offers the options as radios, one picked at a time: picking the picked one again keeps it', () => {
      vi.useFakeTimers()
      const h = show({ step: 'method', items: [item], credits, mana })
      expect(screen.getByRole('radiogroup', { name: 'Payment method' })).toBeInTheDocument()
      arm()
      fireEvent.click(box(/^Credits/))
      expect(box(/^Credits/).checked).toBe(true)
      expect(box(/^MANA/).checked).toBe(false)
      fireEvent.click(buy())
      expect(h.onPay).toHaveBeenCalledWith('credits')
    })

    it('adds a cart’s total under its options, in the currency picked', () => {
      show({ step: 'method', items: [item, item], credits: { ...credits, price: 1000 }, mana: { ...mana, price: 10000n * MANA, balance: 20000n * MANA } })
      const dialog = screen.getByRole('dialog', { name: 'Buy 2 Items' })
      expect(dialog.textContent).toContain('Total: 2 Items1,000')
      fireEvent.click(box(/^MANA/))
      expect(dialog.textContent).toContain('Total: 2 Items10,000')
      expect(buy('Buy items')).toBeInTheDocument()
    })

    it('shows a Credits balance short of the price off, and picks MANA', () => {
      vi.useFakeTimers()
      const h = show({ step: 'method', items: [item], credits: { price: 500, balance: 200, pays: false }, mana })
      expect(box(/^Credits/).disabled).toBe(true)
      expect(option(/^Credits/)).toBe('CreditsCredits Balance:200500')
      expect(box(/^MANA/).checked).toBe(true)
      arm()
      fireEvent.click(buy())
      expect(h.onPay).toHaveBeenCalledWith('mana')
    })

    it('shows the Credits off with no amount, and MANA with no rate, when the Credits can’t price it', () => {
      show({ step: 'method', items: [item], credits: { price: null, balance: 700, pays: false }, mana })
      expect(box(/^Credits/).disabled).toBe(true)
      expect(option(/^Credits/)).toBe('CreditsCredits Balance:700')
      expect(option(/^MANA/)).toBe('MANAMANA Balance:7,0005,000')
    })

    describe('Credits and MANA together', () => {
      // The design's case: a 50-credit item, 20 credits held, 3,400 MANA; the balance pays 20 and MANA the other 300.
      const short = { price: 50, balance: 20, pays: false }
      const enough = { price: 500n * MANA, balance: 3400n * MANA, pays: true }
      const split = { credits: 20, mana: 300n * MANA }
      // Where both may go together the options are real checkboxes, each its own.
      const box = (name: RegExp) => screen.getByRole('checkbox', { name }) as HTMLInputElement
      const option = (name: RegExp) => box(name).closest('label')?.textContent
      const radio = (name: RegExp) => screen.getByRole('radio', { name }) as HTMLInputElement

      it('ticks both by default, each with its part, and BUY pays with both', () => {
        vi.useFakeTimers()
        const h = show({ step: 'method', items: [item], credits: short, mana: enough, split })
        expect(box(/^Credits/).checked).toBe(true)
        expect(box(/^MANA/).checked).toBe(true)
        expect(option(/^Credits/)).toBe('CreditsCredits Balance:2020')
        expect(option(/^MANA/)).toBe('MANAMANA Balance:3,4003001 Credit = 10 MANA')
        arm()
        fireEvent.click(buy())
        expect(h.onPay).toHaveBeenCalledWith('split')
      })

      it('ticks each box on its own: Credits alone that can’t pay holds BUY, MANA alone pays its whole price', () => {
        vi.useFakeTimers()
        const h = show({ step: 'method', items: [item], credits: short, mana: enough, split })
        arm()
        fireEvent.click(box(/^MANA/))
        expect(box(/^Credits/).checked).toBe(true)
        expect(box(/^MANA/).checked).toBe(false)
        expect(option(/^Credits/)).toBe('CreditsCredits Balance:2050')
        expect(buy().disabled).toBe(true)
        fireEvent.click(box(/^MANA/))
        fireEvent.click(box(/^Credits/))
        expect(box(/^MANA/).checked).toBe(true)
        expect(option(/^MANA/)).toBe('MANAMANA Balance:3,4005001 Credit = 10 MANA')
        fireEvent.click(buy())
        expect(h.onPay).toHaveBeenCalledWith('mana')
      })

      it('lets MANA that can’t pay alone be ticked as the rest of a split, and holds BUY while it is alone', () => {
        vi.useFakeTimers()
        const h = show({ step: 'method', items: [item], credits: short, mana: { ...enough, balance: 400n * MANA, pays: false }, split })
        arm()
        expect(box(/^MANA/).disabled).toBe(false)
        fireEvent.click(box(/^Credits/))
        expect(box(/^MANA/).checked).toBe(true)
        expect(buy().disabled).toBe(true)
        fireEvent.click(buy())
        expect(h.onPay).not.toHaveBeenCalled()
      })

      it('drops both when a new read no longer offers them together, and doesn’t tick them back', () => {
        vi.useFakeTimers()
        const onPay = vi.fn()
        const view = (offered: boolean): ShopPurchaseStep => ({ step: 'method', items: [item], credits: short, mana: { ...enough, balance: 200n * MANA, pays: false }, ...(offered ? { split } : {}) })
        const { rerender } = render(<ShopPurchaseDialog view={view(true)} onClose={vi.fn()} onPay={onPay} />)
        arm()
        rerender(<ShopPurchaseDialog view={view(false)} onClose={vi.fn()} onPay={onPay} />)
        // Not together any more: radios again, none picked.
        expect(radio(/^Credits/).checked || radio(/^MANA/).checked).toBe(false)
        rerender(<ShopPurchaseDialog view={view(true)} onClose={vi.fn()} onPay={onPay} />)
        expect(box(/^Credits/).checked || box(/^MANA/).checked).toBe(false)
        expect(buy().disabled).toBe(true)
      })

      it('drops a lone tick that can’t pay when the view is sent again, and keeps one that pays', () => {
        vi.useFakeTimers()
        const onPay = vi.fn()
        // The same split read again: a new view object, as each balance read sends.
        const view = (): ShopPurchaseStep => ({ step: 'method', items: [item], credits: { ...short }, mana: { ...enough }, split: { ...split } })
        const { rerender } = render(<ShopPurchaseDialog view={view()} onClose={vi.fn()} onPay={onPay} />)
        arm()
        // Credits alone, mid-way to another pick: it doesn't pay, so the new view drops it and BUY waits.
        fireEvent.click(box(/^MANA/))
        rerender(<ShopPurchaseDialog view={view()} onClose={vi.fn()} onPay={onPay} />)
        expect(box(/^Credits/).checked || box(/^MANA/).checked).toBe(false)
        expect(buy().disabled).toBe(true)
        fireEvent.click(box(/^Credits/))
        fireEvent.click(box(/^MANA/))
        fireEvent.click(buy())
        expect(onPay).toHaveBeenLastCalledWith('split')
        // MANA alone pays the whole price, so the new view keeps it.
        fireEvent.click(box(/^Credits/))
        rerender(<ShopPurchaseDialog view={view()} onClose={vi.fn()} onPay={onPay} />)
        expect(box(/^Credits/).checked).toBe(false)
        expect(box(/^MANA/).checked).toBe(true)
        fireEvent.click(buy())
        expect(onPay).toHaveBeenLastCalledWith('mana')
      })

      it('keeps one pick at a time, as radios, where they aren’t offered together', () => {
        show({ step: 'method', items: [item], credits, mana })
        expect(screen.queryByRole('checkbox')).toBeNull()
        fireEvent.click(radio(/^MANA/))
        expect(radio(/^Credits/).checked).toBe(false)
        expect(radio(/^MANA/).checked).toBe(true)
      })
    })

    it('shows no Credits price on an item only MANA buys, never "0"', () => {
      show({ step: 'method', items: [{ ...item, credits: 0 }], credits: { price: null, balance: 700, pays: false }, mana })
      expect(screen.queryByTitle('0 Credits')).toBeNull()
      expect(screen.getByText('Golden Armor')).toBeTruthy()
    })

    it('shows no Credits price on any row of a cart only MANA buys, nor a Credits total', () => {
      show({ step: 'method', items: [{ ...item, credits: 0, key: 'a' }, { ...item, name: 'Silver Boots', credits: 0, key: 'b' }], credits: { price: null, balance: 700, pays: false }, mana: { ...mana, price: 10000n * MANA, balance: 20000n * MANA } })
      expect(screen.queryByTitle('0 Credits')).toBeNull()
      // Only the balance speaks of Credits: no row price, no total.
      expect(screen.queryAllByTitle(/ Credits$/).map((el) => el.title)).toEqual(screen.queryAllByTitle('700 Credits').map((el) => el.title))
      expect(screen.getByText('Silver Boots')).toBeTruthy()
    })

    it('shows MANA that can’t pay off, with what it would cost', () => {
      vi.useFakeTimers()
      const h = show({ step: 'method', items: [item], credits, mana: { ...mana, balance: 3n * MANA, pays: false } })
      expect(box(/^MANA/).disabled).toBe(true)
      expect(option(/^MANA/)).toBe('MANAMANA Balance:35,0001 Credit = 10 MANA')
      expect(box(/^Credits/).checked).toBe(true)
      arm()
      fireEvent.click(buy())
      expect(h.onPay).toHaveBeenCalledWith('credits')
    })

    it('holds BUY while neither way pays', () => {
      vi.useFakeTimers()
      const h = show({ step: 'method', items: [item], credits: { ...credits, balance: 200, pays: false }, mana: { ...mana, pays: false } })
      arm()
      expect(buy().disabled).toBe(true)
      fireEvent.click(buy())
      expect(h.onPay).not.toHaveBeenCalled()
    })

    it('picks the buyer’s earlier pick again while it pays, and the Credits once it doesn’t', () => {
      show({ step: 'method', items: [item], credits, mana, initial: 'mana' })
      expect(box(/^MANA/).checked).toBe(true)
      cleanup()
      show({ step: 'method', items: [item], credits, mana: { ...mana, pays: false }, initial: 'mana' })
      expect(box(/^Credits/).checked).toBe(true)
    })

    // A pick that stops paying is dropped, never moved: the same BUY must not spend another way than the buyer picked.
    it.each([
      { picked: 'credits' as const, other: 'mana' as const, picks: /^Credits/, otherOption: /^MANA/ },
      { picked: 'mana' as const, other: 'credits' as const, picks: /^MANA/, otherOption: /^Credits/ }
    ])('drops a $picked pick that stopped paying, with BUY armed, and spends only what the buyer picks next', ({ picked, other, picks, otherOption }) => {
      vi.useFakeTimers()
      const onPay = vi.fn()
      const view = (pays: boolean): ShopPurchaseStep => ({
        step: 'method',
        items: [item],
        credits: { ...credits, pays: picked === 'credits' ? pays : true },
        mana: { ...mana, pays: picked === 'mana' ? pays : true }
      })
      const { rerender } = render(<ShopPurchaseDialog view={view(true)} onClose={vi.fn()} onPay={onPay} />)
      // Picked by the buyer, the other option first, so the pick is theirs and not the step's opening one.
      fireEvent.click(box(otherOption))
      fireEvent.click(box(picks))
      arm()
      rerender(<ShopPurchaseDialog view={view(false)} onClose={vi.fn()} onPay={onPay} />)
      expect(box(/^Credits/).checked).toBe(false)
      expect(box(/^MANA/).checked).toBe(false)
      expect(buy().disabled).toBe(true)
      fireEvent.click(buy())
      expect(onPay).not.toHaveBeenCalled()
      // Paying again later doesn't pick it back.
      rerender(<ShopPurchaseDialog view={view(true)} onClose={vi.fn()} onPay={onPay} />)
      expect(box(picks).checked).toBe(false)
      expect(buy().disabled).toBe(true)
      fireEvent.click(buy())
      expect(onPay).not.toHaveBeenCalled()
      fireEvent.click(box(otherOption))
      fireEvent.click(buy())
      expect(onPay).toHaveBeenCalledTimes(1)
      expect(onPay).toHaveBeenCalledWith(other)
    })
  })

  describe('Authorize MANA', () => {
    const view = { step: 'authorize' as const, venue: 'the Marketplace', network: 'Polygon', required: 12n * 10n ** 18n + 5n * 10n ** 17n }

    afterEach(() => vi.useRealTimers())

    it('names what may spend the MANA, where, and how much this purchase needs, and that the allowance is unlimited and stays', () => {
      vi.useFakeTimers()
      const h = show({ ...view, state: 'ready' })
      const text = screen.getByRole('dialog', { name: 'Authorize MANA' }).textContent
      expect(text).toContain('To buy with MANA, allow the Marketplace on Polygon to spend MANA from your wallet. This purchase needs 12.5 MANA.')
      expect(text).toContain('The allowance is unlimited and stays after this purchase, so buying there again won’t ask for it.')
      // It appears where BUY was: a click meant for that one doesn't authorize.
      fireEvent.click(screen.getByRole('button', { name: 'Authorize' }))
      expect(h.onAuthorize).not.toHaveBeenCalled()
      act(() => void vi.advanceTimersByTime(ARM_MS))
      fireEvent.click(screen.getByRole('button', { name: 'Authorize' }))
      expect(h.onAuthorize).toHaveBeenCalledTimes(1)
    })

    it('says an exact allowance is that amount for this purchase alone, never unlimited', () => {
      show({ ...view, venue: 'Decentraland Credits', state: 'ready', allowance: 'exact' })
      const text = screen.getByRole('dialog', { name: 'Authorize MANA' }).textContent
      expect(text).toContain('allow Decentraland Credits on Polygon to spend MANA')
      expect(text).toContain('The allowance is exactly 12.5 MANA, for this purchase alone: the next one asks again.')
      expect(text).not.toContain('unlimited')
    })

    it('says which payment of several it authorizes', () => {
      show({ ...view, state: 'ready', progress: { at: 2, of: 2 } })
      expect(screen.getByRole('dialog', { name: 'Authorize MANA' }).textContent).toContain('Payment 2 of 2')
    })

    it('waits for the authorization with AUTHORIZE held and CANCEL still live', () => {
      const h = show({ ...view, state: 'waiting' })
      expect(screen.getByRole('dialog', { name: 'Authorize MANA' }).getAttribute('aria-busy')).toBe('true')
      expect((screen.getByRole('button', { name: 'Authorizing...' }) as HTMLButtonElement).disabled).toBe(true)
      expect(screen.getByRole('status', { name: 'Waiting for the authorization to confirm' })).toBeInTheDocument()
      fireEvent.click(screen.getByRole('button', { name: 'Cancel' }))
      expect(h.onClose).toHaveBeenCalled()
    })

    it('says why it didn’t go through, and TRY AGAIN authorizes again', () => {
      vi.useFakeTimers()
      const h = show({ ...view, state: 'failed', message: 'You declined the authorization in your wallet.' })
      expect(screen.getByRole('alert').textContent).toBe('You declined the authorization in your wallet.')
      act(() => void vi.advanceTimersByTime(ARM_MS))
      fireEvent.click(screen.getByRole('button', { name: 'Try again' }))
      expect(h.onAuthorize).toHaveBeenCalledTimes(1)
    })
  })

  describe('a purchase still being confirmed', () => {
    it('says it was sent, with nothing to check, by default', () => {
      show({ step: 'pending' })
      expect(screen.getByRole('dialog', { name: 'Purchase in progress' }).textContent).toContain('Your purchase was sent and is still being confirmed.')
      expect(screen.queryByRole('button', { name: 'Check again' })).toBeNull()
    })

    it('says what is known in its own words, and offers CHECK AGAIN', () => {
      const h = show({ step: 'pending', message: 'We can’t tell yet whether your purchase went out.', check: 'ready' })
      expect(screen.getByRole('dialog', { name: 'Purchase in progress' }).textContent).toContain('We can’t tell yet whether your purchase went out.')
      fireEvent.click(screen.getByRole('button', { name: 'Check again' }))
      expect(h.onCheckAgain).toHaveBeenCalledTimes(1)
      fireEvent.click(screen.getByRole('button', { name: 'Continue shopping' }))
      expect(h.onClose).toHaveBeenCalled()
    })

    it('offers an armed RETRY in place of CHECK AGAIN when it may be retried', () => {
      vi.useFakeTimers()
      try {
        const h = show({ step: 'pending', message: 'You can retry this same purchase fee-free. It can complete only once.', retry: true, check: 'ready' })
        // Announced: RETRY opens on a dialog that has been up for minutes.
        expect(screen.getByRole('status').textContent).toBe('You can retry this same purchase fee-free. It can complete only once.')
        expect(screen.queryByRole('button', { name: 'Check again' })).toBeNull()
        fireEvent.click(screen.getByRole('button', { name: 'Retry' }))
        expect(h.onRetry).not.toHaveBeenCalled()
        act(() => void vi.advanceTimersByTime(ARM_MS))
        fireEvent.click(screen.getByRole('button', { name: 'Retry' }))
        expect(h.onRetry).toHaveBeenCalledTimes(1)
      } finally {
        vi.useRealTimers()
      }
    })

    it('holds CHECK AGAIN while it looks', () => {
      show({ step: 'pending', check: 'checking' })
      expect((screen.getByRole('button', { name: 'Checking...' }) as HTMLButtonElement).disabled).toBe(true)
    })
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

  it('reaches a success list that scrolls with Tab, after the first focus lands on CONTINUE SHOPPING', async () => {
    const ui = createShopOverlay(vi.fn())
    const many = Array.from({ length: 7 }, (_, i) => ({ ...item, name: `Item ${i + 1}` }))
    await act(async () => ui.show({ step: 'success', items: many }, { onClose: vi.fn(), onBackpack: vi.fn() }))
    const row = screen.getByRole('list', { name: '7 items bought' })
    expect(screen.getAllByRole('listitem').map((el) => el.getAttribute('aria-label'))).toEqual(['Item 1', 'Item 2', 'Item 3', 'Item 4', 'Item 5', 'Item 6', 'Item 7'])
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

  it('Buy Item with its payment options: the first focus on CANCEL, and Tab stops once at the options, on the picked one', async () => {
    const ui = createShopOverlay(vi.fn())
    const MANA = 10n ** 18n
    const view = { step: 'method' as const, items: [item], credits: { price: 500, balance: 700, pays: true }, mana: { price: 12n * MANA, balance: 40n * MANA, pays: true } }
    await act(async () => ui.show(view, { onClose: vi.fn(), onPay: vi.fn() }))
    const cancel = screen.getByRole('button', { name: 'Cancel' })
    const credits = screen.getByRole('radio', { name: /^Credits/ })
    expect(document.activeElement).toBe(cancel)
    const visited = new Set<Element | null>()
    for (let i = 0; i < 4; i++) {
      fireEvent.keyDown(document.activeElement!, { key: 'Tab' })
      visited.add(document.activeElement)
    }
    // A radio group is one stop, its picked radio (the arrow keys move within it).
    expect(visited).toEqual(new Set([credits, cancel, screen.getByRole('button', { name: 'Buy item' })]))
  })

  it('skips an option that can’t pay, and a held BUY, when tabbing', async () => {
    const ui = createShopOverlay(vi.fn())
    const MANA = 10n ** 18n
    const view = { step: 'method' as const, items: [item], credits: { price: 500, balance: 200, pays: false }, mana: { price: 12n * MANA, balance: 4n * MANA, pays: false } }
    await act(async () => ui.show(view, { onClose: vi.fn(), onPay: vi.fn() }))
    const cancel = screen.getByRole('button', { name: 'Cancel' })
    cancel.focus()
    const visited = new Set<Element | null>()
    for (let i = 0; i < 3; i++) {
      fireEvent.keyDown(document.activeElement!, { key: 'Tab' })
      visited.add(document.activeElement)
    }
    expect(visited).toEqual(new Set([cancel]))
  })

  it('lets Escape cancel Authorize MANA while it confirms', async () => {
    const ui = createShopOverlay(vi.fn())
    const onClose = vi.fn()
    await act(async () => ui.show({ step: 'authorize', venue: 'the Marketplace', network: 'Polygon', required: 10n ** 18n, state: 'waiting' }, { onClose, onAuthorize: vi.fn() }))
    expect(document.activeElement?.textContent).toBe('Cancel')
    fireEvent.keyDown(screen.getByRole('dialog'), { key: 'Escape' })
    expect(onClose).toHaveBeenCalledTimes(1)
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
