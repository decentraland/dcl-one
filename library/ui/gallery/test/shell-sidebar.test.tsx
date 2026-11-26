import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { HEIGHT_MESSAGE, OPEN_MESSAGE, WIDTH_MESSAGE } from '../src/messages'
import { COLLAPSED_GROUPS_KEY } from '../src/shell/collapsedGroups'

vi.stubGlobal('__GENERATED_COMPONENTS__', [{ name: 'Button', variants: 2 }])
vi.stubGlobal('__GENERATED_REVIEW_URL__', '../dcl-ui-review/')
vi.stubGlobal(
  'ResizeObserver',
  class {
    observe() {}
    disconnect() {}
  }
)
const { Shell } = await import('../src/shell/Shell')

const GROUPS = [
  'Foundations',
  'Actions',
  'Forms',
  'Navigation',
  'Cards',
  'Feedback',
  'Backgrounds',
  'HUD'
]
const toggle = (name: string) => screen.getByRole('button', { name })
const expanded = (name: string) => toggle(name).getAttribute('aria-expanded') === 'true'
const collapsedGroups = () => GROUPS.filter((name) => !expanded(name))
const stored = () => JSON.parse(window.localStorage.getItem(COLLAPSED_GROUPS_KEY) ?? 'null')
const heading = () => screen.getByRole('heading', { level: 1 }).textContent

beforeEach(() => {
  window.localStorage.clear()
  window.history.replaceState(null, '', '/gallery/?component=Button')
})
afterEach(() => {
  vi.restoreAllMocks()
  window.history.replaceState(null, '', '/')
})

describe('when a reviewer collapses a sidebar group', () => {
  it('should make every group heading a button that names the list it controls, all expanded', () => {
    render(<Shell search="?component=Button" />)
    expect(
      screen
        .getAllByRole('button')
        .filter((button) => button.hasAttribute('aria-expanded'))
        .map((button) => button.textContent)
    ).toEqual(GROUPS)
    for (const name of GROUPS) {
      const button = toggle(name)
      expect(button.closest('h2, h3')).not.toBeNull()
      expect(button.getAttribute('type')).toBe('button')
      const region = document.getElementById(button.getAttribute('aria-controls')!)!
      expect(region.hidden).toBe(false)
      expect(region.querySelector('a')).not.toBeNull()
    }
    expect(new Set(GROUPS.map((name) => toggle(name).getAttribute('aria-controls'))).size).toBe(
      GROUPS.length
    )
  })

  it('should hide only that list, keep its links out of reach, and show it again', () => {
    render(<Shell search="?component=Button" />)
    fireEvent.click(toggle('Forms'))
    expect(collapsedGroups()).toEqual(['Forms'])
    expect(document.getElementById(toggle('Forms').getAttribute('aria-controls')!)!.hidden).toBe(
      true
    )
    expect(screen.queryByRole('link', { name: 'Switch' })).toBeNull()
    expect(screen.getByRole('link', { name: 'TabsBar' })).toBeTruthy()
    fireEvent.click(toggle('Forms'))
    expect(collapsedGroups()).toEqual([])
    expect(screen.getByRole('link', { name: 'Switch' })).toBeTruthy()
  })

  it('should let the reviewer collapse the group of the page that is open', () => {
    render(<Shell search="?component=Button" />)
    fireEvent.click(toggle('Actions'))
    expect(collapsedGroups()).toEqual(['Actions'])
    expect(heading()).toMatch(/^Button/)
  })

  it('should remember the collapsed groups on the next visit', () => {
    const { unmount } = render(<Shell search="?component=Button" />)
    fireEvent.click(toggle('HUD'))
    fireEvent.click(toggle('Cards'))
    expect(stored()).toEqual(['hud', 'component:Cards'])
    unmount()
    render(<Shell search="?component=Button" />)
    expect(collapsedGroups()).toEqual(['Cards', 'HUD'])
  })
})

describe('when a reviewer collapses or expands every group at once', () => {
  it('should collapse them all, then offer to expand them all', () => {
    render(<Shell search="?source=generated&component=Button" />)
    fireEvent.click(screen.getByRole('button', { name: 'Collapse all' }))
    expect(collapsedGroups()).toEqual(GROUPS)
    expect(screen.queryByRole('button', { name: 'Collapse all' })).toBeNull()
    fireEvent.click(screen.getByRole('button', { name: 'Expand all' }))
    expect(collapsedGroups()).toEqual([])
    expect(stored()).toEqual([])
  })

  it('should collapse the rest when one group is already collapsed', () => {
    render(<Shell search="?component=Button" />)
    fireEvent.click(toggle('Feedback'))
    fireEvent.click(screen.getByRole('button', { name: 'Collapse all' }))
    expect(collapsedGroups()).toEqual(GROUPS)
    expect(screen.getByRole('button', { name: 'Expand all' })).toBeTruthy()
  })

  it('should offer to collapse again as soon as one group is expanded', () => {
    render(<Shell search="?component=Button" />)
    fireEvent.click(screen.getByRole('button', { name: 'Collapse all' }))
    fireEvent.click(toggle('Navigation'))
    expect(screen.getByRole('button', { name: 'Collapse all' })).toBeTruthy()
  })
})

describe('when a link opens a page inside a collapsed group', () => {
  it('should expand the group of a deep link, and leave the other stored groups collapsed', () => {
    window.localStorage.setItem(
      COLLAPSED_GROUPS_KEY,
      JSON.stringify(['component:Forms', 'hud', 'foundations'])
    )
    render(<Shell search="?component=Switch" />)
    expect(collapsedGroups()).toEqual(['Foundations', 'HUD'])
    expect(screen.getByRole('link', { name: 'Switch' }).getAttribute('aria-current')).toBe('page')
    expect(stored()).toEqual(['foundations', 'hud'])
  })

  it.each([
    ['?foundations=icons', 'Foundations'],
    ['?hud=Tabs', 'HUD'],
    ['?component=EventCardHost', 'Cards']
  ])('should expand the group that lists %s', (search, group) => {
    window.localStorage.setItem(
      COLLAPSED_GROUPS_KEY,
      JSON.stringify(['foundations', 'hud', 'component:Cards'])
    )
    render(<Shell search={search} />)
    expect(expanded(group)).toBe(true)
    expect(collapsedGroups()).toHaveLength(2)
  })

  it('should expand the group when back returns to a page the reviewer had hidden', async () => {
    render(<Shell search={window.location.search} />)
    fireEvent.click(screen.getByRole('link', { name: 'Switch' }))
    fireEvent.click(screen.getByRole('button', { name: 'Collapse all' }))
    expect(collapsedGroups()).toEqual(GROUPS)
    window.history.back()
    await waitFor(() => expect(heading()).toMatch(/^Button/))
    expect(collapsedGroups()).toEqual(GROUPS.filter((name) => name !== 'Actions'))
  })

  it('should keep every group collapsed for a page no group lists', () => {
    const all = ['foundations', 'component:Actions', 'hud']
    window.localStorage.setItem(COLLAPSED_GROUPS_KEY, JSON.stringify(all))
    const { unmount } = render(<Shell search="?component=Showcase" />)
    expect(collapsedGroups()).toEqual(['Foundations', 'Actions', 'HUD'])
    unmount()
    render(<Shell search="?source=generated&component=Button" />)
    expect(collapsedGroups()).toEqual(['Foundations', 'Actions', 'HUD'])
  })
})

describe('when the stored sidebar state cannot be used', () => {
  it('should ignore a stored group that does not exist', () => {
    window.localStorage.setItem(COLLAPSED_GROUPS_KEY, JSON.stringify(['component:Gone', 'hud']))
    render(<Shell search="?component=Button" />)
    expect(collapsedGroups()).toEqual(['HUD'])
    expect(stored()).toEqual(['hud'])
  })

  it('should work for this visit when storage throws on every read and write', () => {
    vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new DOMException('blocked', 'SecurityError')
    })
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new DOMException('blocked', 'SecurityError')
    })
    render(<Shell search="?component=Button" />)
    expect(collapsedGroups()).toEqual([])
    fireEvent.click(toggle('Forms'))
    expect(collapsedGroups()).toEqual(['Forms'])
    fireEvent.click(screen.getByRole('button', { name: 'Collapse all' }))
    expect(collapsedGroups()).toEqual(GROUPS)
  })

  it('should start expanded when storage holds something else', () => {
    window.localStorage.setItem(COLLAPSED_GROUPS_KEY, '{"hud":true')
    render(<Shell search="?component=Button" />)
    expect(collapsedGroups()).toEqual([])
  })
})

describe('when a reviewer filters the sidebar', () => {
  it('should show a match inside a collapsed group, and restore the group once the filter is cleared', () => {
    render(<Shell search="?component=Button" />)
    fireEvent.click(toggle('Forms'))
    fireEvent.change(screen.getByRole('searchbox', { name: 'Filter' }), {
      target: { value: 'switch' }
    })
    expect(screen.getByRole('link', { name: 'Switch' })).toBeTruthy()
    expect(expanded('Forms')).toBe(true)
    fireEvent.change(screen.getByRole('searchbox', { name: 'Filter' }), { target: { value: '' } })
    expect(collapsedGroups()).toEqual(['Forms'])
  })

  it('should keep the collapse buttons in the tab order, say why they do nothing, and leave the groups as they were', () => {
    render(<Shell search="?component=Button" />)
    fireEvent.click(toggle('Forms'))
    const all = screen.getByRole('button', { name: 'Collapse all' }) as HTMLButtonElement
    expect(toggle('Forms').hasAttribute('aria-disabled')).toBe(false)
    expect(toggle('Forms').hasAttribute('aria-describedby')).toBe(false)
    expect(all.hasAttribute('aria-disabled')).toBe(false)
    fireEvent.change(screen.getByRole('searchbox', { name: 'Filter' }), {
      target: { value: 'switch' }
    })
    for (const button of [toggle('Forms') as HTMLButtonElement, all]) {
      expect(button.disabled).toBe(false)
      expect(button.getAttribute('aria-disabled')).toBe('true')
      expect(document.getElementById(button.getAttribute('aria-describedby')!)!.textContent).toBe(
        'Groups stay open while filtering.'
      )
      button.focus()
      expect(document.activeElement).toBe(button)
      fireEvent.click(button)
    }
    expect(expanded('Forms')).toBe(true)
    expect(all.textContent).toBe('Collapse all')
    expect(stored()).toEqual(['component:Forms'])
    fireEvent.change(screen.getByRole('searchbox', { name: 'Filter' }), { target: { value: '' } })
    expect(collapsedGroups()).toEqual(['Forms'])
    expect(toggle('Forms').hasAttribute('aria-disabled')).toBe(false)
    fireEvent.click(toggle('Forms'))
    expect(collapsedGroups()).toEqual([])
  })
})

describe('when a reviewer opens the components Showcase', () => {
  const post = (source: Window | null, data: unknown) =>
    act(() => {
      window.dispatchEvent(new MessageEvent('message', { source, data }))
    })

  it('should list it first under Components and open it in the components canvas', () => {
    render(<Shell search={window.location.search} />)
    const links = [
      ...screen
        .getByRole('navigation', { name: 'Gallery' })
        .querySelectorAll('a[data-source="runtime"]')
    ].map((a) => new URL((a as HTMLAnchorElement).href).searchParams.get('component'))
    expect(links.filter(Boolean)[0]).toBe('Showcase')
    fireEvent.click(screen.getAllByRole('link', { name: 'Showcase' })[0]!)
    expect(window.location.search).toBe('?component=Showcase')
    expect(heading()).toBe('Showcase')
    expect(screen.getByText(/^Every component page in one canvas/)).toBeTruthy()
    expect(screen.queryByText('Exports shown:', { exact: false })).toBeNull()
    expect(screen.getByTitle('Showcase canvas').getAttribute('src')).toBe(
      'frame.html?component=Showcase'
    )
  })

  it('should keep the first component page as the page a bare or unknown link opens', () => {
    const { unmount } = render(<Shell search="" />)
    expect(heading()).toMatch(/^Button/)
    unmount()
    render(<Shell search="?component=Nope" />)
    expect(screen.getByRole('alert').textContent).toBe(
      'No runtime page named “Nope”; showing Button.'
    )
  })

  it('should open the page a Showcase link names, in place and at the chosen width', () => {
    window.history.replaceState(null, '', '/gallery/?component=Showcase&width=768')
    render(<Shell search={window.location.search} />)
    const canvas = screen.getByTitle('Showcase canvas') as HTMLIFrameElement
    post(canvas.contentWindow, { type: OPEN_MESSAGE, component: 'Tabs' })
    expect(window.location.search).toBe('?component=Tabs&width=768')
    expect(heading()).toBe('Tabs')
    expect(expanded('Navigation')).toBe(true)
  })

  it('should ignore a request for an unknown page, a malformed one, or one from another window', () => {
    window.history.replaceState(null, '', '/gallery/?component=Showcase')
    render(<Shell search={window.location.search} />)
    const canvas = screen.getByTitle('Showcase canvas') as HTMLIFrameElement
    post(canvas.contentWindow, { type: OPEN_MESSAGE, component: 'Nope' })
    post(canvas.contentWindow, { type: OPEN_MESSAGE, component: 7 })
    post(canvas.contentWindow, { type: OPEN_MESSAGE })
    post(canvas.contentWindow, null)
    post(window, { type: OPEN_MESSAGE, component: 'Tabs' })
    post(null, { type: OPEN_MESSAGE, component: 'Tabs' })
    expect(window.location.search).toBe('?component=Showcase')
    expect(heading()).toBe('Showcase')
  })
})

describe('when the canvas of a runtime page reports to the gallery around it', () => {
  const post = (source: Window | null, data: unknown) =>
    act(() => {
      window.dispatchEvent(new MessageEvent('message', { source, data }))
    })

  it('should start the canvas at 480px and follow the height it reports, never under 240px', () => {
    render(<Shell search="?component=Button" />)
    const canvas = screen.getByTitle('Button canvas') as HTMLIFrameElement
    expect(canvas.style.height).toBe('480px')
    post(canvas.contentWindow, { type: HEIGHT_MESSAGE, height: 900.2 })
    expect(canvas.style.height).toBe('901px')
    post(canvas.contentWindow, { type: HEIGHT_MESSAGE, height: 100 })
    expect(canvas.style.height).toBe('240px')
  })

  it('should ignore a height from another window, of another kind, or that is not a number', () => {
    render(<Shell search="?component=Button" />)
    const canvas = screen.getByTitle('Button canvas') as HTMLIFrameElement
    post(window, { type: HEIGHT_MESSAGE, height: 2000 })
    post(null, { type: HEIGHT_MESSAGE, height: 2000 })
    post(canvas.contentWindow, { type: OPEN_MESSAGE, height: 2000 })
    post(canvas.contentWindow, { type: HEIGHT_MESSAGE, height: '2000' })
    post(canvas.contentWindow, null)
    expect(canvas.style.height).toBe('480px')
  })

  it('should tell the canvas when the reviewer picks another width, and not when another page opens at the same one', () => {
    window.history.replaceState(null, '', '/gallery/?component=Showcase&width=768')
    render(<Shell search={window.location.search} />)
    const canvas = screen.getByTitle('Showcase canvas') as HTMLIFrameElement
    const told = vi.spyOn(canvas.contentWindow!, 'postMessage')
    const input = screen.getByRole('spinbutton', { name: 'Custom px' })
    fireEvent.change(input, { target: { value: '1024' } })
    fireEvent.submit(input.closest('form')!)
    expect(window.location.search).toBe('?component=Showcase&width=1024')
    expect(screen.getByTitle('Showcase canvas')).toBe(canvas)
    expect(told.mock.calls).toEqual([[{ type: WIDTH_MESSAGE }, window.location.origin]])
    fireEvent.submit(input.closest('form')!)
    expect(told).toHaveBeenCalledTimes(1)
  })
})
