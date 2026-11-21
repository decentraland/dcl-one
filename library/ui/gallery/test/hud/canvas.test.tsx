import { useState, type ReactNode } from 'react'
import { render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { Showcase, SHOWCASE_SECTIONS } from '../../../apps/hud/design/Showcase'
import { HudCanvas, hudCanvasPages } from '../../src/hud/HudCanvas'
import { hudPages } from '../../src/hudPages'
import { stubElementScroll } from './jsdom'

stubElementScroll()

// Mounted as an element, as the Showcase and the gallery mount a section, so its hooks belong to it.
function mount(page: { render: () => ReactNode }) {
  const Render = page.render
  return render(<Render />)
}

const drawn = (container: HTMLElement) =>
  [...container.querySelectorAll<HTMLElement>('[data-example]')].map((node) => node.dataset.example)

describe('when the HUD pages render with local data', () => {
  // Restored after each case, so a failed assertion cannot leave the spy on later ones.
  afterEach(() => {
    vi.unstubAllGlobals()
    vi.restoreAllMocks()
  })

  it.each(hudCanvasPages.map((page) => [page.name, page] as const))(
    'should render %s without a fetch, a remote picture or a console error',
    (_, page) => {
      const fetch = vi.fn()
      vi.stubGlobal('fetch', fetch)
      const errors = vi.spyOn(console, 'error').mockImplementation(() => {})
      const { container } = mount(page)
      const pictures = [...container.querySelectorAll('img, image')].map(
        (node) => node.getAttribute('src') ?? node.getAttribute('href') ?? ''
      )
      expect(container.childElementCount).toBeGreaterThan(0)
      expect(pictures.filter((src) => /^https?:/.test(src))).toEqual([])
      expect(fetch).not.toHaveBeenCalled()
      expect(errors).not.toHaveBeenCalled()
    }
  )

  it('should give every page a unique name, the gallery and the canvas the same ones', () => {
    expect(new Set(hudPages.pages).size).toBe(hudPages.pages.length)
    expect(hudCanvasPages.map((page) => page.name)).toEqual(hudPages.pages.slice(1))
  })

  it('should mount a page whose render calls a hook, as the hosts do', () => {
    const { container } = mount({
      render: () => {
        const [label] = useState('Hooked')
        return <p>{label}</p>
      }
    })
    expect(container.textContent).toBe('Hooked')
  })

  it('should give every labelled picker in the Showcase its own label', () => {
    const { container } = render(<Showcase />)
    // The pickers a label names; one without a label shows its value, as two Dropdowns of one example may.
    const names = [...container.querySelectorAll('[aria-haspopup="listbox"][aria-label]')].map(
      (node) => node.getAttribute('aria-label')
    )
    expect(names).toContain('Resolution')
    expect(names.filter((name, i) => names.indexOf(name) !== i)).toEqual([])
  })

  it('should keep every section in the Showcase the HUD hosts open with ?showcase=1', () => {
    render(<Showcase />)
    for (const section of SHOWCASE_SECTIONS)
      expect(screen.getAllByText(section.title, { selector: 'section > p' })).toHaveLength(1)
  })
})

describe('when the HUD canvas opens a page', () => {
  it('should draw one section on its own page', () => {
    const { container } = render(<HudCanvas search="?hud=Tabs" />)
    expect(container.querySelector('main')?.dataset.hud).toBe('Tabs')
    expect(drawn(container)).toEqual(['Tabs'])
  })

  it('should draw every section, in order, then the glyphs on the Showcase page', () => {
    const { container } = render(<HudCanvas search="?hud=Showcase" />)
    expect(drawn(container)).toEqual([
      ...SHOWCASE_SECTIONS.map((section) => section.name),
      'Glyphs'
    ])
  })

  it.each([
    ['?hud=Nope', 'No HUD section matches ?hud=Nope.'],
    ['', 'No HUD section matches an empty query.']
  ])('should answer %j with an alert and draw nothing', (search, message) => {
    const { container } = render(<HudCanvas search={search} />)
    expect(screen.getByRole('alert').textContent).toBe(message)
    expect(drawn(container)).toEqual([])
  })
})
