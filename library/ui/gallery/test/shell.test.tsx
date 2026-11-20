import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

vi.stubGlobal('__GENERATED_COMPONENTS__', [
  { name: 'Button', variants: 2 },
  { name: 'ShopModals', variants: 1 }
])
vi.stubGlobal('__GENERATED_REVIEW_URL__', '../dcl-ui-review/')
const { Shell } = await import('../src/shell/Shell')

const heading = () => screen.getByRole('heading', { level: 1 }).textContent
const link = (name: string) => screen.getAllByRole('link', { name: new RegExp(`^${name}`) })[0]!

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

describe('when the sidebar lists foundations, components and the Figma reference', () => {
  it('should put foundations first, keep the Figma reference collapsed and label only shared names', () => {
    render(<Shell search="?component=Switch" />)
    const sections = screen.getAllByRole('heading', { level: 2 }).map((node) => node.textContent)
    expect(sections.slice(0, 2)).toEqual(['Foundations', 'Components'])
    const reference = screen
      .getByText('Figma reference', { selector: 'summary' })
      .closest('details')!
    expect(reference.open).toBe(false)
    expect(screen.getByRole('link', { name: 'Colors' }).textContent).toBe('Colors')
    expect(link('Switch').textContent).toBe('Switch')
    expect(screen.getAllByRole('link', { name: /^Button/ }).map((a) => a.textContent)).toEqual([
      'Buttonruntime',
      'ButtonFigma'
    ])
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
