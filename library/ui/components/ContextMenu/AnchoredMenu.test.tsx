import '@testing-library/jest-dom/vitest'
import { act, fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { AnchoredMenu, anchoredMenuPosition } from './AnchoredMenu'

describe('anchoredMenuPosition', () => {
  const size = { width: 220, height: 207 }
  const bounds = { width: 1920, height: 1080 }

  it('opens right of and above the point', () => {
    expect(anchoredMenuPosition({ x: 500, y: 400 }, size, bounds)).toEqual({ left: 511, top: 382 })
  })

  it('flips to the other side where it would leave the bounds', () => {
    expect(anchoredMenuPosition({ x: 1800, y: 1000 }, size, bounds)).toEqual({
      left: 1800 - 11 - 220,
      top: 1000 + 18 - 207
    })
  })

  it('stays at the origin when the menu is larger than the bounds', () => {
    expect(anchoredMenuPosition({ x: 100, y: 5 }, size, { width: 100, height: 100 })).toEqual({ left: 0, top: 0 })
  })
})

describe('AnchoredMenu', () => {
  const items = [{ label: 'Share' }, { kind: 'separator' as const }, { label: 'Delete', danger: true }]
  // jsdom tracks no input modality: this sets whether an element shows the keyboard's focus (`:focus-visible`)
  const showsFocusRing = (element: Element, shows: boolean) => {
    const matches = element.matches.bind(element)
    return vi
      .spyOn(element, 'matches')
      .mockImplementation((selector) => (selector === ':focus-visible' ? shows : matches(selector)))
  }

  it('is portaled to the body and named', () => {
    const view = render(<AnchoredMenu anchor={{ x: 10, y: 40 }} items={items} onClose={() => {}} aria-label="Photo options" />)
    const menu = screen.getByRole('menu', { name: 'Photo options' })
    expect(view.container).not.toContainElement(menu)
  })

  it('focuses its first row from an opener that shows the keyboard focus, as after Enter', () => {
    render(<button type="button">opener</button>)
    const opener = screen.getByRole('button', { name: 'opener' })
    const ring = showsFocusRing(opener, true)
    opener.focus()
    render(<AnchoredMenu anchor={{ x: 0, y: 0 }} items={items} onClose={() => {}} ignore={opener} />)
    expect(screen.getByRole('menuitem', { name: 'Share' })).toHaveFocus()
    ring.mockRestore()
  })

  it('takes no focus from an opener the pointer pressed, nor without one, so a later key press rings no row', () => {
    render(<button type="button">opener</button>)
    const opener = screen.getByRole('button', { name: 'opener' })
    const ring = showsFocusRing(opener, false)
    opener.focus()
    const first = render(<AnchoredMenu anchor={{ x: 0, y: 0 }} items={items} onClose={() => {}} ignore={opener} />)
    expect(opener).toHaveFocus()
    first.unmount()
    opener.blur()
    const bodyRing = showsFocusRing(document.body, false)
    render(<AnchoredMenu anchor={{ x: 0, y: 0 }} items={items} onClose={() => {}} />)
    expect(document.body).toHaveFocus()
    bodyRing.mockRestore()
    ring.mockRestore()
  })

  it('draws nothing, takes no focus and hears no press while closed', () => {
    const onClose = vi.fn()
    const whileOpen = vi.fn(() => () => {})
    render(<button type="button">opener</button>)
    const opener = screen.getByRole('button', { name: 'opener' })
    opener.focus()
    render(<AnchoredMenu open={false} anchor={{ x: 0, y: 0 }} items={items} onClose={onClose} whileOpen={whileOpen} />)
    expect(screen.queryByRole('menu')).toBeNull()
    expect(opener).toHaveFocus()
    fireEvent.mouseDown(document.body)
    expect(onClose).not.toHaveBeenCalled()
    expect(whileOpen).not.toHaveBeenCalled()
  })

  it('closes before running the entry it was clicked on', () => {
    const order: string[] = []
    render(
      <AnchoredMenu
        anchor={{ x: 10, y: 40 }}
        onClose={() => order.push('close')}
        items={[{ label: 'Share', onClick: () => order.push('run') }, { label: 'Plain' }]}
      />
    )
    fireEvent.click(screen.getByRole('menuitem', { name: 'Share' }))
    fireEvent.click(screen.getByRole('menuitem', { name: 'Plain' }))
    expect(order).toEqual(['close', 'run', 'close'])
  })

  it('does not close from a switch row', () => {
    const onClose = vi.fn()
    const onChange = vi.fn()
    render(<AnchoredMenu anchor={{ x: 0, y: 0 }} onClose={onClose} items={[{ kind: 'toggle', label: 'Mute', onChange }]} />)
    fireEvent.click(screen.getByRole('menuitemcheckbox', { name: 'Mute' }))
    expect(onChange).toHaveBeenCalledWith(true)
    expect(onClose).not.toHaveBeenCalled()
  })

  it('closes on a press outside, and not on a press inside or on the ignored element', () => {
    const onClose = vi.fn()
    const Host = () => {
      const opener = <button type="button">opener</button>
      return (
        <>
          {opener}
          <p>outside</p>
        </>
      )
    }
    render(<Host />)
    const opener = screen.getByRole('button', { name: 'opener' })
    render(<AnchoredMenu anchor={{ x: 0, y: 0 }} onClose={onClose} items={items} ignore={opener} />)
    fireEvent.mouseDown(screen.getByRole('menuitem', { name: 'Share' }))
    fireEvent.mouseDown(opener)
    expect(onClose).not.toHaveBeenCalled()
    fireEvent.mouseDown(screen.getByText('outside'))
    expect(onClose).toHaveBeenCalledTimes(1)
  })

  it('moves the focus with the arrows, around both ends', () => {
    render(<AnchoredMenu anchor={{ x: 0, y: 0 }} onClose={() => {}} items={items} />)
    const share = screen.getByRole('menuitem', { name: 'Share' })
    const remove = screen.getByRole('menuitem', { name: 'Delete' })
    share.focus()
    fireEvent.keyDown(share, { key: 'ArrowDown' })
    expect(remove).toHaveFocus()
    fireEvent.keyDown(remove, { key: 'ArrowDown' })
    expect(share).toHaveFocus()
    fireEvent.keyDown(share, { key: 'ArrowUp' })
    expect(remove).toHaveFocus()
    fireEvent.keyDown(remove, { key: 'Enter' })
    expect(remove).toHaveFocus()
  })

  it('returns the focus to its opener as it closes, unless the focus went elsewhere', () => {
    render(
      <>
        <button type="button">opener</button>
        <button type="button">other</button>
      </>
    )
    const opener = screen.getByRole('button', { name: 'opener' })
    const other = screen.getByRole('button', { name: 'other' })
    opener.focus()
    const first = render(<AnchoredMenu anchor={{ x: 0, y: 0 }} onClose={() => {}} items={items} />)
    first.unmount()
    expect(opener).toHaveFocus()
    const second = render(<AnchoredMenu anchor={{ x: 0, y: 0 }} onClose={() => {}} items={items} ignore={opener} />)
    other.focus()
    second.unmount()
    expect(other).toHaveFocus()
  })

  it("closes from the host's closer and runs its cleanup as it leaves", () => {
    const off = vi.fn()
    let closer: (() => void) | undefined
    const onClose = vi.fn()
    const view = render(
      <AnchoredMenu
        anchor={{ x: 0, y: 0 }}
        onClose={onClose}
        items={items}
        whileOpen={(close) => {
          closer = close
          return off
        }}
      />
    )
    act(() => closer?.())
    expect(onClose).toHaveBeenCalledTimes(1)
    expect(off).not.toHaveBeenCalled()
    view.unmount()
    expect(off).toHaveBeenCalledTimes(1)
  })

  it('places the menu in px of the given scale, and treats a missing or invalid scale as 1', () => {
    const view = render(<AnchoredMenu anchor={{ x: 100, y: 200 }} onClose={() => {}} items={items} />)
    const box = (): HTMLElement => screen.getByRole('menu').parentElement as HTMLElement
    expect(box().style.left).toBe('111px')
    expect(box().style.top).toBe('182px')
    view.rerender(<AnchoredMenu anchor={{ x: 100, y: 200 }} onClose={() => {}} items={items} scale={2} />)
    expect(box().style.left).toBe('122px')
    expect(box().style.top).toBe('164px')
    view.rerender(<AnchoredMenu anchor={{ x: 100, y: 200 }} onClose={() => {}} items={items} scale={0} />)
    expect(box().style.left).toBe('111px')
  })
})
