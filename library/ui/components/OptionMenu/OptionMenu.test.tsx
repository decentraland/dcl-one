import '@testing-library/jest-dom/vitest'
import { act, fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { OptionMenu, type OptionSection } from './OptionMenu'

const sections = (onSort = vi.fn(), onView = vi.fn()): OptionSection[] => [
  {
    label: 'Sort by',
    options: [
      { id: 'newest', label: 'Newest' },
      { id: 'oldest', label: 'Oldest' }
    ],
    value: 'newest',
    onChange: onSort
  },
  { label: 'View', options: [{ id: 'all', label: 'All Items' }], value: 'none', onChange: onView }
]

describe('OptionMenu', () => {
  it('is a menu of radio options, with one check for each section that has a chosen option', () => {
    const view = render(<OptionMenu sections={sections()} onClose={() => {}} className="mine" />)
    const menu = screen.getByRole('menu')
    expect(menu).toHaveClass('mine')
    expect(screen.getByText('Sort by')).toBeInTheDocument()
    expect(screen.getByRole('menuitemradio', { name: 'Newest' })).toHaveAttribute('aria-checked', 'true')
    expect(screen.getByRole('menuitemradio', { name: 'Oldest' })).toHaveAttribute('aria-checked', 'false')
    expect(screen.getByRole('menuitemradio', { name: 'All Items' })).toHaveAttribute('aria-checked', 'false')
    expect(view.container.querySelectorAll('svg')).toHaveLength(1)
    expect(screen.getByRole('menuitemradio', { name: 'Newest' }).querySelector('svg')).not.toBeNull()
  })

  it('draws a rule before every section but the first, and nothing for no sections', () => {
    const view = render(<OptionMenu sections={sections()} onClose={() => {}} />)
    expect(view.container.querySelectorAll('span[aria-hidden="true"]:empty')).toHaveLength(1)
    view.rerender(<OptionMenu sections={[]} onClose={() => {}} />)
    expect(screen.getByRole('menu')).toBeEmptyDOMElement()
  })

  it("reports the option pressed to its own section and leaves closing to the caller", () => {
    const onSort = vi.fn()
    const onView = vi.fn()
    const onClose = vi.fn()
    render(<OptionMenu sections={sections(onSort, onView)} onClose={onClose} />)
    fireEvent.click(screen.getByRole('menuitemradio', { name: 'Oldest' }))
    expect(onSort).toHaveBeenCalledWith('oldest')
    expect(onView).not.toHaveBeenCalled()
    expect(onClose).not.toHaveBeenCalled()
  })

  it("draws the caller's mark in place of the check", () => {
    const view = render(<OptionMenu sections={sections()} onClose={() => {}} check={<i data-testid="mark" />} />)
    expect(screen.getByTestId('mark').parentElement).toHaveAttribute('aria-hidden', 'true')
    expect(view.container.querySelector('svg')).toBeNull()
  })

  it('closes on a press outside its holder, and not on a press inside the list or beside it in the holder', () => {
    const onClose = vi.fn()
    render(
      <>
        <div>
          <button type="button">toggle</button>
          <OptionMenu sections={sections()} onClose={onClose} />
        </div>
        <p>outside</p>
      </>
    )
    fireEvent.mouseDown(screen.getByRole('menuitemradio', { name: 'Newest' }))
    fireEvent.mouseDown(screen.getByRole('button', { name: 'toggle' }))
    expect(onClose).not.toHaveBeenCalled()
    fireEvent.mouseDown(screen.getByText('outside'))
    expect(onClose).toHaveBeenCalledTimes(1)
  })

  it("hands the host a closer as it mounts and runs the host's cleanup as it leaves", () => {
    const off = vi.fn()
    let closer: (() => void) | undefined
    const whileOpen = vi.fn((close: () => void) => {
      closer = close
      return off
    })
    const onClose = vi.fn()
    const view = render(<OptionMenu sections={sections()} onClose={onClose} whileOpen={whileOpen} />)
    expect(whileOpen).toHaveBeenCalledTimes(1)
    act(() => closer?.())
    expect(onClose).toHaveBeenCalledTimes(1)
    expect(off).not.toHaveBeenCalled()
    view.unmount()
    expect(off).toHaveBeenCalledTimes(1)
  })

  it('stops hearing presses once it leaves', () => {
    const onClose = vi.fn()
    const view = render(<OptionMenu sections={sections()} onClose={onClose} />)
    view.unmount()
    fireEvent.mouseDown(document.body)
    expect(onClose).not.toHaveBeenCalled()
  })
})
