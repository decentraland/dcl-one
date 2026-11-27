import { readFileSync } from 'node:fs'
import { act, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useState } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { Tabs } from '../Tabs/Tabs'
import { TabsBar } from './TabsBar'

function ButtonTabs({ orientation }: { orientation?: 'horizontal' | 'vertical' }) {
  const [value, setValue] = useState('all')
  return (
    <TabsBar label="Events" variant="pill" orientation={orientation}>
      {['all', 'my', 'past'].map((id) => (
        <Tabs key={id} label={id} variant="pill" selected={value === id} onClick={() => setValue(id)} />
      ))}
    </TabsBar>
  )
}

describe('TabsBar', () => {
  describe('when rendered', () => {
    it('is a named tab list', () => {
      render(
        <TabsBar label="Events">
          <Tabs label="All" href="/a" selected />
        </TabsBar>
      )
      expect(screen.getByRole('tablist', { name: 'Events' })).toContainElement(screen.getByRole('tab'))
    })
  })

  describe('when the arrows move between button tabs', () => {
    it('focuses and selects the next tab, wrapping at the ends', async () => {
      render(<ButtonTabs />)
      await userEvent.tab()
      expect(screen.getByRole('tab', { name: 'all' })).toHaveFocus()
      await userEvent.keyboard('{ArrowRight}')
      expect(screen.getByRole('tab', { name: 'my' })).toHaveFocus()
      expect(screen.getByRole('tab', { name: 'my' })).toHaveAttribute('aria-selected', 'true')
      await userEvent.keyboard('{ArrowLeft}{ArrowLeft}')
      expect(screen.getByRole('tab', { name: 'past' })).toHaveFocus()
      await userEvent.keyboard('{Home}')
      expect(screen.getByRole('tab', { name: 'all' })).toHaveAttribute('aria-selected', 'true')
      await userEvent.keyboard('{End}')
      expect(screen.getByRole('tab', { name: 'past' })).toHaveFocus()
    })
  })

  describe('when the arrows move between link tabs', () => {
    it('moves focus only and leaves the selection to the link', async () => {
      render(
        <TabsBar label="Events">
          <Tabs label="All" href="#all" selected />
          <Tabs label="Mine" href="#mine" />
        </TabsBar>
      )
      await userEvent.tab()
      await userEvent.keyboard('{ArrowRight}')
      expect(screen.getByRole('tab', { name: 'Mine' })).toHaveFocus()
      expect(screen.getByRole('tab', { name: 'All' })).toHaveAttribute('aria-selected', 'true')
    })
  })

  describe('when it has an inset and scrolls', () => {
    it('adds the inset and scroller classes after the variant, keeping the caller class', () => {
      render(
        <TabsBar label="Profile sections" inset="content" scrollable className="frame">
          <Tabs label="Overview" href="/p" selected />
        </TabsBar>
      )
      const bar = screen.getByRole('tablist', { name: 'Profile sections' })
      expect(bar.className.split(' ')).toEqual(['root', 'underline', 'insetContent', 'scrollable', 'frame'])
    })
  })

  describe('when it has neither', () => {
    it('keeps only the variant class', () => {
      render(
        <TabsBar label="Events" variant="pill">
          <Tabs label="All" variant="pill" selected />
        </TabsBar>
      )
      expect(screen.getByRole('tablist').className.split(' ')).toEqual(['root', 'pill'])
    })
  })

  describe('when it scrolls', () => {
    const rule = (file: string, selector: string) => {
      const css = readFileSync(new URL(file, import.meta.url), 'utf8')
      const start = css.indexOf(`${selector} {`)
      return start < 0 ? '' : css.slice(start, css.indexOf('}', start))
    }

    it('adds no block padding, so the selected indicator stays on the bar bottom rule', () => {
      const scrollable = rule('./TabsBar.module.css', '.scrollable')
      expect(scrollable).toContain('overflow-x: auto')
      expect(scrollable).not.toMatch(/padding/)
    })

    it('draws the underline tab focus ring inside the tab, where the scroller does not clip it', () => {
      expect(rule('../Tabs/Tabs.module.css', '.underline:focus-visible')).toContain('outline-offset: -2px')
    })
  })
  describe('when it takes the hud appearance', () => {
    function Strip({ onPick }: { onPick?: (id: string) => void }) {
      const [value, setValue] = useState('wearables')
      const pick = (id: string) => {
        setValue(id)
        onPick?.(id)
      }
      return (
        <TabsBar appearance="hud" variant="section" fill label="Backpack sections" className="placed">
          <Tabs appearance="hud" label="Wearables" selected={value === 'wearables'} onClick={() => pick('wearables')} />
          <Tabs appearance="hud" label="Heading" disabled />
          <Tabs appearance="hud" label="Emotes" selected={value === 'emotes'} onClick={() => pick('emotes')} />
        </TabsBar>
      )
    }

    it('is a named tab list with the variant and fill classes, and tabs without the default look', () => {
      render(<Strip />)
      const list = screen.getByRole('tablist', { name: 'Backpack sections' })
      expect(list.className).toMatch(/hudSection/)
      expect(list.className).toMatch(/hudFill/)
      expect(list.className).toMatch(/placed/)
      expect(list.className).not.toMatch(/underline/)
      const selected = screen.getByRole('tab', { name: 'Wearables', selected: true })
      expect(selected.className).toMatch(/hud/)
      expect(selected.className).not.toMatch(/root|underline/)
      expect(selected).toHaveAttribute('tabindex', '0')
      expect(screen.getByRole('tab', { name: 'Emotes' })).toHaveAttribute('tabindex', '-1')
    })

    it('selects with the arrows and skips a disabled tab, wrapping round', async () => {
      const picked: string[] = []
      render(<Strip onPick={(id) => picked.push(id)} />)
      screen.getByRole('tab', { name: 'Wearables' }).focus()
      await userEvent.keyboard('{ArrowRight}')
      expect(screen.getByRole('tab', { name: 'Emotes', selected: true })).toHaveFocus()
      await userEvent.keyboard('{ArrowRight}')
      expect(screen.getByRole('tab', { name: 'Wearables', selected: true })).toHaveFocus()
      await userEvent.keyboard('{End}')
      expect(screen.getByRole('tab', { name: 'Emotes', selected: true })).toHaveFocus()
      expect(picked).toEqual(['emotes', 'wearables', 'emotes'])
    })

    it('falls back to the pill look for a variant the hud does not draw, and to no label attribute without one', () => {
      render(
        <TabsBar appearance="hud" variant="compact" label="">
          <Tabs appearance="hud" label="One" selected />
        </TabsBar>
      )
      const list = screen.getByRole('tablist')
      expect(list.className).toMatch(/hudPill/)
      expect(list).not.toHaveAttribute('aria-label')
    })
  })

  describe('when it is vertical', () => {
    /* Stands in for the browser's media query: true is the desktop column, false the wrapped rows. */
    function mockColumn(matches: boolean) {
      const listeners = new Set<(event: MediaQueryListEvent) => void>()
      const query = {
        matches,
        addEventListener: (_: string, listener: (event: MediaQueryListEvent) => void) => listeners.add(listener),
        removeEventListener: (_: string, listener: (event: MediaQueryListEvent) => void) => listeners.delete(listener)
      }
      vi.stubGlobal(
        'matchMedia',
        vi.fn((media: string) => {
          expect(media).toBe('(min-width: 1024px)')
          return query
        })
      )
      return {
        listeners,
        change(next: boolean) {
          query.matches = next
          for (const listener of listeners) listener({ matches: next } as MediaQueryListEvent)
        }
      }
    }

    afterEach(() => {
      vi.unstubAllGlobals()
    })

    it('adds the vertical class after the variant and marks the column vertical', () => {
      mockColumn(true)
      render(<ButtonTabs orientation="vertical" />)
      const list = screen.getByRole('tablist', { name: 'Events' })
      expect(list.className.split(' ')).toEqual(['root', 'pill', 'vertical'])
      expect(list).toHaveAttribute('aria-orientation', 'vertical')
    })

    it('moves and selects with Up and Down, wrapping at the ends, and keeps Home and End', async () => {
      mockColumn(true)
      render(<ButtonTabs orientation="vertical" />)
      await userEvent.tab()
      await userEvent.keyboard('{ArrowDown}')
      expect(screen.getByRole('tab', { name: 'my', selected: true })).toHaveFocus()
      await userEvent.keyboard('{ArrowUp}{ArrowUp}')
      expect(screen.getByRole('tab', { name: 'past', selected: true })).toHaveFocus()
      await userEvent.keyboard('{ArrowDown}')
      expect(screen.getByRole('tab', { name: 'all', selected: true })).toHaveFocus()
      await userEvent.keyboard('{End}')
      expect(screen.getByRole('tab', { name: 'past', selected: true })).toHaveFocus()
      await userEvent.keyboard('{Home}')
      expect(screen.getByRole('tab', { name: 'all', selected: true })).toHaveFocus()
    })

    it('keeps Left and Right for the rows it wraps onto on narrow screens', async () => {
      mockColumn(false)
      render(<ButtonTabs orientation="vertical" />)
      await userEvent.tab()
      await userEvent.keyboard('{ArrowRight}')
      expect(screen.getByRole('tab', { name: 'my', selected: true })).toHaveFocus()
      await userEvent.keyboard('{ArrowLeft}{ArrowLeft}')
      expect(screen.getByRole('tab', { name: 'past', selected: true })).toHaveFocus()
    })

    it('drops aria-orientation while the tabs are rows and restores it when the column returns', () => {
      const column = mockColumn(false)
      const { unmount } = render(<ButtonTabs orientation="vertical" />)
      const list = screen.getByRole('tablist')
      expect(list).not.toHaveAttribute('aria-orientation')
      act(() => column.change(true))
      expect(list).toHaveAttribute('aria-orientation', 'vertical')
      act(() => column.change(false))
      expect(list).not.toHaveAttribute('aria-orientation')
      unmount()
      expect(column.listeners.size).toBe(0)
    })

    it('treats a browser without matchMedia as the desktop column', () => {
      vi.stubGlobal('matchMedia', undefined)
      render(<ButtonTabs orientation="vertical" />)
      expect(screen.getByRole('tablist')).toHaveAttribute('aria-orientation', 'vertical')
    })

    it('is ignored by the hud appearance, which keeps its own strip and keys', async () => {
      const media = mockColumn(true)
      render(
        <TabsBar appearance="hud" orientation="vertical" label="Strip">
          <Tabs appearance="hud" label="One" selected />
          <Tabs appearance="hud" label="Two" />
        </TabsBar>
      )
      const list = screen.getByRole('tablist')
      expect(list.className).not.toMatch(/vertical/)
      expect(list).not.toHaveAttribute('aria-orientation')
      expect(media.listeners.size).toBe(0)
      screen.getByRole('tab', { name: 'One' }).focus()
      await userEvent.keyboard('{ArrowDown}')
      expect(screen.getByRole('tab', { name: 'One' })).toHaveFocus()
    })
  })

  describe('when it is horizontal, the default', () => {
    it('ignores Up and Down, has no aria-orientation and never reads the media query', async () => {
      const matchMedia = vi.fn()
      vi.stubGlobal('matchMedia', matchMedia)
      render(<ButtonTabs />)
      await userEvent.tab()
      await userEvent.keyboard('{ArrowDown}{ArrowUp}')
      expect(screen.getByRole('tab', { name: 'all', selected: true })).toHaveFocus()
      expect(screen.getByRole('tablist')).not.toHaveAttribute('aria-orientation')
      expect(matchMedia).not.toHaveBeenCalled()
      vi.unstubAllGlobals()
    })
  })
})
