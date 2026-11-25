import '@testing-library/jest-dom/vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { Pager, pageWindow } from './Pager'

describe('pageWindow', () => {
  it('shows every page when there are five or fewer', () => {
    expect(pageWindow(0, 3)).toEqual([0, 1, 2])
  })

  it('starts at the first page until the current one passes the middle', () => {
    expect(pageWindow(2, 9)).toEqual([0, 1, 2, 3, 4])
  })

  it('centres the current page', () => {
    expect(pageWindow(4, 9)).toEqual([2, 3, 4, 5, 6])
  })

  it('clamps at the last page', () => {
    expect(pageWindow(8, 9)).toEqual([4, 5, 6, 7, 8])
  })
})

describe('Pager', () => {
  describe('when there is nothing to page through', () => {
    it.each([0, 1])('renders nothing for %s pages', (count) => {
      const { container } = render(<Pager page={0} count={count} onChange={() => {}} />)
      expect(container).toBeEmptyDOMElement()
    })
  })

  describe('when there are several pages', () => {
    it('is a navigation with the pages numbered from one and the current one marked', () => {
      render(<Pager page={2} count={9} onChange={() => {}} className="mine" />)
      const nav = screen.getByRole('navigation', { name: 'Pages' })
      expect(nav).toHaveClass('mine')
      expect(nav).toHaveAttribute('data-variant', 'backpack')
      expect(screen.getByRole('button', { name: '3' })).toHaveAttribute('aria-current', 'page')
      expect(screen.getByRole('button', { name: '1' })).not.toHaveAttribute('aria-current')
      expect(screen.queryByText('...')).toBeNull()
    })

    it('goes to the page that is clicked', () => {
      const onChange = vi.fn()
      render(<Pager page={2} count={9} onChange={onChange} />)
      fireEvent.click(screen.getByRole('button', { name: '5' }))
      expect(onChange).toHaveBeenCalledWith(4)
    })

    it('steps back and forward with the arrows', () => {
      const onChange = vi.fn()
      render(<Pager page={2} count={9} onChange={onChange} />)
      fireEvent.click(screen.getByRole('button', { name: 'Previous page' }))
      fireEvent.click(screen.getByRole('button', { name: 'Next page' }))
      expect(onChange).toHaveBeenNthCalledWith(1, 1)
      expect(onChange).toHaveBeenNthCalledWith(2, 3)
    })

    it('wraps from the first page to the last and back', () => {
      const onChange = vi.fn()
      const { rerender } = render(<Pager page={0} count={9} onChange={onChange} />)
      fireEvent.click(screen.getByRole('button', { name: 'Previous page' }))
      rerender(<Pager page={8} count={9} onChange={onChange} />)
      fireEvent.click(screen.getByRole('button', { name: 'Next page' }))
      expect(onChange).toHaveBeenNthCalledWith(1, 8)
      expect(onChange).toHaveBeenNthCalledWith(2, 0)
    })

    it("draws the caller's glyph in both arrows", () => {
      render(<Pager page={0} count={3} onChange={() => {}} arrowIcon={<i data-testid="glyph" />} />)
      expect(screen.getAllByTestId('glyph')).toHaveLength(2)
    })
  })

  describe('when it is the shop variant', () => {
    it('shows an ellipsis while pages follow the window', () => {
      render(<Pager variant="shop" page={0} count={9} onChange={() => {}} />)
      expect(screen.getByRole('navigation')).toHaveAttribute('data-variant', 'shop')
      expect(screen.getByText('...')).toHaveAttribute('aria-hidden', 'true')
    })

    it('shows none once the window reaches the last page', () => {
      render(<Pager variant="shop" page={8} count={9} onChange={() => {}} />)
      expect(screen.queryByText('...')).toBeNull()
    })
  })
})
