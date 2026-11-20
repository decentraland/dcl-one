import { createElement, type ComponentType } from 'react'
import { render } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { claimProblems } from '../src/coverage'
import { entries } from '../src/examples'
import { foundationPages } from '../src/foundations/FoundationPages'
import type { GalleryEntry } from '../src/registry'

// Every component the package root exports records its name when it mounts, so a page's `components` claim is
// checked against what its examples really render through the public entry.
const instrumentation = vi.hoisted(() => ({ mounted: new Set<string>() }))
vi.mock('@dcl/one-ui', async (importOriginal) => {
  const actual = await importOriginal<Record<string, unknown>>()
  return Object.fromEntries(
    Object.entries(actual).map(([name, value]) => {
      if (!/^[A-Z]/.test(name) || typeof value !== 'function') return [name, value]
      const Original = value as ComponentType<Record<string, unknown>>
      const Recorded = (props: Record<string, unknown>) => {
        instrumentation.mounted.add(name)
        return createElement(Original, props)
      }
      return [name, Recorded]
    })
  )
})

function mountedBy(entry: Pick<GalleryEntry, 'examples'>): Set<string> {
  instrumentation.mounted.clear()
  for (const example of entry.examples) render(<>{example.render()}</>).unmount()
  return new Set(instrumentation.mounted)
}

const examples = entries.flatMap((entry) =>
  entry.examples.map((example) => [`${entry.name} ${example.id}`, example] as const)
)

describe('when the gallery renders its examples with deterministic data', () => {
  afterEach(() => vi.unstubAllGlobals())

  it.each(examples)('should render %s without network access or errors', (_, example) => {
    const fetch = vi.fn()
    vi.stubGlobal('fetch', fetch)
    const errors = vi.spyOn(console, 'error').mockImplementation(() => {})
    const { container } = render(<>{example.render()}</>)
    expect(container.childElementCount).toBeGreaterThan(0)
    expect(fetch).not.toHaveBeenCalled()
    expect(errors).not.toHaveBeenCalled()
    errors.mockRestore()
  })

  it.each(foundationPages.map((page) => [page.name, page] as const))(
    'should render the %s foundations page',
    (_, page) => {
      const { container } = render(<>{page.render()}</>)
      expect(container.textContent).not.toBe('')
    }
  )

  it('should give every page and example a unique id', () => {
    const ids = examples.map(([id]) => id)
    expect(new Set(ids).size).toBe(ids.length)
  })
})

describe('when a page claims the public exports it shows', () => {
  beforeEach(() => instrumentation.mounted.clear())

  it.each(entries.map((entry) => [entry.name, entry] as const))(
    'should mount every export the %s page claims',
    (_, entry) => {
      expect(claimProblems(entry, mountedBy(entry))).toEqual([])
    }
  )

  it('should fail a page whose only example renders a placeholder instead of its claimed exports', () => {
    const fake = {
      name: 'EventCard',
      components: ['EventCard', 'EventCardHost'],
      examples: [{ id: 'fake', title: 'Fake', render: () => <div>Placeholder</div> }]
    }
    expect(claimProblems(fake, mountedBy(fake))).toEqual([
      'EventCard: claims EventCard, but no example mounts it',
      'EventCard: claims EventCardHost, but no example mounts it'
    ])
  })
})
