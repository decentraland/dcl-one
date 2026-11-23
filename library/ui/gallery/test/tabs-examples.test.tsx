import { fireEvent, render, screen, within } from '@testing-library/react'
import type { ReactNode } from 'react'
import { describe, expect, it } from 'vitest'
import { entries } from '../src/examples'

function mount(page: string, id: string) {
  const example = entries.find((entry) => entry.name === page)!.examples.find((e) => e.id === id)!
  const Render = example.render as () => ReactNode
  return render(<Render />)
}

describe('when a Tabs or TabsBar gallery example is used', () => {
  it('should select the clicked tab, so the selection motion plays', () => {
    mount('Tabs', 'variants')
    for (const variant of ['underline', 'compact', 'pill']) {
      const list = screen.getByRole('tablist', { name: `${variant} tabs` })
      const [first, second] = within(list).getAllByRole('tab')
      expect(first?.getAttribute('aria-selected')).toBe('true')
      fireEvent.click(second!)
      expect(second?.getAttribute('aria-selected')).toBe('true')
      expect(first?.getAttribute('aria-selected')).toBe('false')
    }
  })

  it('should select with the arrow keys', () => {
    mount('Tabs', 'variants')
    const [first, second] = within(
      screen.getByRole('tablist', { name: 'underline tabs' })
    ).getAllByRole('tab')
    fireEvent.keyDown(first!, { key: 'ArrowRight' })
    expect(second?.getAttribute('aria-selected')).toBe('true')
  })

  it('should select a link tab in place, without following it', () => {
    mount('Tabs', 'devices')
    const link = screen.getByRole('tab', { name: 'mobile link' })
    expect(fireEvent.click(link)).toBe(false)
    expect(link.getAttribute('aria-selected')).toBe('true')
  })

  it('should move the current route on click, without navigating', () => {
    mount('Tabs', 'navigation')
    const collections = screen.getByRole('link', { name: 'Collections' })
    expect(fireEvent.click(collections)).toBe(false)
    expect(collections.getAttribute('aria-current')).toBe('page')
    expect(screen.getByRole('link', { name: 'Overview' }).getAttribute('aria-current')).toBeNull()
  })

  it('should move the current route of a navigation TabsBar on click, without navigating', () => {
    mount('TabsBar', 'navigation-create')
    const collections = screen.getAllByRole('link', { name: 'Collections' })[0]!
    expect(fireEvent.click(collections)).toBe(false)
    expect(collections.getAttribute('aria-current')).toBe('page')
  })
})
