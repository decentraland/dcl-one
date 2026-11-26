import { act, fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import * as library from '@dcl/one-ui'
import {
  COMPONENT_OVERVIEW,
  componentGroups,
  componentPages,
  groupEntries
} from '../src/componentPages'
import { entries } from '../src/examples'
import { Canvas } from '../src/frame/Canvas'
import { ComponentShowcase } from '../src/frame/ComponentShowcase'
import { keepLinksInCanvas } from '../src/frame/links'
import { HEIGHT_MESSAGE, OPEN_MESSAGE, WIDTH_MESSAGE } from '../src/messages'
import type { GalleryEntry } from '../src/registry'

vi.stubGlobal(
  'ResizeObserver',
  class {
    observe() {}
    disconnect() {}
  }
)

afterEach(() => vi.restoreAllMocks())

const page = (name: string, group: string, extra: Partial<GalleryEntry> = {}): GalleryEntry => ({
  name,
  group,
  summary: `${name} summary`,
  components: [name],
  examples: [
    { id: 'one', title: `${name} one`, render: () => <p>{name} first</p> },
    { id: 'two', title: `${name} two`, render: () => <p>{name} second</p> }
  ],
  ...extra
})
const sections = (container: HTMLElement, attribute: string) =>
  [...container.querySelectorAll(`[${attribute}]`)].map((node) => node.getAttribute(attribute))

describe('when the component pages are grouped for the sidebar and the Showcase', () => {
  it('should keep each group at its first page and list the pages of each by name', () => {
    const groups = groupEntries([page('C', 'One'), page('B', 'Two'), page('A', 'One')])
    expect(groups.map((group) => [group.name, group.entries.map((entry) => entry.name)])).toEqual([
      ['One', ['A', 'C']],
      ['Two', ['B']]
    ])
    expect(groupEntries([])).toEqual([])
  })

  it('should hold every registry page exactly once', () => {
    expect(componentGroups.flatMap((group) => group.entries)).toHaveLength(entries.length)
    expect(new Set(componentGroups.flatMap((group) => group.entries)).size).toBe(entries.length)
  })

  it('should keep the Showcase name free: no page, shown export or public export takes it', () => {
    expect(entries.map((entry) => entry.name)).not.toContain(COMPONENT_OVERVIEW)
    expect(entries.flatMap((entry) => entry.components)).not.toContain(COMPONENT_OVERVIEW)
    expect(Object.keys(library)).not.toContain(COMPONENT_OVERVIEW)
    expect(componentPages.at(-1)).toBe(COMPONENT_OVERVIEW)
    expect(componentPages[0]).toBe(componentGroups[0]!.entries[0]!.name)
  })
})

describe('when the Showcase canvas draws the component pages', () => {
  it('should draw every page of the registry once, under its group, in the sidebar order', () => {
    const { container } = render(<Canvas search="?component=Showcase" />)
    expect(container.querySelector('main')!.dataset.component).toBe('Showcase')
    expect(sections(container, 'data-showcase-group')).toEqual(
      componentGroups.map((group) => group.name)
    )
    expect(sections(container, 'data-showcase-page')).toEqual(componentPages.slice(0, -1))
    expect(sections(container, 'data-showcase-page')).toHaveLength(entries.length)
    expect(
      [...container.querySelectorAll('[data-showcase-group] > h2')].map((node) => node.textContent)
    ).toEqual(componentGroups.map((group) => group.name))
    expect(container.querySelector('[data-example-error]')).toBeNull()
    expect(container.querySelector('[data-showcase-empty]')).toBeNull()
  })

  it('should draw the same examples a page draws on its own canvas, not copies of them', () => {
    const groups = groupEntries([page('Alpha', 'One'), page('Beta', 'Two')])
    const { container } = render(<ComponentShowcase groups={groups} />)
    const alpha = container.querySelector<HTMLElement>('[data-showcase-page="Alpha"]')!
    expect(sections(alpha, 'data-example')).toEqual(['one', 'two'])
    expect(within(alpha).getByText('Alpha first')).toBeTruthy()
    expect(within(alpha).getByText('Alpha summary')).toBeTruthy()
    // The page name is the heading above its examples, whose titles sit one level under it.
    expect(within(alpha).getByRole('heading', { level: 3 }).textContent).toBe('Alpha')
    expect(
      within(alpha)
        .getAllByRole('heading', { level: 4 })
        .map((heading) => heading.textContent)
    ).toEqual(['Alpha one', 'Alpha two'])
    expect(
      within(container.querySelector<HTMLElement>('[data-showcase-group="Two"]')!).getByText(
        'Beta first'
      )
    ).toBeTruthy()
  })

  it('should keep an example that fails to itself, and draw the pages after it', () => {
    vi.spyOn(console, 'error').mockImplementation(() => {})
    const broken = page('Broken', 'One', {
      examples: [
        {
          id: 'throws',
          title: 'Throws',
          render: (): never => {
            throw new Error('The example failed')
          }
        }
      ]
    })
    render(<ComponentShowcase groups={groupEntries([broken, page('After', 'One')])} />)
    expect(screen.getByRole('alert').textContent).toBe('The example failed')
    expect(screen.getByText('After first')).toBeTruthy()
  })

  it('should say so for a page that has no examples, and still link to it', () => {
    const empty = page('Empty', 'One', { examples: [] })
    const { container } = render(
      <ComponentShowcase groups={groupEntries([empty, page('After', 'One')])} />
    )
    const section = container.querySelector<HTMLElement>('[data-showcase-page="Empty"]')!
    expect(section.querySelector('[data-showcase-empty]')!.textContent).toBe(
      'This page has no examples yet.'
    )
    expect(section.querySelector('[data-example], iframe')).toBeNull()
    expect(within(section).getByRole('link', { name: 'Empty' })).toBeTruthy()
    expect(screen.getByText('After first')).toBeTruthy()
  })

  it('should draw nothing but the canvas for a registry with no pages', () => {
    const { container } = render(<ComponentShowcase groups={[]} />)
    expect(container.querySelector('main')!.children).toHaveLength(0)
  })

  it('should give a page positioned against the viewport a canvas of its own, at least as tall as it asks', () => {
    const { container } = render(
      <ComponentShowcase groups={groupEntries([page('Dialog', 'One', { minHeight: 720 })])} />
    )
    const frame = screen.getByTitle('Dialog canvas') as HTMLIFrameElement
    expect(frame.getAttribute('src')).toBe('frame.html?component=Dialog&embedded=1')
    expect(frame.style.height).toBe('720px')
    expect(container.querySelector('[data-example]')).toBeNull()
  })

  it('should follow the height its own canvas reports, never under what the page asks', () => {
    render(<ComponentShowcase groups={groupEntries([page('Dialog', 'One', { minHeight: 720 })])} />)
    const frame = screen.getByTitle('Dialog canvas') as HTMLIFrameElement
    const report = (source: Window | null, data: unknown) =>
      act(() => {
        window.dispatchEvent(new MessageEvent('message', { source, data }))
      })
    report(frame.contentWindow, { type: HEIGHT_MESSAGE, height: 900.2 })
    expect(frame.style.height).toBe('901px')
    report(frame.contentWindow, { type: HEIGHT_MESSAGE, height: 300 })
    expect(frame.style.height).toBe('720px')
  })

  it('should ignore a height from another window, of another kind, or that is not a number', () => {
    render(
      <ComponentShowcase
        groups={groupEntries([
          page('Dialog', 'One', { minHeight: 720 }),
          page('Drawer', 'One', { minHeight: 480 })
        ])}
      />
    )
    const dialog = screen.getByTitle('Dialog canvas') as HTMLIFrameElement
    const drawer = screen.getByTitle('Drawer canvas') as HTMLIFrameElement
    const report = (source: Window | null, data: unknown) =>
      act(() => {
        window.dispatchEvent(new MessageEvent('message', { source, data }))
      })
    report(window, { type: HEIGHT_MESSAGE, height: 2000 })
    report(null, { type: HEIGHT_MESSAGE, height: 2000 })
    report(dialog.contentWindow, { type: OPEN_MESSAGE, height: 2000 })
    report(dialog.contentWindow, { type: HEIGHT_MESSAGE, height: '2000' })
    report(dialog.contentWindow, null)
    expect(dialog.style.height).toBe('720px')
    // Each canvas listens to its own frame only.
    report(drawer.contentWindow, { type: HEIGHT_MESSAGE, height: 1000 })
    expect(dialog.style.height).toBe('720px')
    expect(drawer.style.height).toBe('1000px')
  })

  it('should use an own canvas for exactly the registry pages that ask for a taller one', () => {
    const { container } = render(<ComponentShowcase />)
    expect(
      [...container.querySelectorAll('iframe')].map((frame) => frame.title.replace(' canvas', ''))
    ).toEqual(
      componentGroups
        .flatMap((group) => group.entries)
        .filter((entry) => entry.minHeight)
        .map((entry) => entry.name)
    )
  })
})

describe('when a reviewer follows a page link on the Showcase canvas', () => {
  const groups = groupEntries([page('Alpha', 'One')])
  const embed = () => {
    const parent = { postMessage: vi.fn() }
    vi.spyOn(window, 'parent', 'get').mockReturnValue(parent as unknown as Window)
    return parent
  }

  it('should link each page to its own gallery page, leaving the canvas', () => {
    render(<ComponentShowcase groups={groups} />)
    const link = screen.getByRole('link', { name: 'Alpha' })
    expect(link.getAttribute('href')).toBe('./?component=Alpha')
    expect(link.getAttribute('target')).toBe('_top')
  })

  it('should carry the width chosen in the gallery around it, and follow it when the canvas is resized', () => {
    const location = { search: '?component=Showcase&width=768' }
    vi.spyOn(window, 'parent', 'get').mockReturnValue({ location } as unknown as Window)
    render(<ComponentShowcase groups={groups} />)
    const link = screen.getByRole('link', { name: 'Alpha' })
    expect(link.getAttribute('href')).toBe('./?component=Alpha&width=768')
    location.search = '?component=Showcase&width=1024'
    act(() => {
      window.dispatchEvent(new Event('resize'))
    })
    expect(link.getAttribute('href')).toBe('./?component=Alpha&width=1024')
    location.search = '?component=Showcase'
    act(() => {
      window.dispatchEvent(new Event('resize'))
    })
    expect(link.getAttribute('href')).toBe('./?component=Alpha')
  })

  describe('and the gallery says the width changed while the canvas keeps its size', () => {
    // A message as the gallery around the canvas posts it: jsdom takes no stand-in window as an event's source.
    const message = (source: unknown, data: unknown) =>
      act(() => {
        const event = new MessageEvent('message', { data })
        Object.defineProperty(event, 'source', { value: source })
        window.dispatchEvent(event)
      })
    const embedAt = (search: string) => {
      const parent = { location: { search } }
      vi.spyOn(window, 'parent', 'get').mockReturnValue(parent as unknown as Window)
      return parent
    }

    it('should carry the new width with no resize, and drop it for the fill width', () => {
      const parent = embedAt('?component=Showcase')
      render(<ComponentShowcase groups={groups} />)
      const link = screen.getByRole('link', { name: 'Alpha' })
      expect(link.getAttribute('href')).toBe('./?component=Alpha')
      parent.location.search = '?component=Showcase&width=1024'
      message(parent, { type: WIDTH_MESSAGE })
      expect(link.getAttribute('href')).toBe('./?component=Alpha&width=1024')
      parent.location.search = '?component=Showcase'
      message(parent, { type: WIDTH_MESSAGE })
      expect(link.getAttribute('href')).toBe('./?component=Alpha')
    })

    it('should ignore a message from another window, of another kind, or with no data', () => {
      const parent = embedAt('?component=Showcase&width=768')
      render(<ComponentShowcase groups={groups} />)
      const link = screen.getByRole('link', { name: 'Alpha' })
      parent.location.search = '?component=Showcase&width=1024'
      message(window, { type: WIDTH_MESSAGE })
      message(null, { type: WIDTH_MESSAGE })
      message(parent, { type: OPEN_MESSAGE })
      message(parent, null)
      expect(link.getAttribute('href')).toBe('./?component=Alpha&width=768')
    })

    it('should stop listening once the canvas is gone', () => {
      const parent = embedAt('?component=Showcase&width=768')
      const { unmount } = render(<ComponentShowcase groups={groups} />)
      const read = vi.spyOn(parent, 'location', 'get')
      unmount()
      message(parent, { type: WIDTH_MESSAGE })
      expect(read).not.toHaveBeenCalled()
    })
  })

  it.each(['?component=Showcase&width=wide', '?component=Showcase&width=12', ''])(
    'should link without a width when the gallery around it has %j',
    (search) => {
      vi.spyOn(window, 'parent', 'get').mockReturnValue({
        location: { search }
      } as unknown as Window)
      render(<ComponentShowcase groups={groups} />)
      expect(screen.getByRole('link', { name: 'Alpha' }).getAttribute('href')).toBe(
        './?component=Alpha'
      )
    }
  )

  it('should link without a width when the page around it keeps its URL out of reach', () => {
    vi.spyOn(window, 'parent', 'get').mockReturnValue({
      get location(): never {
        throw new DOMException('cross-origin', 'SecurityError')
      }
    } as unknown as Window)
    render(<ComponentShowcase groups={groups} />)
    expect(screen.getByRole('link', { name: 'Alpha' }).getAttribute('href')).toBe(
      './?component=Alpha'
    )
  })

  it('should ask the gallery to open the page in place when the canvas is inside it', () => {
    const parent = embed()
    render(<ComponentShowcase groups={groups} />)
    expect(fireEvent.click(screen.getByRole('link', { name: 'Alpha' }))).toBe(false)
    expect(parent.postMessage).toHaveBeenCalledWith(
      { type: OPEN_MESSAGE, component: 'Alpha' },
      window.location.origin
    )
  })

  it('should leave a modified click to the browser, so it can open a new tab', () => {
    const parent = embed()
    render(<ComponentShowcase groups={groups} />)
    const link = screen.getByRole('link', { name: 'Alpha' })
    for (const modifier of ['ctrlKey', 'metaKey', 'shiftKey', 'altKey'])
      expect(fireEvent.click(link, { [modifier]: true })).toBe(true)
    expect(fireEvent.click(link, { button: 1 })).toBe(true)
    expect(parent.postMessage).not.toHaveBeenCalled()
  })

  it('should follow the link itself when the canvas is open on its own', () => {
    const post = vi.spyOn(window, 'postMessage')
    render(<ComponentShowcase groups={groups} />)
    expect(fireEvent.click(screen.getByRole('link', { name: 'Alpha' }))).toBe(true)
    expect(post).not.toHaveBeenCalled()
  })

  it('should keep example links in the canvas while the page links still leave it', () => {
    keepLinksInCanvas()
    const withLink = page('Linked', 'One', {
      examples: [{ id: 'link', title: 'Link', render: () => <a href="/places">Places</a> }]
    })
    render(<ComponentShowcase groups={groupEntries([withLink])} />)
    expect(fireEvent.click(screen.getByRole('link', { name: 'Places' }))).toBe(false)
    expect(fireEvent.click(screen.getByRole('link', { name: 'Linked' }))).toBe(true)
  })
})

describe('when a canvas is drawn inside the gallery', () => {
  it('should leave scrolling to the gallery, which sizes the frame, and restore it on unmount', () => {
    const parent = { postMessage: vi.fn() }
    vi.spyOn(window, 'parent', 'get').mockReturnValue(parent as unknown as Window)
    const { unmount } = render(<Canvas search="?component=Button" />)
    expect(document.documentElement.style.overflowY).toBe('hidden')
    expect(parent.postMessage).toHaveBeenCalled()
    unmount()
    expect(document.documentElement.style.overflowY).toBe('')
  })

  it('should keep its own scrolling when it is open on its own', () => {
    render(<Canvas search="?component=Button" />)
    expect(document.documentElement.style.overflowY).toBe('')
  })
})

describe('when the Showcase draws a page in a canvas inside itself', () => {
  it('should start an example that opens on load closed, and open it from its trigger', () => {
    const { container } = render(<Canvas search="?component=Modal&embedded=1" />)
    expect(container.querySelector('main')!.dataset.component).toBe('Modal')
    expect(document.querySelector('dialog')).toBeNull()
    fireEvent.click(screen.getByRole('button', { name: 'Open message dialog' }))
    expect(document.querySelector('dialog')).not.toBeNull()
  })

  it.each(['?component=Modal', '?component=Modal&embedded=0', '?component=Modal&embedded='])(
    'should open that example on load on the page itself (%s)',
    (search) => {
      render(<Canvas search={search} />)
      expect(document.querySelector('dialog')).not.toBeNull()
    }
  )
})
