import { fireEvent, render, screen } from '@testing-library/react'
import type { ComponentProps } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from 'vitest'
import { Tabs } from '../components/Tabs/Tabs'
import { TabsBar } from '../components/TabsBar/TabsBar'
const NavigationTab = Tabs
const NavigationBar = TabsBar
describe('when preserving the panel contract', () => {
  afterEach(() => {
    vi.resetAllMocks()
  })

  describe('and a panel tab receives arrow activation', () => {
    let select: Mock

    beforeEach(() => {
      select = vi.fn()
      render(
        <TabsBar label="Panels">
          <Tabs label="First" selected onClick={select} />
          <Tabs label="Second" />
        </TabsBar>
      )
      const second = screen.getByRole('tab', { name: 'Second' })
      second.focus()
      fireEvent.keyDown(second, { key: 'Home' })
    })

    it('should move focus to the first native panel tab', () => {
      expect(screen.getByRole('tab', { name: 'First' })).toHaveFocus()
    })

    it('should activate the focused tab once', () => {
      expect(select).toHaveBeenCalledOnce()
    })
  })

  describe('and a panel tab has an href', () => {
    it('should preserve the selected panel tab without guessing route intent', () => {
      render(<Tabs label="Panel link" href="#panel" selected />)
      expect(screen.getByRole('tab')).toHaveAttribute('aria-selected', 'true')
    })
  })
})
describe('when explicitly navigating routes', () => {
  let items: NonNullable<ComponentProps<typeof TabsBar>['items']>

  beforeEach(() => {
    items = [
      { id: 'a', label: 'Alpha', href: '/alpha' },
      { id: 'b', label: 'Beta', href: '/beta' }
    ]
  })

  afterEach(() => {
    vi.resetAllMocks()
  })

  describe('and a single navigation tab is selected', () => {
    beforeEach(() => {
      render(
        <NavigationTab mode="navigation" label="Route" href="/route" selected target="_blank" />
      )
    })

    it('should expose a current-page anchor', () => {
      expect(screen.getByRole('link')).toHaveAttribute('aria-current', 'page')
    })

    it('should not expose panel tab semantics', () => {
      expect(screen.queryByRole('tab')).toBeNull()
      expect(screen.getByRole('link')).not.toHaveAttribute('tabindex', '-1')
    })

    it('should protect the opener of a new-tab target', () => {
      expect(screen.getByRole('link')).toHaveAttribute('rel', 'noopener noreferrer')
    })
  })

  describe('and a navigation bar renders controlled items with app-owned controls', () => {
    beforeEach(() => {
      render(
        <NavigationBar
          mode="navigation"
          label="Store"
          page="shop"
          items={items}
          activeId="b"
          search={<input aria-label="Search" />}
          actions={<button>Filter</button>}
        />
      )
    })

    it('should render a named navigation landmark', () => {
      expect(screen.getByRole('navigation')).toHaveAccessibleName('Store')
    })

    it('should mark the active route item as the current page', () => {
      expect(screen.getByRole('link', { name: 'Beta' })).toHaveAttribute('aria-current', 'page')
    })

    it('should render the app-owned search control', () => {
      expect(screen.getByRole('textbox')).toHaveAccessibleName('Search')
    })

    it('should not render a tablist', () => {
      expect(screen.queryByRole('tablist')).toBeNull()
    })
  })

  describe('and a route item is clicked', () => {
    let select: Mock
    let link: HTMLElement

    beforeEach(() => {
      select = vi.fn()
      render(<NavigationBar mode="navigation" items={items} activeId="a" onSelect={select} />)
      link = screen.getByRole('link', { name: 'Beta' })
    })

    describe('and a native modifier is held', () => {
      beforeEach(() => {
        fireEvent.click(link, { ctrlKey: true })
      })

      it('should leave the click to the browser without selecting', () => {
        expect(select).not.toHaveBeenCalled()
      })

      describe('and the item is then clicked without a modifier', () => {
        beforeEach(() => {
          fireEvent.click(link)
        })

        it('should claim the click and select the item id', () => {
          expect(select).toHaveBeenCalledWith('b')
        })
      })
    })
  })
})
