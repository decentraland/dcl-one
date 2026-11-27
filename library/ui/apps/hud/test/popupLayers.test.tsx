import { describe, it, expect, vi, afterEach } from 'vitest'
import { render, screen, fireEvent, act } from '@testing-library/react'
import { PopupHost, openPopup, closeTopPopup, hasOpenPopup, showConfirm, showDialog, resetPopups } from '../design'

// The popup layer keeps the stack, the focus and the close rules, and draws each layer with the shared
// ModalBackdrop. These tests pin what the layer owns across that seam: the order stacked dialogs close in,
// where the focus goes, that a dialog's promise takes its first answer only, and that a close guard still
// refuses. The session's Cancel (Escape) reaches the layer as closeTopPopup(): see popups.test.tsx.

// A layer is what PopupHost mounts in the render container and makes focusable for its trap. Content is never
// matched, whatever its class names.
const backdrops = (): HTMLElement[] => [...document.querySelectorAll<HTMLElement>('body > div > [tabindex="-1"]')]
const topBackdrop = (): HTMLElement => backdrops().at(-1) as HTMLElement
const cancel = (): void => act(() => closeTopPopup())

afterEach(resetPopups)

describe('the layer each popup is drawn on', () => {
  it('draws the dim scrim by default, the deep one and the fixed size when asked', () => {
    render(<PopupHost />)
    act(() => {
      openPopup(() => <div>plain</div>)
      openPopup(() => <div>deep</div>, { deep: true })
      openPopup(() => <div>fixed</div>, { fixed: true, className: 'mine' })
    })
    const [plain, deep, fixed] = backdrops()
    expect(plain.dataset.tone).toBe('dim')
    expect(plain.dataset.fixed).toBeUndefined()
    expect(deep.dataset.tone).toBe('deep')
    expect(fixed.dataset.tone).toBe('dim')
    expect(fixed.dataset.fixed).toBe('true')
    expect(fixed).toHaveClass('mine')
    expect(fixed).toHaveAttribute('tabindex', '-1')
    // the content scales in through one layer between it and the scrim
    expect(screen.getByText('plain').parentElement?.parentElement).toBe(plain)
  })

  it('draws no scrim for an anchored popover, whatever else it asks for: the content sits on the click-catcher', () => {
    render(<PopupHost />)
    act(() => {
      openPopup(() => <div>popover</div>, { dim: false, deep: true, fixed: true })
    })
    const [layer] = backdrops()
    expect(layer.dataset.tone).toBeUndefined()
    expect(layer.dataset.fixed).toBeUndefined()
    expect(screen.getByText('popover').parentElement).toBe(layer)
    fireEvent.click(layer)
    expect(screen.queryByText('popover')).toBeNull()
  })

  it('draws no layer at all for content that owns its scrim, which still closes from the stack', () => {
    render(<PopupHost />)
    act(() => {
      openPopup(() => <div>bare</div>, { backdrop: false })
    })
    expect(screen.getByText('bare')).toBeInTheDocument()
    expect(backdrops()).toHaveLength(0)
    cancel()
    expect(screen.queryByText('bare')).toBeNull()
  })
})

describe('stacked dialogs', () => {
  it('Cancel closes only the top dialog each time, in order, and each resolves when it closes', async () => {
    render(<PopupHost />)
    const settled: string[] = []
    act(() => {
      void showDialog({ title: 'First', actions: [{ id: 'ok', label: 'OK' }] }).then((r) => settled.push(`first:${r}`))
      void showDialog({ title: 'Second', actions: [{ id: 'ok', label: 'OK' }] }).then((r) => settled.push(`second:${r}`))
      void showDialog({ title: 'Third', actions: [{ id: 'ok', label: 'OK' }] }).then((r) => settled.push(`third:${r}`))
    })
    expect(screen.getAllByRole('dialog').map((d) => d.getAttribute('aria-label'))).toEqual(['First', 'Second', 'Third'])

    cancel()
    await act(async () => {})
    expect(screen.getAllByRole('dialog').map((d) => d.getAttribute('aria-label'))).toEqual(['First', 'Second'])
    expect(settled).toEqual(['third:null'])

    cancel()
    await act(async () => {})
    expect(screen.getAllByRole('dialog').map((d) => d.getAttribute('aria-label'))).toEqual(['First'])
    expect(settled).toEqual(['third:null', 'second:null'])

    cancel()
    await act(async () => {})
    expect(screen.queryByRole('dialog')).toBeNull()
    expect(settled).toEqual(['third:null', 'second:null', 'first:null'])
    expect(hasOpenPopup()).toBe(false)
  })

  it('a press on the top card closes nothing, and a press on the top scrim closes only the top dialog', () => {
    render(<PopupHost />)
    act(() => {
      void showDialog({ title: 'Below', actions: [{ id: 'ok', label: 'OK' }] })
      void showDialog({ title: 'Above', actions: [{ id: 'ok', label: 'OK' }] })
    })
    fireEvent.click(screen.getByRole('dialog', { name: 'Above' }))
    expect(screen.getAllByRole('dialog')).toHaveLength(2)
    fireEvent.click(topBackdrop())
    expect(screen.getByRole('dialog', { name: 'Below' })).toBeInTheDocument()
    expect(screen.queryByRole('dialog', { name: 'Above' })).toBeNull()
  })
})

describe('focus', () => {
  it('moves to the top layer on open, back to the layer below when it closes, and to the opener at the end', () => {
    render(
      <>
        <button>opener</button>
        <PopupHost />
      </>
    )
    const opener = screen.getByRole('button', { name: 'opener' })
    opener.focus()

    act(() => {
      void showDialog({ title: 'First', actions: [{ id: 'ok', label: 'OK' }] })
    })
    const first = topBackdrop()
    expect(first).toHaveFocus()

    act(() => {
      void showDialog({ title: 'Second', actions: [{ id: 'ok', label: 'OK' }] })
    })
    expect(topBackdrop()).not.toBe(first)
    expect(topBackdrop()).toHaveFocus()

    cancel()
    expect(first).toHaveFocus()

    cancel()
    expect(opener).toHaveFocus()
  })

  it('returns to the opener when the dialog closes from its action', () => {
    render(
      <>
        <button>opener</button>
        <PopupHost />
      </>
    )
    const opener = screen.getByRole('button', { name: 'opener' })
    opener.focus()
    act(() => {
      void showConfirm({ title: 'Sure?' })
    })
    expect(opener).not.toHaveFocus()
    fireEvent.click(screen.getByRole('button', { name: 'Confirm' }))
    expect(opener).toHaveFocus()
  })

  it('keeps Tab inside the top layer', () => {
    render(<PopupHost />)
    act(() => {
      void showConfirm({ title: 'Sure?' })
    })
    const close = screen.getByRole('button', { name: 'Close' })
    const confirm = screen.getByRole('button', { name: 'Confirm' })
    confirm.focus()
    fireEvent.keyDown(document, { key: 'Tab' })
    expect(close).toHaveFocus()
    fireEvent.keyDown(document, { key: 'Tab', shiftKey: true })
    expect(confirm).toHaveFocus()
  })
})

describe("a confirm's answer", () => {
  const open = (): { answer: Promise<boolean>; onAnswer: ReturnType<typeof vi.fn> } => {
    const onAnswer = vi.fn()
    let answer!: Promise<boolean>
    act(() => {
      answer = showConfirm({ title: 'Sure?' })
      void answer.then(onAnswer)
    })
    return { answer, onAnswer }
  }

  it('is true from Confirm, once: the close that follows does not answer again', async () => {
    render(<PopupHost />)
    const { answer, onAnswer } = open()
    fireEvent.click(screen.getByRole('button', { name: 'Confirm' }))
    expect(await answer).toBe(true)
    cancel() // nothing left to close
    await act(async () => {})
    expect(onAnswer).toHaveBeenCalledTimes(1)
    expect(screen.queryByRole('dialog')).toBeNull()
  })

  it('is false from Cancel, from the × and from a press on the scrim', async () => {
    render(<PopupHost />)
    let { answer } = open()
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }))
    expect(await answer).toBe(false)
    ;({ answer } = open())
    fireEvent.click(screen.getByRole('button', { name: 'Close' }))
    expect(await answer).toBe(false)
    ;({ answer } = open())
    fireEvent.click(topBackdrop())
    expect(await answer).toBe(false)
    expect(screen.queryByRole('dialog')).toBeNull()
  })

  it('is false from the session Cancel (Escape)', async () => {
    render(<PopupHost />)
    const { answer, onAnswer } = open()
    cancel()
    expect(await answer).toBe(false)
    expect(onAnswer).toHaveBeenCalledTimes(1)
  })

  it('stays pending while a dialog opened above it closes, and then takes its own answer', async () => {
    render(<PopupHost />)
    const { answer, onAnswer } = open()
    let above!: Promise<string | null>
    act(() => {
      above = showDialog({ title: 'Above', actions: [{ id: 'ok', label: 'OK' }] })
    })
    cancel()
    expect(await above).toBeNull()
    expect(onAnswer).not.toHaveBeenCalled()
    expect(screen.getByRole('dialog', { name: 'Sure?' })).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'Confirm' }))
    expect(await answer).toBe(true)
    expect(onAnswer).toHaveBeenCalledTimes(1)
  })
})

describe('a dialog closed by its owner', () => {
  it('resolves null when its signal aborts, also under another dialog, which stays open', async () => {
    render(<PopupHost />)
    const owner = new AbortController()
    const onAnswer = vi.fn()
    let below!: Promise<string | null>
    act(() => {
      below = showDialog({ title: 'Below', actions: [{ id: 'ok', label: 'OK' }], signal: owner.signal })
      void below.then(onAnswer)
      void showDialog({ title: 'Above', actions: [{ id: 'ok', label: 'OK' }] })
    })
    act(() => owner.abort())
    expect(await below).toBeNull()
    expect(screen.queryByRole('dialog', { name: 'Below' })).toBeNull()
    expect(screen.getByRole('dialog', { name: 'Above' })).toBeInTheDocument()
    act(() => owner.abort()) // already closed
    await act(async () => {})
    expect(onAnswer).toHaveBeenCalledTimes(1)
  })

  it('does not open at all for a signal that has already aborted', async () => {
    render(<PopupHost />)
    const owner = new AbortController()
    owner.abort()
    let answer!: Promise<string | null>
    act(() => {
      answer = showDialog({ title: 'Never', actions: [{ id: 'ok', label: 'OK' }], signal: owner.signal })
    })
    expect(await answer).toBeNull()
    expect(screen.queryByRole('dialog')).toBeNull()
    expect(hasOpenPopup()).toBe(false)
  })

  it('draws a dialog with no actions and no body, which still closes', async () => {
    render(<PopupHost />)
    let answer!: Promise<string | null>
    act(() => {
      answer = showDialog({ title: 'Nothing to choose', actions: [] })
    })
    const dialog = screen.getByRole('dialog', { name: 'Nothing to choose' })
    expect(dialog.querySelectorAll('button')).toHaveLength(1) // the × alone
    cancel()
    expect(await answer).toBeNull()
  })
})

describe('a close guard', () => {
  it('refuses the session Cancel and the popup own close, and leaves the popup and its onClose untouched', () => {
    render(<PopupHost />)
    const onClose = vi.fn()
    const confirmClose = vi.fn(() => false)
    act(() => {
      // as a dialog card does, the content keeps its presses from the scrim
      openPopup((close) => <div onClick={(e) => e.stopPropagation()}><button onClick={close}>form close</button></div>, { confirmClose, onClose })
    })
    cancel()
    fireEvent.click(screen.getByRole('button', { name: 'form close' }))
    expect(confirmClose).toHaveBeenCalledTimes(2)
    expect(screen.getByRole('button', { name: 'form close' })).toBeInTheDocument()
    expect(onClose).not.toHaveBeenCalled()
  })

  it('is not asked on a press on a scrim that does not close, and is asked on one that does', () => {
    render(<PopupHost />)
    const confirmClose = vi.fn(() => false)
    let closes = false
    act(() => {
      openPopup(() => <div>form</div>, { confirmClose, backdropClickCloses: () => closes })
    })
    fireEvent.click(topBackdrop())
    expect(confirmClose).not.toHaveBeenCalled()
    closes = true
    fireEvent.click(topBackdrop())
    expect(confirmClose).toHaveBeenCalledTimes(1)
    expect(screen.getByText('form')).toBeInTheDocument()
  })

  it('closes once it answers yes, at once or later, and keeps the popup open on a later no', async () => {
    render(<PopupHost />)
    const onClose = vi.fn()
    let answer: boolean | Promise<boolean> = Promise.resolve(false)
    act(() => {
      openPopup(() => <div>form</div>, { confirmClose: () => answer, onClose })
    })
    cancel()
    await act(async () => {})
    expect(screen.getByText('form')).toBeInTheDocument()

    answer = Promise.resolve(true)
    cancel()
    await act(async () => {})
    expect(screen.queryByText('form')).toBeNull()
    expect(onClose).toHaveBeenCalledTimes(1)

    act(() => {
      openPopup(() => <div>again</div>, { confirmClose: () => true, onClose })
    })
    cancel() // a yes at once closes before the next tick
    expect(screen.queryByText('again')).toBeNull()
    expect(onClose).toHaveBeenCalledTimes(2)
  })

  it("is bypassed by the owner's handle", () => {
    render(<PopupHost />)
    const onClose = vi.fn()
    let close!: () => void
    act(() => {
      close = openPopup(() => <div>form</div>, { confirmClose: () => false, onClose })
    })
    act(() => close())
    expect(screen.queryByText('form')).toBeNull()
    expect(onClose).toHaveBeenCalledTimes(1)
  })

  it('guards only its own popup: a dialog above it closes, and the guarded one below stays', () => {
    render(<PopupHost />)
    act(() => {
      openPopup(() => <div>form</div>, { confirmClose: () => false })
      void showDialog({ title: 'Above', actions: [{ id: 'ok', label: 'OK' }] })
    })
    cancel()
    expect(screen.queryByRole('dialog', { name: 'Above' })).toBeNull()
    cancel()
    expect(screen.getByText('form')).toBeInTheDocument()
  })
})
