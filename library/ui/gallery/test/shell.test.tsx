import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { SHOWCASE_INDEX } from '../../apps/hud/design/ShowcaseIndex'

vi.stubGlobal('__GENERATED_COMPONENTS__', [
  { name: 'Button', variants: 2 },
  { name: 'ShopModals', variants: 1 }
])
vi.stubGlobal('__GENERATED_REVIEW_URL__', '../dcl-ui-review/')
// jsdom lays nothing out and has no ResizeObserver: the observer runs its callback once, on observe.
vi.stubGlobal(
  'ResizeObserver',
  class {
    constructor(private readonly callback: () => void) {}
    observe() {
      this.callback()
    }
    disconnect() {}
  }
)
const { Shell } = await import('../src/shell/Shell')

const heading = () => screen.getByRole('heading', { level: 1 }).textContent
const link = (name: string) => screen.getAllByRole('link', { name: new RegExp(`^${name}`) })[0]!
const hudLinks = () => [
  ...screen.getByRole('navigation', { name: 'Gallery' }).querySelectorAll('a[data-source="hud"]')
]
const hudLink = (name: string) =>
  hudLinks().find((a) => new URL((a as HTMLAnchorElement).href).searchParams.get('hud') === name)!

describe('when a reviewer moves through the gallery sidebar', () => {
  beforeEach(() => window.history.replaceState(null, '', '/gallery/?component=Button'))
  afterEach(() => window.history.replaceState(null, '', '/'))

  it('should open the selection in place, keeping the sidebar and its scroll position', () => {
    render(<Shell search={window.location.search} />)
    const nav = screen.getByRole('navigation', { name: 'Gallery' })
    nav.scrollTop = 240
    const opened = fireEvent.click(link('Switch'))
    expect(opened).toBe(false)
    expect(heading()).toBe('Switch')
    expect(window.location.search).toBe('?component=Switch')
    expect(screen.getByRole('navigation', { name: 'Gallery' })).toBe(nav)
    expect(nav.scrollTop).toBe(240)
  })

  it('should restore the earlier selection on back', async () => {
    render(<Shell search={window.location.search} />)
    fireEvent.click(link('Switch'))
    expect(heading()).toBe('Switch')
    window.history.back()
    await waitFor(() => expect(heading()).toMatch(/^Button/))
  })

  it('should leave a modified click to the browser, so it can open a new tab', () => {
    render(<Shell search={window.location.search} />)
    expect(fireEvent.click(link('Switch'), { ctrlKey: true })).toBe(true)
    expect(fireEvent.click(link('Switch'), { metaKey: true })).toBe(true)
    expect(heading()).toMatch(/^Button/)
  })
})

describe('when the sidebar lists foundations, components, the HUD and the Figma reference', () => {
  it('should put foundations first, keep the Figma reference collapsed and label only shared names', () => {
    render(<Shell search="?component=Switch" />)
    const sections = screen.getAllByRole('heading', { level: 2 }).map((node) => node.textContent)
    expect(sections).toEqual(['Foundations', 'Components', 'HUD'])
    const reference = screen
      .getByText('Figma reference', { selector: 'summary' })
      .closest('details')!
    expect(reference.open).toBe(false)
    expect(screen.getByRole('link', { name: 'Colors' }).textContent).toBe('Colors')
    expect(link('Switch').textContent).toBe('Switch')
    // The runtime and Figma Buttons carry their source; the HUD's sits under its own heading.
    expect(screen.getAllByRole('link', { name: /^Button/ }).map((a) => a.textContent)).toEqual([
      'Buttonruntime',
      'Button',
      'ButtonFigma'
    ])
    expect(hudLink('Button').textContent).toBe('Button')
    expect(
      [...reference.querySelectorAll('a')].find((a) => a.textContent?.startsWith('ShopModals'))
        ?.textContent
    ).toBe('ShopModals')
  })

  it('should open the Figma reference with its explanation when it is the current route', () => {
    render(<Shell search="?source=generated&component=Button" />)
    expect(
      screen.getByText('Figma reference', { selector: 'summary' }).closest('details')!.open
    ).toBe(true)
    expect(heading()).toBe('Button Figma reference')
    expect(screen.getByText(/for comparison only: apps never use it/).textContent).toContain(
      'Apps use the Button component instead.'
    )
    expect(screen.getByTitle('Figma reference').getAttribute('src')).toBe(
      '../dcl-ui-review/?component=Button'
    )
  })
})

describe('when a reviewer opens the HUD Showcase', () => {
  beforeEach(() => window.history.replaceState(null, '', '/gallery/?component=Button'))
  afterEach(() => window.history.replaceState(null, '', '/'))

  it('should list the Showcase, every section in its order, then the glyphs', () => {
    render(<Shell search={window.location.search} />)
    expect(
      hudLinks().map((a) => new URL((a as HTMLAnchorElement).href).searchParams.get('hud'))
    ).toEqual(['Showcase', ...SHOWCASE_INDEX.map((section) => section.name), 'Glyphs'])
  })

  it('should open a section in place, in a window of the HUD document, labelled HUD', () => {
    render(<Shell search={window.location.search} />)
    expect(fireEvent.click(hudLink('Tabs'))).toBe(false)
    expect(window.location.search).toBe('?hud=Tabs')
    expect(heading()).toBe('TabsHUD')
    expect(screen.getByText('Exports shown:', { exact: false }).textContent).toBe(
      'Exports shown: Tabs'
    )
    const canvas = screen.getByTitle('Tabs canvas')
    expect(canvas.getAttribute('src')).toBe('hud.html?hud=Tabs')
    expect(canvas.className).toBe('canvas window')
    expect(canvas.style.height).toBe('')
  })

  it('should keep the width when moving from a runtime page to a section', () => {
    window.history.replaceState(null, '', '/gallery/?component=Button&width=1280')
    render(<Shell search={window.location.search} />)
    fireEvent.click(hudLink('Modal'))
    expect(window.location.search).toBe('?hud=Modal&width=1280')
    expect(screen.getByTitle('Modal canvas').style.width).toBe('1280px')
  })

  it('should open a component on the section that renders it, and name a missing section', () => {
    const { unmount } = render(<Shell search="?hud=MenuRow" />)
    expect(screen.getByText('MenuRow is shown on the MenuPanel page.')).toBeTruthy()
    expect(heading()).toBe('MenuPanelHUD')
    unmount()
    render(<Shell search="?hud=Nope" />)
    expect(screen.getByRole('alert').textContent).toBe(
      'No HUD section named “Nope”; showing Showcase.'
    )
    expect(screen.getByTitle('Showcase canvas').getAttribute('src')).toBe('hud.html?hud=Showcase')
  })
})

describe('when the chosen canvas width does not fit beside the sidebar', () => {
  // The space jsdom does not lay out: the viewport's width and the canvas's own.
  const lay = (room: number) => {
    vi.spyOn(HTMLElement.prototype, 'clientWidth', 'get').mockImplementation(function (
      this: HTMLElement
    ) {
      return this.dataset.width ? room : 0
    })
    vi.spyOn(HTMLElement.prototype, 'offsetWidth', 'get').mockImplementation(function (
      this: HTMLElement
    ) {
      return this.tagName === 'IFRAME' ? Number.parseFloat(this.style.width) + 2 : 0
    })
    vi.spyOn(HTMLElement.prototype, 'offsetHeight', 'get').mockReturnValue(600)
  }
  afterEach(() => vi.restoreAllMocks())

  it('should draw it scaled down to the space, keeping its width, with no horizontal scroll', () => {
    lay(1000)
    render(<Shell search="?component=BlogCard&width=1280" />)
    const canvas = screen.getByTitle('BlogCard canvas')
    const scale = Number(/^scale\((.+)\)$/.exec(canvas.style.transform)?.[1])
    // 1000px less the viewport's padding, for a 1282px canvas (1280px and its border).
    expect(scale).toBeGreaterThan(960 / 1282)
    expect(scale).toBeLessThanOrEqual(1000 / 1282)
    expect(canvas.style.width).toBe('1280px')
    expect(parseFloat(canvas.parentElement!.style.width)).toBeCloseTo(1282 * scale)
    expect(parseFloat(canvas.parentElement!.style.height)).toBeCloseTo(600 * scale)
    expect(screen.getByText(/to fit this window; the canvas is 1280px wide/)).toBeTruthy()
  })

  it('should draw a canvas that fits at its own size', () => {
    lay(1400)
    render(<Shell search="?component=BlogCard&width=1280" />)
    const canvas = screen.getByTitle('BlogCard canvas')
    expect(canvas.style.transform).toBe('')
    expect(canvas.parentElement!.getAttribute('style')).toBeNull()
    expect(screen.queryByText(/to fit this window/)).toBeNull()
  })

  it('should never scale a canvas that fills the space', () => {
    lay(300)
    render(<Shell search="?component=BlogCard" />)
    expect(screen.getByTitle('BlogCard canvas').style.transform).toBe('')
  })
})
