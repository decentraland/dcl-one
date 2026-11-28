import '@testing-library/jest-dom/vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { PaginationDots } from './PaginationDots'

describe('PaginationDots', () => {
  describe('when there is nothing to page through', () => {
    it.each([0, 1, -2, Number.NaN])('should render nothing for %s pages', (count) => {
      const { container } = render(<PaginationDots count={count} current={0} onSelect={() => {}} />)
      expect(container).toBeEmptyDOMElement()
    })
  })

  describe('when there are several pages', () => {
    it('should render one button per page, named "Go to slide N" by default', () => {
      render(<PaginationDots count={3} current={0} onSelect={() => {}} />)
      expect(screen.getAllByRole('button').map((dot) => dot.getAttribute('aria-label'))).toEqual([
        'Go to slide 1',
        'Go to slide 2',
        'Go to slide 3'
      ])
    })

    it('should mark only the current page', () => {
      render(<PaginationDots count={3} current={1} onSelect={() => {}} />)
      const dots = screen.getAllByRole('button')
      expect(dots.map((dot) => dot.getAttribute('aria-current'))).toEqual([null, 'true', null])
    })

    it('should mark no page when the current one is outside the pages', () => {
      render(<PaginationDots count={3} current={5} onSelect={() => {}} />)
      expect(screen.queryByRole('button', { current: true })).not.toBeInTheDocument()
    })

    it('should ask for the page of the pressed dot, counted from zero', () => {
      const onSelect = vi.fn()
      render(<PaginationDots count={4} current={0} onSelect={onSelect} />)
      fireEvent.click(screen.getByRole('button', { name: 'Go to slide 3' }))
      expect(onSelect).toHaveBeenCalledTimes(1)
      expect(onSelect).toHaveBeenCalledWith(2)
    })

    it('should ask for the current page again when its dot is pressed', () => {
      const onSelect = vi.fn()
      render(<PaginationDots count={2} current={1} onSelect={onSelect} />)
      fireEvent.click(screen.getByRole('button', { name: 'Go to slide 2' }))
      expect(onSelect).toHaveBeenCalledWith(1)
    })

    it('should render buttons that do not submit a surrounding form', () => {
      const onSubmit = vi.fn((event: SubmitEvent) => event.preventDefault())
      render(
        <form onSubmit={(event) => onSubmit(event.nativeEvent as SubmitEvent)}>
          <PaginationDots count={2} current={0} onSelect={() => {}} />
        </form>
      )
      fireEvent.click(screen.getByRole('button', { name: 'Go to slide 2' }))
      expect(onSubmit).not.toHaveBeenCalled()
    })
  })

  describe('when the caller names the row and its dots', () => {
    it('should expose the dots as a named group with the caller page names', () => {
      render(
        <PaginationDots
          count={2}
          current={0}
          onSelect={() => {}}
          label="Upcoming pages"
          pageLabel={(page, count) => `Page ${page} of ${count}`}
        />
      )
      const group = screen.getByRole('group', { name: 'Upcoming pages' })
      expect(group).toContainElement(screen.getByRole('button', { name: 'Page 2 of 2' }))
    })
  })

  describe('when the row has no name', () => {
    it('should not add a group role', () => {
      render(<PaginationDots count={2} current={0} onSelect={() => {}} />)
      expect(screen.queryByRole('group')).not.toBeInTheDocument()
    })
  })

  describe('when an appearance is chosen', () => {
    it('should use the Figma look by default', () => {
      const { container } = render(<PaginationDots count={2} current={0} onSelect={() => {}} />)
      expect(container.firstElementChild).toHaveAttribute('data-appearance', 'default')
    })

    it('should carry the plain look and the caller class', () => {
      const { container } = render(
        <PaginationDots count={2} current={0} onSelect={() => {}} appearance="plain" className="extra" />
      )
      expect(container.firstElementChild).toHaveAttribute('data-appearance', 'plain')
      expect(container.firstElementChild).toHaveClass('extra')
    })
  })
})
