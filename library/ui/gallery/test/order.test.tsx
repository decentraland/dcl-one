import { fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { componentGroups, groupEntries } from '../src/componentPages'
import { foundationPages } from '../src/foundations/FoundationPages'
import { Canvas } from '../src/frame/Canvas'
import { hudSections } from '../src/hudPages'
import { compareLabels, sortByLabel } from '../src/order'
import type { GalleryEntry } from '../src/registry'

// The generated Review's catalog is in capture order; these also differ by case alone and carry digits.
vi.stubGlobal('__GENERATED_COMPONENTS__', [
  { name: 'Tabs', variants: 1 },
  { name: 'button', variants: 1 },
  { name: 'Item10', variants: 1 },
  { name: 'Button', variants: 2 },
  { name: 'Item2', variants: 1 }
])
vi.stubGlobal('__GENERATED_REVIEW_URL__', '../dcl-ui-review/')
vi.stubGlobal(
  'ResizeObserver',
  class {
    observe() {}
    disconnect() {}
  }
)
const { Shell } = await import('../src/shell/Shell')

// The order the gallery promises, stated apart from the code that produces it.
const alphabetical = (labels: readonly string[]) =>
  [...labels].sort((a, b) => a.localeCompare(b, 'en', { sensitivity: 'base', numeric: true }))
const page = (name: string, group: string): GalleryEntry => ({
  name,
  group,
  summary: '',
  components: [name],
  examples: []
})
const labels = (links: Iterable<Element>) =>
  [...links].map((a) => a.querySelector('span')!.textContent)
const nav = () => screen.getByRole('navigation', { name: 'Gallery' })
/** The labels a sidebar group lists, in the order it lists them. */
const listed = (group: string) => {
  const button = screen.getByRole('button', { name: group })
  return labels(
    document.getElementById(button.getAttribute('aria-controls')!)!.querySelectorAll('a')
  )
}
const figmaListed = () =>
  labels(
    screen
      .getByText('Figma reference', { selector: 'summary' })
      .closest('details')!
      .querySelectorAll('a')
  )
const filter = (text: string) =>
  fireEvent.change(screen.getByRole('searchbox', { name: 'Filter' }), { target: { value: text } })

afterEach(() => window.localStorage.clear())

describe('when labels are put in the gallery order', () => {
  it('should ignore case, and order labels that differ by case alone the same way from any start', () => {
    expect(sortByLabel(['tabs', 'Chip', 'button', 'Avatar'], (label) => label)).toEqual([
      'Avatar',
      'button',
      'Chip',
      'tabs'
    ])
    const sameLetters = ['BUTTON', 'Button', 'button']
    for (const start of [
      ['button', 'BUTTON', 'Button'],
      ['Button', 'button', 'BUTTON'],
      sameLetters
    ])
      expect(sortByLabel(start, (label) => label)).toEqual(sameLetters)
    expect(compareLabels('Button', 'Button')).toBe(0)
  })

  it('should order digits by their value, not character by character', () => {
    expect(sortByLabel(['Item10', 'Item2', 'item1', 'Item'], (label) => label)).toEqual([
      'Item',
      'item1',
      'Item2',
      'Item10'
    ])
  })

  it('should return one item or none as given, and leave the list it was handed as it was', () => {
    expect(sortByLabel([], (label: string) => label)).toEqual([])
    expect(sortByLabel(['Only'], (label) => label)).toEqual(['Only'])
    const declared = [{ title: 'b' }, { title: 'A' }] as const
    expect(sortByLabel(declared, (item) => item.title)).toEqual([{ title: 'A' }, { title: 'b' }])
    expect(declared.map((item) => item.title)).toEqual(['b', 'A'])
  })
})

describe('when pages are declared out of order', () => {
  it('should list each group by name, a group of one as it is, the groups where they first appear', () => {
    const groups = groupEntries([
      page('Tabs', 'Second'),
      page('item10', 'First'),
      page('Solo', 'Third'),
      page('Item2', 'First'),
      page('button', 'Second'),
      page('Chip', 'Second'),
      page('Button', 'Second')
    ])
    expect(groups.map((group) => [group.name, group.entries.map((entry) => entry.name)])).toEqual([
      ['Second', ['Button', 'button', 'Chip', 'Tabs']],
      ['First', ['Item2', 'item10']],
      ['Third', ['Solo']]
    ])
  })
})

describe('when the sidebar lists its groups', () => {
  it('should list every group of components by name', () => {
    render(<Shell search="?component=Switch" />)
    for (const group of componentGroups) {
      const names = listed(group.name)
      expect(names).toEqual(alphabetical(group.entries.map((entry) => entry.name)))
    }
    // A group of one is listed too.
    expect(componentGroups.map((group) => group.entries.length)).toContain(1)
  })

  it('should list the foundations by title and the HUD by name, its Showcase first', () => {
    render(<Shell search="?component=Switch" />)
    expect(listed('Foundations')).toEqual(alphabetical(foundationPages.map((item) => item.title)))
    const [overview, ...sections] = listed('HUD')
    expect(overview).toBe('Showcase')
    expect(sections).toEqual(alphabetical(hudSections.map((section) => section.name)))
    expect(sections).toContain('Glyphs')
  })

  it('should list the Figma reference by name, its foundations last', () => {
    render(<Shell search="?component=Switch" />)
    expect(figmaListed()).toEqual(['Button', 'button', 'Item2', 'Item10', 'Tabs', 'Foundations'])
  })

  it('should keep the Showcase above the groups of components', () => {
    render(<Shell search="?component=Switch" />)
    const runtime = labels(nav().querySelectorAll('a[data-source="runtime"]'))
    expect(runtime[foundationPages.length]).toBe('Showcase')
    expect(runtime.slice(foundationPages.length + 1)).toEqual(
      componentGroups.flatMap((group) => listed(group.name))
    )
  })
})

describe('when the Showcase draws what the sidebar lists', () => {
  it('should draw the same groups, and the pages of each in the same order', () => {
    render(<Shell search="?component=Switch" />)
    const sidebar = componentGroups.map((group) => [group.name, listed(group.name)])
    const { container } = render(<Canvas search="?component=Showcase" />)
    const showcase = [...container.querySelectorAll<HTMLElement>('[data-showcase-group]')].map(
      (group) => [
        group.dataset.showcaseGroup,
        [...group.querySelectorAll<HTMLElement>('[data-showcase-page]')].map(
          (section) => section.querySelector('h3')!.textContent
        )
      ]
    )
    expect(showcase).toEqual(sidebar)
  })
})

describe('when a reviewer filters the sidebar', () => {
  it('should keep the matches of every group in order', () => {
    render(<Shell search="?component=Switch" />)
    filter('b')
    const matched = componentGroups
      .map((group) => group.name)
      .filter((name) => screen.queryByRole('button', { name }))
    expect(matched.length).toBeGreaterThan(1)
    for (const name of matched) {
      expect(listed(name).length).toBeGreaterThan(0)
      expect(listed(name)).toEqual(alphabetical(listed(name)))
    }
    expect(listed('HUD')).toEqual(alphabetical(listed('HUD')))
    expect(figmaListed()).toEqual(['Button', 'button', 'Tabs', 'Foundations'])
  })

  it('should list no page and no group of components when nothing matches', () => {
    render(<Shell search="?component=Switch" />)
    filter('no page has this name')
    expect(nav().querySelectorAll('a[data-source="runtime"], a[data-source="hud"]')).toHaveLength(0)
    for (const group of componentGroups)
      expect(screen.queryByRole('button', { name: group.name })).toBeNull()
    expect(listed('Foundations')).toEqual([])
    expect(listed('HUD')).toEqual([])
    expect(figmaListed()).toEqual(['Foundations'])
  })
})
