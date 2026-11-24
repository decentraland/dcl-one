import { useRef, useState } from 'react'
import { act, fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { renderToString } from 'react-dom/server'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { Tooltip } from './Tooltip'

const box = (left: number, top: number, width: number, height: number) =>
  ({
    left,
    top,
    width,
    height,
    right: left + width,
    bottom: top + height,
    x: left,
    y: top,
    toJSON: () => ({})
  }) as DOMRect

// jsdom has no layout: give the wrapper of the trigger and the tip the boxes a browser would measure.
const layout = (trigger: DOMRect, tip: DOMRect) =>
  vi.spyOn(Element.prototype, 'getBoundingClientRect').mockImplementation(function (this: Element) {
    return this.getAttribute('role') === 'tooltip'
      ? tip
      : this.querySelector('button')
        ? trigger
        : box(0, 0, 0, 0)
  })

afterEach(() => {
  vi.useRealTimers()
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
})

describe('Tooltip', () => {
  it('a label and a shortcut render inside the wrapper with no portal', () => {
    const { container } = render(
      <Tooltip label="Rare" shortcut="R" side="top" className="tip">
        <button>Rarity</button>
      </Tooltip>
    )

    const tip = screen.getByRole('tooltip')
    expect(tip.textContent).toBe('Rare[R]')
    expect(container.contains(tip)).toBe(true)
    expect(tip.getAttribute('style')).toBeNull()
    expect(tip.id).toBe('')
  })

  it('a portal tip with a label opens on hover and closes when the pointer leaves', async () => {
    const user = userEvent.setup()
    render(
      <Tooltip label="Move" portal>
        <button>Tool</button>
      </Tooltip>
    )
    expect(screen.queryByRole('tooltip')).toBeNull()

    await user.hover(screen.getByRole('button', { name: 'Tool' }))
    expect(screen.getByRole('tooltip').textContent).toBe('Move')

    await user.unhover(screen.getByRole('button', { name: 'Tool' }))
    expect(screen.queryByRole('tooltip')).toBeNull()
  })

  it('rich content shows in place of the label', async () => {
    const user = userEvent.setup()
    render(
      <Tooltip
        portal
        content={
          <>
            Sets the size. <a href="https://example.test/docs">Learn more</a>
          </>
        }
      >
        <button>Size</button>
      </Tooltip>
    )

    await user.hover(screen.getByRole('button', { name: 'Size' }))

    expect(screen.getByRole('tooltip').textContent).toBe('Sets the size. Learn more')
    expect(screen.getByRole('link', { name: 'Learn more' }).getAttribute('href')).toBe(
      'https://example.test/docs'
    )
  })

  it('an interactive tip stays open while the pointer moves into it', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true })
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime })
    render(
      <Tooltip interactive content={<a href="https://example.test/docs">Learn more</a>}>
        <button>Size</button>
      </Tooltip>
    )

    await user.hover(screen.getByRole('button', { name: 'Size' }))
    await user.hover(screen.getByRole('link', { name: 'Learn more' }))
    act(() => {
      vi.advanceTimersByTime(1000)
    })

    expect(screen.getByRole('link', { name: 'Learn more' })).toBeInTheDocument()
  })

  it('an interactive tip closes soon after the pointer leaves it', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true })
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime })
    render(
      <Tooltip interactive content={<a href="https://example.test/docs">Learn more</a>}>
        <button>Size</button>
      </Tooltip>
    )

    await user.hover(screen.getByRole('button', { name: 'Size' }))
    await user.hover(screen.getByRole('link', { name: 'Learn more' }))
    await user.unhover(screen.getByRole('link', { name: 'Learn more' }))
    act(() => {
      vi.advanceTimersByTime(1000)
    })

    expect(screen.queryByRole('tooltip')).toBeNull()
  })

  it('a pointer press inside an interactive tip does not close it', async () => {
    const user = userEvent.setup()
    render(
      <Tooltip interactive content={<a href="#docs">Learn more</a>}>
        <button>Size</button>
      </Tooltip>
    )

    await user.hover(screen.getByRole('button', { name: 'Size' }))
    await user.hover(screen.getByRole('link', { name: 'Learn more' }))
    fireEvent.pointerDown(screen.getByRole('link', { name: 'Learn more' }))

    expect(screen.getByRole('tooltip')).toBeInTheDocument()
  })

  it('a controlled tip follows the open prop and reports hover without opening itself', async () => {
    const user = userEvent.setup()
    const onOpenChange = vi.fn()
    const { rerender } = render(
      <Tooltip label="Door" open={false} onOpenChange={onOpenChange}>
        <button>Asset</button>
      </Tooltip>
    )

    await user.hover(screen.getByRole('button', { name: 'Asset' }))
    expect(screen.queryByRole('tooltip')).toBeNull()
    expect(onOpenChange).toHaveBeenLastCalledWith(true)

    rerender(
      <Tooltip label="Door" open onOpenChange={onOpenChange}>
        <button>Asset</button>
      </Tooltip>
    )
    expect(screen.getByRole('tooltip').textContent).toBe('Door')

    await user.unhover(screen.getByRole('button', { name: 'Asset' }))
    expect(screen.getByRole('tooltip')).toBeInTheDocument()
    expect(onOpenChange).toHaveBeenLastCalledWith(false)
    expect(onOpenChange).toHaveBeenCalledTimes(2)
  })

  it('a tip the parent opened reports the Escape key', async () => {
    const user = userEvent.setup()
    const onOpenChange = vi.fn()
    render(
      <Tooltip label="Door" open onOpenChange={onOpenChange}>
        <button>Asset</button>
      </Tooltip>
    )

    await user.keyboard('{Escape}')

    expect(onOpenChange.mock.calls).toEqual([[false]])
  })

  it('an uncontrolled tip reports each change once', async () => {
    const user = userEvent.setup()
    const onOpenChange = vi.fn()
    render(
      <Tooltip label="Door" onOpenChange={onOpenChange}>
        <button>Asset</button>
      </Tooltip>
    )

    await user.hover(screen.getByRole('button', { name: 'Asset' }))
    await user.keyboard('{Escape}')
    await user.unhover(screen.getByRole('button', { name: 'Asset' }))

    expect(onOpenChange.mock.calls).toEqual([[true], [false]])
    expect(screen.queryByRole('tooltip')).toBeNull()
  })

  it('the wrapper names the open tip as its description', async () => {
    const user = userEvent.setup()
    render(
      <Tooltip label="Move" portal>
        <button>Tool</button>
      </Tooltip>
    )

    await user.hover(screen.getByRole('button', { name: 'Tool' }))

    const tip = screen.getByRole('tooltip')
    expect(tip.id).not.toBe('')
    expect(
      screen.getByRole('button', { name: 'Tool' }).parentElement?.getAttribute('aria-describedby')
    ).toBe(tip.id)
  })

  it('a tip on an outside anchor opens from the anchor and describes it', async () => {
    const user = userEvent.setup()
    const Anchored = () => {
      const [anchor, setAnchor] = useState<HTMLButtonElement | null>(null)
      return (
        <>
          <button ref={setAnchor}>Tile</button>
          <Tooltip label="Door" anchor={anchor} />
        </>
      )
    }
    render(<Anchored />)
    const tile = screen.getByRole('button', { name: 'Tile' })

    await user.hover(tile)
    expect(screen.getByRole('tooltip').textContent).toBe('Door')
    expect(tile.getAttribute('aria-describedby')).toBe(screen.getByRole('tooltip').id)

    await user.unhover(tile)
    expect(screen.queryByRole('tooltip')).toBeNull()
    expect(tile.hasAttribute('aria-describedby')).toBe(false)
  })

  it('a controlled tip on an outside anchor sits beside the anchor', () => {
    const anchor = document.createElement('div')
    document.body.append(anchor)
    vi.spyOn(anchor, 'getBoundingClientRect').mockReturnValue(box(100, 200, 80, 40))

    render(
      <Tooltip content={<p>Opens on click.</p>} anchor={anchor} open bare tipClassName="item-tip" />
    )

    const tip = screen.getByRole('tooltip')
    expect(tip.style.left).toBe('192px')
    expect(tip.style.top).toBe('220px')
    expect(tip.classList.contains('item-tip')).toBe(true)
    anchor.remove()
  })

  it('a maximum width reaches the tip as a custom property', () => {
    render(
      <Tooltip label="A long explanation of the field" maxWidth={250}>
        <button>Field</button>
      </Tooltip>
    )

    expect(screen.getByRole('tooltip').style.getPropertyValue('--tt-max-width')).toBe('250px')
  })

  it('a tip that does not fit on its side flips to the opposite side', async () => {
    const user = userEvent.setup()
    layout(box(window.innerWidth - 60, 300, 40, 20), box(0, 0, 200, 50))
    render(
      <Tooltip label="Wide" side="right" keepInViewport>
        <button>Edge</button>
      </Tooltip>
    )

    await user.hover(screen.getByRole('button', { name: 'Edge' }))

    const tip = screen.getByRole('tooltip')
    expect(tip.style.left).toBe(window.innerWidth - 60 - 12 - 200 + 'px')
    expect(tip.style.top).toBe('285px')
  })

  it('a tip that crosses a viewport edge shifts back inside', async () => {
    const user = userEvent.setup()
    layout(box(4, 300, 40, 20), box(0, 0, 200, 50))
    render(
      <Tooltip label="Wide" side="top" keepInViewport>
        <button>Edge</button>
      </Tooltip>
    )

    await user.hover(screen.getByRole('button', { name: 'Edge' }))

    const tip = screen.getByRole('tooltip')
    expect(tip.style.left).toBe('8px')
    expect(tip.style.top).toBe('238px')
  })

  it('a tip that fits stays on its side', async () => {
    const user = userEvent.setup()
    layout(box(400, 300, 40, 20), box(0, 0, 200, 50))
    render(
      <Tooltip label="Wide" side="right" keepInViewport>
        <button>Middle</button>
      </Tooltip>
    )

    await user.hover(screen.getByRole('button', { name: 'Middle' }))

    expect(screen.getByRole('tooltip').style.left).toBe('452px')
  })

  it('a tip set to close on scroll closes when the page scrolls', async () => {
    const user = userEvent.setup()
    render(
      <Tooltip label="Move" closeOnScroll>
        <button>Tool</button>
      </Tooltip>
    )
    await user.hover(screen.getByRole('button', { name: 'Tool' }))

    fireEvent.scroll(window)

    expect(screen.queryByRole('tooltip')).toBeNull()
  })

  it('a portal tip stays open when the page scrolls', async () => {
    const user = userEvent.setup()
    render(
      <Tooltip label="Move" portal>
        <button>Tool</button>
      </Tooltip>
    )
    await user.hover(screen.getByRole('button', { name: 'Tool' }))

    fireEvent.scroll(window)

    expect(screen.getByRole('tooltip')).toBeInTheDocument()
  })

  it('a disabled tip never opens', async () => {
    const user = userEvent.setup()
    const onOpenChange = vi.fn()
    const { rerender } = render(
      <Tooltip label="Reload" portal disabled onOpenChange={onOpenChange}>
        <button>Reload</button>
      </Tooltip>
    )

    await user.hover(screen.getByRole('button', { name: 'Reload' }))
    expect(screen.queryByRole('tooltip')).toBeNull()
    expect(onOpenChange).not.toHaveBeenCalled()

    rerender(
      <Tooltip label="Reload" disabled>
        <button>Reload</button>
      </Tooltip>
    )
    expect(screen.queryByRole('tooltip')).toBeNull()
  })

  it('a tip with an arrow draws its pointer, and a tip with none does not', () => {
    const { rerender } = render(
      <Tooltip label="Rare" side="top" arrow>
        <button>Rarity</button>
      </Tooltip>
    )
    expect(screen.getByRole('tooltip')).toHaveClass('tt__tip--arrow', 'tt__tip--top')

    rerender(
      <Tooltip label="Rare" side="top">
        <button>Rarity</button>
      </Tooltip>
    )
    expect(screen.getByRole('tooltip')).not.toHaveClass('tt__tip--arrow')
    expect(screen.getByRole('tooltip')).toHaveClass('tt__tip--top')
  })

  it('the arrow of a tip that flipped points from the side the tip is on', async () => {
    const user = userEvent.setup()
    layout(box(window.innerWidth - 60, 300, 40, 20), box(0, 0, 200, 50))
    render(
      <Tooltip label="Wide" side="right" keepInViewport arrow>
        <button>Edge</button>
      </Tooltip>
    )

    await user.hover(screen.getByRole('button', { name: 'Edge' }))

    expect(screen.getByRole('tooltip')).toHaveClass('tt__tip--arrow', 'tt__tip--left')
    expect(screen.getByRole('tooltip')).not.toHaveClass('tt__tip--right')
  })

  it('a tip with an enter delay opens when the pointer stayed that long', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true })
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime })
    const onOpenChange = vi.fn()
    render(
      <Tooltip label="Move" enterDelay={2000} onOpenChange={onOpenChange}>
        <button>Tool</button>
      </Tooltip>
    )

    await user.hover(screen.getByRole('button', { name: 'Tool' }))
    act(() => {
      vi.advanceTimersByTime(1000)
    })
    expect(screen.queryByRole('tooltip')).toBeNull()
    expect(onOpenChange).not.toHaveBeenCalled()

    act(() => {
      vi.advanceTimersByTime(1500)
    })
    expect(screen.getByRole('tooltip').textContent).toBe('Move')
    expect(onOpenChange.mock.calls).toEqual([[true]])
  })

  it('a tip with an enter delay stays closed when the pointer leaves before that', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true })
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime })
    const onOpenChange = vi.fn()
    render(
      <Tooltip label="Move" enterDelay={2000} onOpenChange={onOpenChange}>
        <button>Tool</button>
      </Tooltip>
    )

    await user.hover(screen.getByRole('button', { name: 'Tool' }))
    act(() => {
      vi.advanceTimersByTime(1000)
    })
    await user.unhover(screen.getByRole('button', { name: 'Tool' }))
    act(() => {
      vi.advanceTimersByTime(5000)
    })

    expect(screen.queryByRole('tooltip')).toBeNull()
    expect(onOpenChange).not.toHaveBeenCalled()
  })

  it('an interactive tip with an enter delay never opens when the pointer leaves before the delay', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true })
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime })
    const onOpenChange = vi.fn()
    render(
      <Tooltip
        content={<a href="/docs">Docs</a>}
        interactive
        enterDelay={100}
        onOpenChange={onOpenChange}
      >
        <button>Tool</button>
      </Tooltip>
    )

    await user.hover(screen.getByRole('button', { name: 'Tool' }))
    act(() => {
      vi.advanceTimersByTime(50)
    })
    await user.unhover(screen.getByRole('button', { name: 'Tool' }))
    act(() => {
      vi.advanceTimersByTime(1000)
    })

    expect(screen.queryByRole('tooltip')).toBeNull()
    expect(onOpenChange).not.toHaveBeenCalled()
  })

  it('a controlled tip asks to open on the next hover after the parent refused a request while it closed', async () => {
    const user = userEvent.setup()
    const asked: boolean[] = []
    function Host() {
      const [open, setOpen] = useState(true)
      const [anchor, setAnchor] = useState<HTMLButtonElement | null>(null)
      // The parent moves focus back to the anchor when the tip closes, and refuses the open request that focus makes.
      const returning = useRef(false)
      const onOpenChange = (next: boolean) => {
        asked.push(next)
        if (returning.current) return
        if (!next) {
          returning.current = true
          anchor?.focus()
          returning.current = false
        }
        setOpen(next)
      }
      return (
        <Tooltip label="Move" open={open} onOpenChange={onOpenChange} anchor={anchor}>
          <button ref={setAnchor}>Tool</button>
        </Tooltip>
      )
    }
    // The focus the parent moves is keyboard focus: the Escape key came before it.
    const matches = Element.prototype.matches
    const keyboardFocus = vi.spyOn(Element.prototype, 'matches').mockImplementation(function (
      this: Element,
      selector: string
    ) {
      return selector === ':focus-visible'
        ? this === document.activeElement
        : matches.call(this, selector)
    })
    try {
      render(<Host />)
      expect(screen.getByRole('tooltip')).toBeInTheDocument()

      await user.keyboard('{Escape}')
      expect(asked).toEqual([false, true])
      expect(screen.queryByRole('tooltip')).toBeNull()
      expect(screen.getByRole('button', { name: 'Tool' })).toHaveFocus()

      asked.length = 0
      await user.hover(screen.getByRole('button', { name: 'Tool' }))

      expect(asked).toEqual([true])
      expect(screen.getByRole('tooltip').textContent).toBe('Move')
    } finally {
      keyboardFocus.mockRestore()
    }
  })

  it('a tip with an enter delay opens at once on keyboard focus and closes when focus leaves', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true })
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime })
    render(
      <>
        <Tooltip label="Move" enterDelay={2000}>
          <button>Tool</button>
        </Tooltip>
        <button>Next</button>
      </>
    )

    await user.tab()
    expect(screen.getByRole('button', { name: 'Tool' })).toHaveFocus()
    expect(screen.getByRole('tooltip').textContent).toBe('Move')

    await user.tab()
    expect(screen.queryByRole('tooltip')).toBeNull()
  })

  it('a tip with an enter delay closes on Escape', async () => {
    const user = userEvent.setup()
    render(
      <Tooltip label="Move" enterDelay={0}>
        <button>Tool</button>
      </Tooltip>
    )
    await user.hover(screen.getByRole('button', { name: 'Tool' }))
    expect(screen.getByRole('tooltip')).toBeInTheDocument()

    await user.keyboard('{Escape}')

    expect(screen.queryByRole('tooltip')).toBeNull()
  })

  it('a tip that labels its child is the name of the control and not its description', () => {
    render(
      <Tooltip label="Delete" labelChild>
        <button>
          <svg aria-hidden="true" />
        </button>
      </Tooltip>
    )

    const button = screen.getByRole('button', { name: 'Delete' })
    expect(button).toHaveAccessibleDescription('')
    expect(button.parentElement?.hasAttribute('aria-describedby')).toBe(false)
    expect(screen.getByRole('tooltip').textContent).toBe('Delete')
  })

  it('a portal tip that labels its child names the control before it opens and does not describe it when open', async () => {
    const user = userEvent.setup()
    render(
      <Tooltip label="Delete" labelChild portal>
        <button>
          <svg aria-hidden="true" />
        </button>
      </Tooltip>
    )
    const button = screen.getByRole('button', { name: 'Delete' })
    expect(screen.queryByRole('tooltip')).toBeNull()

    await user.hover(button)

    expect(screen.getByRole('tooltip').textContent).toBe('Delete')
    expect(screen.getByRole('button', { name: 'Delete' })).toBe(button)
    expect(button).toHaveAccessibleDescription('')
    expect(button.parentElement?.hasAttribute('aria-describedby')).toBe(false)
  })

  it('rich content that labels its child names the control while the tip is open', async () => {
    const user = userEvent.setup()
    render(
      <Tooltip content={<b>Delete the scene</b>} labelChild portal>
        <button>
          <svg aria-hidden="true" />
        </button>
      </Tooltip>
    )
    const button = screen.getByRole('button')

    await user.hover(button)
    expect(screen.getByRole('button', { name: 'Delete the scene' })).toBe(button)

    await user.unhover(button)
    expect(button.hasAttribute('aria-labelledby')).toBe(false)
  })

  it('a control that has a name keeps it', () => {
    render(
      <Tooltip label="Delete" labelChild>
        <button aria-label="Remove the scene">
          <svg aria-hidden="true" />
        </button>
      </Tooltip>
    )

    expect(screen.getByRole('button', { name: 'Remove the scene' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Delete' })).toBeNull()
  })

  it('a tip that labels an outside anchor names the anchor and does not describe it', async () => {
    const user = userEvent.setup()
    const Anchored = () => {
      const [anchor, setAnchor] = useState<HTMLButtonElement | null>(null)
      return (
        <>
          <button ref={setAnchor}>
            <svg aria-hidden="true" />
          </button>
          <Tooltip label="Door" anchor={anchor} labelChild />
        </>
      )
    }
    render(<Anchored />)
    const tile = screen.getByRole('button', { name: 'Door' })

    await user.hover(tile)

    expect(screen.getByRole('tooltip').textContent).toBe('Door')
    expect(tile.hasAttribute('aria-describedby')).toBe(false)
  })

  it('a disabled tip that labels its child gives the control no name', () => {
    render(
      <Tooltip label="Delete" labelChild disabled>
        <button>
          <svg aria-hidden="true" />
        </button>
      </Tooltip>
    )

    expect(screen.queryByRole('button', { name: 'Delete' })).toBeNull()
    expect(screen.getByRole('button').hasAttribute('aria-label')).toBe(false)
  })

  it('a tip that does not label its child leaves the name of the control as it is', async () => {
    const user = userEvent.setup()
    render(
      <Tooltip label="Move" portal>
        <button>Tool</button>
      </Tooltip>
    )

    await user.hover(screen.getByRole('button', { name: 'Tool' }))

    expect(screen.getByRole('button', { name: 'Tool' }).hasAttribute('aria-label')).toBe(false)
    expect(screen.getByRole('button', { name: 'Tool' }).hasAttribute('aria-labelledby')).toBe(false)
  })

  it.each([
    ['start', 'bottom', '400px', '342px'],
    ['center', 'bottom', '420px', '342px'],
    ['end', 'bottom', '440px', '342px'],
    ['start', 'top', '400px', '268px'],
    ['start', 'right', '512px', '300px'],
    ['end', 'right', '512px', '310px'],
    ['end', 'left', '328px', '310px']
  ] as const)(
    'a tip aligned to the %s on the %s lines up with that edge of the anchor',
    async (align, side, left, top) => {
      const user = userEvent.setup()
      layout(box(400, 300, 100, 30), box(0, 0, 60, 20))
      render(
        <Tooltip label="Move" side={side} align={align} keepInViewport>
          <button>Tool</button>
        </Tooltip>
      )

      await user.hover(screen.getByRole('button', { name: 'Tool' }))

      const tip = screen.getByRole('tooltip')
      expect(tip.style.left).toBe(left)
      expect(tip.style.top).toBe(top)
    }
  )

  it.each([
    ['top', { keepInViewport: true }, '272px'],
    ['bottom', { keepInViewport: true }, '338px'],
    ['top', { portal: true }, '292px'],
    ['bottom', { portal: true }, '338px']
  ] as const)(
    'a tip on the %s with a gap of 8 (%o) sits 8 pixels from the anchor',
    async (side, mode, top) => {
      const user = userEvent.setup()
      layout(box(400, 300, 100, 30), box(0, 0, 60, 20))
      render(
        <Tooltip label="Move" side={side} gap={8} {...mode}>
          <button>Tool</button>
        </Tooltip>
      )

      await user.hover(screen.getByRole('button', { name: 'Tool' }))

      expect(screen.getByRole('tooltip').style.top).toBe(top)
    }
  )

  it('a tip in the page takes its gap from a custom property', () => {
    render(
      <Tooltip label="Rare" side="top" gap={8}>
        <button>Rarity</button>
      </Tooltip>
    )

    expect(screen.getByRole('tooltip').style.getPropertyValue('--tt-gap')).toBe('8px')
  })

  it('a tip with no alignment is centred on the anchor', async () => {
    const user = userEvent.setup()
    layout(box(400, 300, 100, 30), box(0, 0, 60, 20))
    render(
      <Tooltip label="Move" side="bottom" keepInViewport>
        <button>Tool</button>
      </Tooltip>
    )

    await user.hover(screen.getByRole('button', { name: 'Tool' }))

    expect(screen.getByRole('tooltip').style.left).toBe('420px')
  })

  it('a snapped tip above its anchor has its lower edge on a whole device pixel', async () => {
    const user = userEvent.setup()
    layout(box(400.3, 300.4, 101, 30), box(0, 0, 60, 24.5))
    render(
      <Tooltip label="Move" side="top" keepInViewport snap>
        <button>Tool</button>
      </Tooltip>
    )

    await user.hover(screen.getByRole('button', { name: 'Tool' }))

    const tip = screen.getByRole('tooltip')
    expect(tip.style.left).toBe('421px')
    expect(tip.style.top).toBe('263.5px')
  })

  it('a snapped tip sits on whole device pixels', async () => {
    const user = userEvent.setup()
    vi.stubGlobal('devicePixelRatio', 2)
    layout(box(400.3, 300.4, 101, 30), box(0, 0, 60.4, 20))
    render(
      <Tooltip label="Move" side="bottom" keepInViewport snap>
        <button>Tool</button>
      </Tooltip>
    )

    await user.hover(screen.getByRole('button', { name: 'Tool' }))

    const tip = screen.getByRole('tooltip')
    expect(tip.style.left).toBe('420.5px')
    expect(tip.style.top).toBe('342.5px')
  })

  it('a tip that is not snapped keeps the measured position', async () => {
    const user = userEvent.setup()
    layout(box(400.3, 300.4, 101, 30), box(0, 0, 60.4, 20))
    render(
      <Tooltip label="Move" side="bottom" keepInViewport>
        <button>Tool</button>
      </Tooltip>
    )

    await user.hover(screen.getByRole('button', { name: 'Tool' }))

    expect(screen.getByRole('tooltip').style.top).toBe('342.4px')
  })

  it('an aligned tip that crosses a viewport edge shifts back inside', async () => {
    const user = userEvent.setup()
    layout(box(window.innerWidth - 40, 300, 30, 30), box(0, 0, 200, 20))
    render(
      <Tooltip label="Move" side="bottom" align="start" keepInViewport>
        <button>Tool</button>
      </Tooltip>
    )

    await user.hover(screen.getByRole('button', { name: 'Tool' }))

    expect(screen.getByRole('tooltip').style.left).toBe(window.innerWidth - 8 - 200 + 'px')
  })
})

describe('when the tip has nothing to say', () => {
  it.each([
    ['an empty label', { label: '' }],
    ['no label and no content', {}],
    ['null content', { content: null }],
    ['empty content', { content: '' }]
  ] as const)('renders the children and no tip for %s', async (_name, props) => {
    const user = userEvent.setup()
    const onOpenChange = vi.fn()
    render(
      <>
        <Tooltip {...props}>
          <button>Tool</button>
        </Tooltip>
        <Tooltip {...props} portal onOpenChange={onOpenChange}>
          <button>Portal</button>
        </Tooltip>
      </>
    )

    expect(screen.getByRole('button', { name: 'Tool' })).toBeInTheDocument()
    expect(screen.queryByRole('tooltip')).toBeNull()
    await user.hover(screen.getByRole('button', { name: 'Portal' }))
    expect(screen.queryByRole('tooltip')).toBeNull()
    expect(onOpenChange).not.toHaveBeenCalled()
    expect(screen.getByRole('button', { name: 'Portal' }).parentElement).not.toHaveAttribute(
      'aria-describedby'
    )
  })

  it('gives a control it would label no empty name', () => {
    render(
      <Tooltip label="" labelChild>
        <button>
          <svg aria-hidden="true" />
        </button>
      </Tooltip>
    )

    expect(screen.getByRole('button')).not.toHaveAttribute('aria-label')
  })
})

describe('when a portal tip is open', () => {
  it('closes on Escape and drops the description from the wrapper', async () => {
    const user = userEvent.setup()
    render(
      <Tooltip label="Move" portal>
        <button>Tool</button>
      </Tooltip>
    )
    const wrapper = screen.getByRole('button', { name: 'Tool' }).parentElement
    await user.hover(screen.getByRole('button', { name: 'Tool' }))
    expect(wrapper).toHaveAttribute('aria-describedby', screen.getByRole('tooltip').id)

    await user.keyboard('{Escape}')

    expect(screen.queryByRole('tooltip')).toBeNull()
    expect(wrapper).not.toHaveAttribute('aria-describedby')
  })

  it('closes on a press outside the anchor and the tip', async () => {
    const user = userEvent.setup()
    render(
      <>
        <Tooltip label="Move" portal>
          <button>Tool</button>
        </Tooltip>
        <button>Elsewhere</button>
      </>
    )
    await user.hover(screen.getByRole('button', { name: 'Tool' }))

    fireEvent.pointerDown(screen.getByRole('button', { name: 'Elsewhere' }))

    expect(screen.queryByRole('tooltip')).toBeNull()
  })

  it('ignores keys other than Escape', async () => {
    const user = userEvent.setup()
    render(
      <Tooltip label="Move" portal>
        <button>Tool</button>
      </Tooltip>
    )
    await user.hover(screen.getByRole('button', { name: 'Tool' }))

    await user.keyboard('{Enter}a')

    expect(screen.getByRole('tooltip')).toBeInTheDocument()
  })
})

describe('when the tip unmounts', () => {
  it('removes the open portal tip from the page', async () => {
    const user = userEvent.setup()
    const { unmount } = render(
      <Tooltip label="Move" portal>
        <button>Tool</button>
      </Tooltip>
    )
    await user.hover(screen.getByRole('button', { name: 'Tool' }))
    expect(screen.getByRole('tooltip')).toBeInTheDocument()

    unmount()

    expect(document.querySelector('[role="tooltip"]')).toBeNull()
  })

  it('never opens from an enter delay that was still running', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true })
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime })
    const onOpenChange = vi.fn()
    const { unmount } = render(
      <Tooltip label="Move" enterDelay={500} onOpenChange={onOpenChange}>
        <button>Tool</button>
      </Tooltip>
    )
    await user.hover(screen.getByRole('button', { name: 'Tool' }))

    unmount()
    act(() => void vi.advanceTimersByTime(1000))

    expect(onOpenChange).not.toHaveBeenCalled()
  })

  it('stops listening to the document once gone', async () => {
    const user = userEvent.setup()
    const onOpenChange = vi.fn()
    const { unmount } = render(
      <Tooltip label="Door" open onOpenChange={onOpenChange}>
        <button>Asset</button>
      </Tooltip>
    )

    unmount()
    await user.keyboard('{Escape}')
    fireEvent.pointerDown(document.body)

    expect(onOpenChange).not.toHaveBeenCalled()
  })

  it('gives an outside anchor back the description it had', async () => {
    const user = userEvent.setup()
    const Anchored = ({ show }: { show: boolean }) => {
      const [anchor, setAnchor] = useState<HTMLButtonElement | null>(null)
      return (
        <>
          <button ref={setAnchor} aria-describedby="hint">
            Tile
          </button>
          <span id="hint">Opens the door</span>
          {show ? <Tooltip label="Door" anchor={anchor} /> : null}
        </>
      )
    }
    const { rerender } = render(<Anchored show />)
    const tile = screen.getByRole('button', { name: 'Tile' })
    await user.hover(tile)
    expect(tile).toHaveAttribute('aria-describedby', `hint ${screen.getByRole('tooltip').id}`)

    tile.setAttribute(
      'aria-describedby',
      tile.getAttribute('aria-describedby') + ' additional-hint'
    )
    rerender(<Anchored show={false} />)

    expect(tile).toHaveAttribute('aria-describedby', 'hint additional-hint')
    expect(screen.queryByRole('tooltip')).toBeNull()
  })

  it('removes the name it gave the control', () => {
    const { rerender } = render(
      <Tooltip label="Delete" labelChild>
        <button>
          <svg aria-hidden="true" />
        </button>
      </Tooltip>
    )
    const button = screen.getByRole('button', { name: 'Delete' })

    rerender(
      <Tooltip label="Delete" labelChild disabled>
        <button>
          <svg aria-hidden="true" />
        </button>
      </Tooltip>
    )

    expect(button).not.toHaveAttribute('aria-label')
  })
})

describe('when a consumer styles the tip', () => {
  it('keeps the legacy hook classes on the wrapper and the tip', () => {
    const { container } = render(
      <Tooltip label="Rare" side="bottom" arrow className="host" tipClassName="host-tip">
        <button>Rarity</button>
      </Tooltip>
    )

    expect(container.firstElementChild).toHaveClass('tt__wrap', 'host')
    expect(screen.getByRole('tooltip')).toHaveClass(
      'tt__tip',
      'tt__tip--bottom',
      'tt__tip--arrow',
      'host-tip'
    )
  })

  it('applies the module classes for the side and each option', () => {
    render(
      <Tooltip content={<b>Rich</b>} side="left" maxWidth={200} bare arrow>
        <button>Rarity</button>
      </Tooltip>
    )

    expect(screen.getByRole('tooltip')).toHaveClass(
      'tip',
      'left',
      'rich',
      'wrapText',
      'bare',
      'arrow'
    )
  })

  it('lets the pointer into an interactive tip', () => {
    render(
      <Tooltip label="Docs" open interactive>
        <button>Rarity</button>
      </Tooltip>
    )

    expect(screen.getByRole('tooltip')).toHaveClass('tip', 'right', 'interactive')
  })

  it('renders on the server with the tip in the page', () => {
    const html = renderToString(
      <Tooltip label="Rare">
        <button>Rarity</button>
      </Tooltip>
    )

    expect(html).toContain('role="tooltip"')
    expect(html).toContain('Rare')
  })
})

describe('when tooltip content or availability changes', () => {
  it.each([false, ''])('uses the label for empty rich content (%s)', (content) => {
    render(
      <Tooltip content={content} label="Save" labelChild>
        <button>Action</button>
      </Tooltip>
    )
    expect(screen.getByRole('tooltip')).toHaveTextContent('Save')
    expect(screen.getByRole('button', { name: 'Save' })).toBeVisible()
  })

  it('renders numeric zero as content', () => {
    render(
      <Tooltip label="Count" content={0}>
        <button>Action</button>
      </Tooltip>
    )
    expect(screen.getByRole('tooltip')).toHaveTextContent(/^0$/)
  })

  it('does not show a shortcut on its own', () => {
    render(
      <Tooltip label="" shortcut="S">
        <button>Action</button>
      </Tooltip>
    )
    expect(screen.queryByRole('tooltip')).toBeNull()
  })

  it('opens when a hovered portal trigger gains content', async () => {
    const user = userEvent.setup()
    const { rerender } = render(
      <Tooltip portal label="">
        <button>Action</button>
      </Tooltip>
    )
    await user.hover(screen.getByRole('button'))
    rerender(
      <Tooltip portal label="Save">
        <button>Action</button>
      </Tooltip>
    )
    expect(screen.getByRole('tooltip')).toHaveTextContent('Save')
  })

  it('keeps an Escape-dismissed tooltip closed when content returns under the pointer', async () => {
    const user = userEvent.setup()
    const trigger = (label: string) => (
      <Tooltip portal label={label}>
        <button>Action</button>
      </Tooltip>
    )
    const { rerender } = render(trigger('Save'))
    const button = screen.getByRole('button')
    await user.hover(button)
    await user.keyboard('{Escape}')
    rerender(trigger(''))
    rerender(trigger('Save'))
    expect(screen.queryByRole('tooltip')).toBeNull()
    await user.unhover(button)
    await user.hover(button)
    expect(screen.getByRole('tooltip')).toHaveTextContent('Save')
  })

  it('cancels the opening delay when disabled', () => {
    vi.useFakeTimers()
    const onOpenChange = vi.fn()
    const { rerender } = render(
      <Tooltip label="Save" enterDelay={50} onOpenChange={onOpenChange}>
        <button>Action</button>
      </Tooltip>
    )
    fireEvent.mouseEnter(screen.getByRole('button'))
    rerender(
      <Tooltip disabled label="Save" enterDelay={50} onOpenChange={onOpenChange}>
        <button>Action</button>
      </Tooltip>
    )
    act(() => vi.advanceTimersByTime(100))
    expect(onOpenChange).not.toHaveBeenCalled()
    expect(screen.queryByRole('tooltip')).toBeNull()
  })

  it('renders the trigger on the server for an open portal', () => {
    const html = renderToString(
      <Tooltip portal open label="Save">
        <button>Action</button>
      </Tooltip>
    )
    expect(html).toContain('Action')
    expect(html).not.toContain('role="tooltip"')
  })
})
