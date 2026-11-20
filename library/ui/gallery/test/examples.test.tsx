import { createRef, forwardRef, memo, type ComponentType, type Ref } from 'react'
import { render } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { claimProblems, componentExports } from '../src/coverage'
import { entries } from '../src/examples'
import { foundationPages } from '../src/foundations/FoundationPages'
import type { GalleryEntry } from '../src/registry'
import { instrumentComponents } from './instrument'

// Every component the package root exports records its name when it mounts, so a page's `components` claim is
// checked against what its examples really render through the public entry.
const instrumentation = vi.hoisted(() => ({ mounted: new Set<string>() }))
vi.mock('@dcl/one-ui', async (importOriginal) => {
  const { instrumentComponents: instrument } = await import('./instrument')
  return instrument(await importOriginal<Record<string, unknown>>(), (name) =>
    instrumentation.mounted.add(name)
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

describe('when a public export is a memo or forwardRef component', () => {
  it('should count it as a component and record it when an example mounts it', () => {
    const mounted = new Set<string>()
    const helper = () => null
    const value = { label: 'not a component' }
    const exports = instrumentComponents(
      {
        Memo: memo(function Memo() {
          return <span>memo</span>
        }),
        Forward: forwardRef<HTMLSpanElement>(function Forward(_, ref) {
          return <span ref={ref}>forward</span>
        }),
        helper,
        Value: value
      },
      (name) => mounted.add(name)
    )
    const Memo = exports.Memo as ComponentType
    const Forward = exports.Forward as ComponentType<{ ref?: Ref<HTMLSpanElement> }>
    const ref = createRef<HTMLSpanElement>()
    render(
      <>
        <Memo />
        <Forward ref={ref} />
      </>
    )
    expect(componentExports(exports, {})).toEqual(['Forward', 'Memo'])
    expect(claimProblems({ name: 'Wrapped', components: ['Memo', 'Forward'] }, mounted)).toEqual([])
    expect(ref.current?.textContent).toBe('forward')
    expect(exports.helper).toBe(helper)
    expect(exports.Value).toBe(value)
  })
})
