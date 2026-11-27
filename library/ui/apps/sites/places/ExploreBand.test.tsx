import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from 'vitest'
import { fireEvent, render, screen, within } from '@testing-library/react'

import { ExploreBand, type ExploreBandLabels, type ExploreSection } from './ExploreBand'

const labels: ExploreBandLabels = {
  sections: { all: 'Explore all', favourites: 'Favourites', my: 'My places' },
  sectionsGroup: 'Places sections',
  search: 'Search places',
  searchPlaceholder: 'Search places',
  category: 'Category',
  filters: 'Filters',
  closeFilters: 'Close',
  removeFilter: (category) => `Remove filter: ${category}`
}
const categories = [
  { value: 'all', label: 'Any Category' },
  { value: 'music', label: 'Music' }
]

describe('ExploreBand', () => {
  let onCategory: Mock<(value: string) => void>
  let onSearch: Mock<(value: string) => void>

  const mount = ({
    section = 'all',
    category = 'all'
  }: { section?: ExploreSection; category?: string } = {}) =>
    render(
      <ExploreBand
        section={section}
        onSection={vi.fn()}
        search=""
        onSearch={onSearch}
        category={category}
        categories={categories}
        onCategory={onCategory}
        allCategory="all"
        labels={labels}
      >
        <p>Results</p>
      </ExploreBand>
    )

  beforeEach(() => {
    onCategory = vi.fn()
    onSearch = vi.fn()
  })

  afterEach(() => {
    vi.resetAllMocks()
  })

  describe('when the visitor types a search', () => {
    beforeEach(() => {
      mount()
      fireEvent.change(screen.getByRole('searchbox', { name: 'Search places' }), {
        target: { value: 'plaza' }
      })
    })

    it('should hand the text to the host', () => {
      expect(onSearch).toHaveBeenCalledWith('plaza')
    })
  })

  describe('when the visitor picks a category from the toolbar', () => {
    beforeEach(() => {
      mount()
      const [select] = screen.getAllByRole('combobox', { name: 'Category' })
      fireEvent.change(select!, { target: { value: 'music' } })
    })

    it('should hand the category to the host', () => {
      expect(onCategory).toHaveBeenCalledWith('music')
    })
  })

  describe('when a category is chosen', () => {
    beforeEach(() => {
      mount({ category: 'music' })
    })

    it('should show the chip as the shared light button', () => {
      const chip = screen.getByRole('button', { name: 'Remove filter: Music' })
      expect(chip).toHaveAttribute('data-variant', 'inverse')
      expect(chip).toHaveAttribute('data-size', 'sm')
      expect(chip).toHaveTextContent('Music')
    })

    describe('and the visitor presses the chip', () => {
      beforeEach(() => {
        fireEvent.click(screen.getByRole('button', { name: 'Remove filter: Music' }))
      })

      it('should go back to every category', () => {
        expect(onCategory).toHaveBeenCalledWith('all')
      })
    })
  })

  describe('when no category is chosen', () => {
    beforeEach(() => {
      mount()
    })

    it('should show no chip', () => {
      expect(screen.queryByRole('button', { name: /Remove filter/ })).toBeNull()
    })
  })

  describe('when the visitor opens the filters', () => {
    beforeEach(() => {
      mount()
      fireEvent.click(screen.getByRole('button', { name: 'Filters' }))
    })

    it('should draw the filters button as the shared dark icon button', () => {
      const filters = screen.getByRole('button', { name: 'Filters' })
      expect(filters).toHaveAttribute('data-variant', 'dark')
      expect(filters).toHaveAttribute('data-icon-only', 'true')
      expect(filters).toHaveAttribute('aria-haspopup', 'dialog')
    })

    it('should open the filters drawer', () => {
      expect(screen.getByRole('dialog', { name: 'Filters' })).toBeInTheDocument()
    })

    describe('and picks a category in it', () => {
      beforeEach(() => {
        const drawer = screen.getByRole('dialog', { name: 'Filters' })
        fireEvent.change(within(drawer).getByRole('combobox', { name: 'Category' }), {
          target: { value: 'music' }
        })
      })

      it('should hand the category to the host', () => {
        expect(onCategory).toHaveBeenCalledWith('music')
      })
    })

    describe('and presses its close button', () => {
      beforeEach(() => {
        fireEvent.click(screen.getByRole('button', { name: 'Close' }))
      })

      it('should close the drawer', () => {
        expect(screen.queryByRole('dialog', { name: 'Filters' })).toBeNull()
      })
    })
  })

  describe('when the section is not Explore all', () => {
    beforeEach(() => {
      mount({ section: 'favourites', category: 'music' })
    })

    it('should offer neither the search, the filters nor the chip', () => {
      expect(screen.queryByRole('searchbox', { name: 'Search places' })).toBeNull()
      expect(screen.queryByRole('button', { name: 'Filters' })).toBeNull()
      expect(screen.queryByRole('button', { name: /Remove filter/ })).toBeNull()
    })
  })
})
