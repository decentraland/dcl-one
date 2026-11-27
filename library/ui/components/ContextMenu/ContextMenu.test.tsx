import '@testing-library/jest-dom/vitest'
import { act, fireEvent, render, screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { ContextMenu, Submenu } from './ContextMenu'

describe('ContextMenu', () => {
  it('is a menu named by its label, with a row for each item and a separator', () => {
    const onClick = vi.fn()
    render(
      <ContextMenu
        aria-label="Player options"
        items={[{ label: 'View profile', onClick }, { kind: 'separator' }, { label: 'Block', danger: true }]}
      />
    )
    const menu = screen.getByRole('menu', { name: 'Player options' })
    expect(within(menu).getAllByRole('menuitem')).toHaveLength(2)
    expect(within(menu).getByRole('separator')).toBeInTheDocument()
    const row = screen.getByRole('menuitem', { name: 'View profile' })
    expect(row).toHaveAttribute('type', 'button')
    fireEvent.click(row)
    expect(onClick).toHaveBeenCalledTimes(1)
  })

  it('draws nothing but the surface for an empty list', () => {
    render(<ContextMenu items={[]} />)
    expect(screen.getByRole('menu')).toBeEmptyDOMElement()
  })

  it('shortens a long address in the header and keeps a short one whole', () => {
    render(
      <ContextMenu
        items={[
          { kind: 'header', name: 'Aria', tag: '#a3f1', address: '0x1234567890abcdef1234' },
          { kind: 'header', name: 'Bo', address: '0x12345678' },
          { kind: 'header', name: 'Cy' }
        ]}
      />
    )
    expect(screen.getByText('0x1234…1234')).toBeInTheDocument()
    expect(screen.getByText('0x12345678')).toBeInTheDocument()
    expect(screen.getByText('#a3f1')).toBeInTheDocument()
  })

  it('draws titles and captions as text, not as rows', () => {
    render(
      <ContextMenu
        items={[
          { kind: 'title', label: 'Sort by' },
          { kind: 'caption', label: 'Invited by Aria', avatar: <i data-testid="face" /> }
        ]}
      />
    )
    expect(screen.getByText('Sort by')).toBeInTheDocument()
    expect(screen.getByText('Invited by Aria')).toBeInTheDocument()
    expect(screen.getByTestId('face')).toBeInTheDocument()
    expect(screen.queryByRole('menuitem')).toBeNull()
  })

  it('marks only the checked option of a set', () => {
    const onClick = vi.fn()
    render(
      <ContextMenu
        items={[
          { kind: 'option', label: 'Newest', checked: true },
          { kind: 'option', label: 'Oldest', checked: false, onClick }
        ]}
      />
    )
    const newest = screen.getByRole('menuitemradio', { name: 'Newest' })
    const oldest = screen.getByRole('menuitemradio', { name: 'Oldest' })
    expect(newest).toHaveAttribute('aria-checked', 'true')
    expect(newest.querySelector('svg')).not.toBeNull()
    expect(oldest).toHaveAttribute('aria-checked', 'false')
    expect(oldest.querySelector('svg')).toBeNull()
    fireEvent.click(oldest)
    expect(onClick).toHaveBeenCalledTimes(1)
  })

  it("draws the mark where a row's icon sits, before the label, and keeps that place empty when unchecked", () => {
    render(
      <ContextMenu
        items={[
          { kind: 'option', label: 'All Messages', checked: true },
          { kind: 'option', label: 'None', checked: false }
        ]}
      />
    )
    const all = screen.getByRole('menuitemradio', { name: 'All Messages' })
    const none = screen.getByRole('menuitemradio', { name: 'None' })
    const [allMark, allLabel] = Array.from(all.children)
    expect(allMark).toHaveAttribute('aria-hidden', 'true')
    expect(allMark?.querySelector('svg')).not.toBeNull()
    expect(allLabel).toHaveTextContent('All Messages')
    const [noneMark, noneLabel] = Array.from(none.children)
    expect(noneMark).toBeEmptyDOMElement()
    expect(noneMark?.className).toBe(allMark?.className)
    expect(noneLabel).toHaveTextContent('None')
  })

  it("keeps an option's own icon after the mark", () => {
    render(<ContextMenu items={[{ kind: 'option', label: 'Grid', checked: true, icon: <i data-testid="grid" /> }]} />)
    const row = screen.getByRole('menuitemradio', { name: 'Grid' })
    const [mark, icon] = Array.from(row.children)
    expect(mark?.querySelector('svg')).not.toBeNull()
    expect(icon).toContainElement(screen.getByTestId('grid'))
  })

  it('toggles a switch row from a click beside the switch and reports the new value', () => {
    const onChange = vi.fn()
    render(<ContextMenu items={[{ kind: 'toggle', label: 'Mute', checked: false, onChange }]} />)
    const row = screen.getByRole('menuitemcheckbox', { name: 'Mute' })
    expect(row).toHaveAttribute('aria-checked', 'false')
    fireEvent.click(row)
    expect(onChange).toHaveBeenLastCalledWith(true)
    expect(row).toHaveAttribute('aria-checked', 'true')
  })

  it('toggles once, not twice, from a click on the switch itself', () => {
    const onChange = vi.fn()
    render(<ContextMenu items={[{ kind: 'toggle', label: 'Mute', onChange }]} />)
    fireEvent.click(screen.getByRole('switch'))
    expect(onChange).toHaveBeenCalledTimes(1)
    expect(onChange).toHaveBeenLastCalledWith(true)
  })

  it("follows the owner's value when it changes back", () => {
    const view = render(<ContextMenu items={[{ kind: 'toggle', label: 'Mute', checked: false }]} />)
    fireEvent.click(screen.getByRole('menuitemcheckbox'))
    expect(screen.getByRole('menuitemcheckbox')).toHaveAttribute('aria-checked', 'true')
    view.rerender(<ContextMenu items={[{ kind: 'toggle', label: 'Mute', checked: true }]} />)
    view.rerender(<ContextMenu items={[{ kind: 'toggle', label: 'Mute', checked: false }]} />)
    expect(screen.getByRole('menuitemcheckbox')).toHaveAttribute('aria-checked', 'false')
  })

  it('shows only a chevron for a submenu row without items, hidden from its name', () => {
    const onClick = vi.fn()
    render(<ContextMenu items={[{ kind: 'submenu', label: 'More', onClick }]} />)
    const row = screen.getByRole('menuitem', { name: 'More' })
    expect(row).not.toHaveAttribute('aria-haspopup')
    const chevron = row.lastElementChild
    expect(chevron).toHaveAttribute('aria-hidden', 'true')
    expect(chevron?.querySelector('svg')).not.toBeNull()
    fireEvent.click(row)
    expect(onClick).toHaveBeenCalledTimes(1)
  })

  it('gives a plain row no chevron', () => {
    render(<ContextMenu items={[{ label: 'View profile' }]} />)
    expect(screen.getByRole('menuitem', { name: 'View profile' }).querySelector('svg')).toBeNull()
  })

  it('opens the items of a submenu row beside it and hands the list to whileSubmenuOpen', () => {
    const off = vi.fn()
    const whileSubmenuOpen = vi.fn(() => off)
    render(
      <ContextMenu
        whileSubmenuOpen={whileSubmenuOpen}
        items={[{ kind: 'submenu', label: 'Invite', items: [{ label: 'Pixel Builders' }] }]}
      />
    )
    const row = screen.getByRole('menuitem', { name: 'Invite' })
    expect(row).toHaveAttribute('aria-expanded', 'false')
    expect(whileSubmenuOpen).not.toHaveBeenCalled()
    fireEvent.click(row)
    expect(row).toHaveAttribute('aria-expanded', 'true')
    expect(screen.getByRole('menuitem', { name: 'Pixel Builders' })).toBeInTheDocument()
    expect(whileSubmenuOpen).toHaveBeenCalledTimes(1)
  })

  it('takes the scroll form', () => {
    const view = render(<ContextMenu items={[]} />)
    const plain = screen.getByRole('menu').className
    view.rerender(<ContextMenu items={[]} scroll />)
    expect(screen.getByRole('menu').className).not.toBe(plain)
  })

  it('fits its rows unless given a width, which it takes in px', () => {
    const view = render(<ContextMenu items={[]} />)
    expect(screen.getByRole('menu')).not.toHaveAttribute('style')
    view.rerender(<ContextMenu items={[]} width={260} />)
    expect(screen.getByRole('menu').style.getPropertyValue('--menu-width')).toBe('260px')
    view.rerender(<ContextMenu items={[]} width={0} />)
    expect(screen.getByRole('menu').style.getPropertyValue('--menu-width')).toBe('0px')
  })
})

describe('Submenu', () => {
  const list = (
    <ContextMenu items={[{ label: 'First' }, { label: 'Second' }]} aria-label="Communities" scroll />
  )

  it("keeps its host row's text chevron outside a menu and takes the menu's icon chevron inside one", () => {
    const icon = <i data-testid="people" />
    const view = render(
      <Submenu label="Invite" icon={icon}>
        {list}
      </Submenu>
    )
    const alone = screen.getByRole('menuitem', { name: 'Invite' })
    expect(alone.lastElementChild).toHaveTextContent('›')
    expect(alone.lastElementChild).toHaveAttribute('aria-hidden', 'true')
    expect(alone.querySelector('svg')).toBeNull()
    const aloneIcon = screen.getByTestId('people').parentElement?.className
    view.unmount()
    render(<ContextMenu items={[{ kind: 'submenu', label: 'Invite', icon, items: [{ label: 'First' }] }]} />)
    const inMenu = screen.getByRole('menuitem', { name: 'Invite' })
    expect(inMenu.lastElementChild).toHaveAttribute('aria-hidden', 'true')
    expect(inMenu.lastElementChild?.querySelector('svg')).not.toBeNull()
    expect(inMenu).not.toHaveTextContent('›')
    expect(screen.getByTestId('people').parentElement?.className).not.toBe(aloneIcon)
  })

  it('opens on hover and closes when the pointer leaves', () => {
    render(<Submenu label="Invite">{list}</Submenu>)
    const row = screen.getByRole('menuitem', { name: 'Invite' })
    expect(row).toHaveAttribute('aria-haspopup', 'menu')
    expect(row).not.toHaveAttribute('aria-controls')
    fireEvent.mouseEnter(row.parentElement as HTMLElement)
    expect(row).toHaveAttribute('aria-expanded', 'true')
    expect(document.getElementById(row.getAttribute('aria-controls') as string)).toContainElement(
      screen.getByRole('menu', { name: 'Communities' })
    )
    fireEvent.mouseLeave(row.parentElement as HTMLElement)
    expect(screen.queryByRole('menu')).toBeNull()
  })

  it('moves the focus to the first item on a keyboard click, and not on a pointer click', () => {
    render(<Submenu label="Invite">{list}</Submenu>)
    const row = screen.getByRole('menuitem', { name: 'Invite' })
    fireEvent.click(row, { detail: 1 })
    expect(screen.getByRole('menuitem', { name: 'First' })).not.toHaveFocus()
    fireEvent.mouseLeave(row.parentElement as HTMLElement)
    fireEvent.click(row, { detail: 0 })
    expect(screen.getByRole('menuitem', { name: 'First' })).toHaveFocus()
  })

  it('closes when the focus leaves the row and its list, and stays open while it moves inside', () => {
    render(
      <>
        <Submenu label="Invite">{list}</Submenu>
        <button type="button">outside</button>
      </>
    )
    const row = screen.getByRole('menuitem', { name: 'Invite' })
    fireEvent.click(row)
    fireEvent.blur(row, { relatedTarget: screen.getByRole('menuitem', { name: 'Second' }) })
    expect(row).toHaveAttribute('aria-expanded', 'true')
    fireEvent.blur(row, { relatedTarget: screen.getByRole('button', { name: 'outside' }) })
    expect(row).toHaveAttribute('aria-expanded', 'false')
  })

  it("closes from the host's closer, returns the focus to the row and runs the cleanup once", () => {
    const off = vi.fn()
    let closer: (() => void) | undefined
    const whileOpen = vi.fn((close: () => void) => {
      closer = close
      return off
    })
    render(
      <Submenu label="Invite" whileOpen={whileOpen}>
        {list}
      </Submenu>
    )
    const row = screen.getByRole('menuitem', { name: 'Invite' })
    fireEvent.click(row, { detail: 0 })
    expect(whileOpen).toHaveBeenCalledTimes(1)
    expect(off).not.toHaveBeenCalled()
    act(() => closer?.())
    expect(row).toHaveAttribute('aria-expanded', 'false')
    expect(row).toHaveFocus()
    expect(off).toHaveBeenCalledTimes(1)
  })

  it('opens and closes without a host closer', () => {
    render(<Submenu label="Invite" icon={<i data-testid="icon" />} className="mine">{list}</Submenu>)
    const row = screen.getByRole('menuitem', { name: 'Invite' })
    expect(row).toHaveClass('mine')
    expect(screen.getByTestId('icon')).toBeInTheDocument()
    fireEvent.click(row)
    fireEvent.mouseLeave(row.parentElement as HTMLElement)
    expect(row).toHaveAttribute('aria-expanded', 'false')
  })

  it('flips to the left and up where the list would leave its clipping ancestor', () => {
    const rect = (right: number, bottom: number) => ({ right, bottom, left: 0, top: 0, width: 0, height: 0, x: 0, y: 0, toJSON: () => ({}) })
    const spy = vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (this: HTMLElement) {
      return this.dataset.clip === 'yes' ? rect(100, 100) : rect(200, 200)
    })
    render(
      <div data-clip="yes" style={{ overflowX: 'hidden', overflowY: 'hidden' }}>
        <Submenu label="Invite">{list}</Submenu>
      </div>
    )
    const row = screen.getByRole('menuitem', { name: 'Invite' })
    fireEvent.click(row)
    const flyout = document.getElementById(row.getAttribute('aria-controls') as string) as HTMLElement
    const flipped = flyout.className
    spy.mockImplementation(() => rect(10, 10))
    fireEvent.mouseLeave(row.parentElement as HTMLElement)
    fireEvent.click(row)
    const straight = (document.getElementById(row.getAttribute('aria-controls') as string) as HTMLElement).className
    spy.mockRestore()
    expect(flipped).not.toBe(straight)
    expect(flipped.split(' ').length).toBe(straight.split(' ').length + 2)
  })
})
