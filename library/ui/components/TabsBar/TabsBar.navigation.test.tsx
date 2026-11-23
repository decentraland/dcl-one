import { act, type AnchorHTMLAttributes, type ReactElement } from 'react'
import { renderToString } from 'react-dom/server'
import { hydrateRoot, type Root } from 'react-dom/client'
import { render, screen, fireEvent, type RenderResult } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, type Mock, vi } from 'vitest'
import { TabsBar } from './TabsBar'

const items = [
  { id: 'news', label: 'News', href: '#news' },
  { id: 'guides', label: 'Guides', href: '#guides' }
] as const

describe('when sections are routes', () => {
  let onSelect: Mock

  beforeEach(() => {
    onSelect = vi.fn()
  })

  afterEach(() => {
    vi.resetAllMocks()
  })

  describe('and the bar has a label', () => {
    beforeEach(() => {
      render(<TabsBar mode="navigation" items={items} activeId="news" label="Blog sections" />)
    })

    it('should expose a named navigation list of native links and the current page', () => {
      expect(screen.getByRole('navigation', { name: 'Blog sections' })).toBeInTheDocument()
      expect(screen.getByRole('list')).toBeInTheDocument()
      expect(screen.getAllByRole('listitem')).toHaveLength(2)
      expect(screen.getByRole('link', { name: 'News' })).toHaveAttribute('href', '#news')
      expect(screen.getByRole('link', { name: 'News' })).toHaveAttribute('aria-current', 'page')
      expect(screen.getByRole('link', { name: 'Guides' })).not.toHaveAttribute('aria-current')
      expect(screen.queryByRole('tablist')).toBeNull()
    })
  })

  describe('and the caller listens for selections', () => {
    beforeEach(() => {
      render(<TabsBar mode="navigation" items={items} activeId="news" onSelect={onSelect} />)
    })

    it('should notify ordinary selections without preventing native routing', () => {
      expect(fireEvent.click(screen.getByRole('link', { name: 'Guides' }))).toBe(true)
      expect(onSelect).toHaveBeenCalledExactlyOnceWith('guides')
      expect(screen.getByRole('link', { name: 'News' })).toHaveAttribute('aria-current', 'page')
    })

    it.each(['ctrlKey', 'metaKey', 'shiftKey', 'altKey'] as const)(
      'should omit selection telemetry for %s clicks',
      (modifier) => {
        expect(
          fireEvent.click(screen.getByRole('link', { name: 'Guides' }), {
            [modifier]: true
          })
        ).toBe(true)
        expect(onSelect).not.toHaveBeenCalled()
      }
    )

    it('should leave middle clicks and context menus native without selection telemetry', () => {
      const link = screen.getByRole('link', { name: 'Guides' })
      expect(
        fireEvent(
          link,
          new MouseEvent('auxclick', {
            button: 1,
            bubbles: true,
            cancelable: true
          })
        )
      ).toBe(true)
      expect(fireEvent.click(link, { button: 1 })).toBe(true)
      expect(fireEvent.contextMenu(link)).toBe(true)
      expect(onSelect).not.toHaveBeenCalled()
    })
  })

  describe('and an ancestor capture handler cancels the click', () => {
    beforeEach(() => {
      render(
        <div onClickCapture={(event) => event.preventDefault()}>
          <TabsBar mode="navigation" items={items} activeId="news" onSelect={onSelect} />
        </div>
      )
    })

    it('should honor the cancellation', () => {
      expect(fireEvent.click(screen.getByRole('link', { name: 'Guides' }))).toBe(false)
      expect(onSelect).not.toHaveBeenCalled()
    })
  })

  describe('and an app link adapter routes ordinary clicks', () => {
    let route: Mock

    beforeEach(() => {
      route = vi.fn()
      function Adapter({
        prefetch,
        ...props
      }: AnchorHTMLAttributes<HTMLAnchorElement> & {
        href: string
        prefetch?: 'intent'
      }) {
        return (
          <a
            {...props}
            data-prefetch={prefetch}
            onClick={(event) => {
              props.onClick?.(event)
              if (
                !event.defaultPrevented &&
                !event.metaKey &&
                !event.ctrlKey &&
                !event.shiftKey &&
                !event.altKey &&
                event.button === 0
              ) {
                route(props.href)
                event.preventDefault()
              }
            }}
          />
        )
      }
      render(<TabsBar mode="navigation" items={items} activeId="news" LinkComponent={Adapter} />)
    })

    it('should delegate intent prefetch and ordinary routing to the adapter', () => {
      const link = screen.getByRole('link', { name: 'Guides' })
      expect(link).toHaveAttribute('data-prefetch', 'intent')
      expect(fireEvent.click(link, { metaKey: true })).toBe(true)
      expect(route).not.toHaveBeenCalled()
      expect(fireEvent.click(link)).toBe(false)
      expect(route).toHaveBeenCalledExactlyOnceWith('#guides')
    })
  })

  describe('and an adapter cancels before invoking the supplied handler', () => {
    beforeEach(() => {
      function Adapter(props: AnchorHTMLAttributes<HTMLAnchorElement> & { href: string }) {
        return (
          <a
            {...props}
            onClick={(event) => {
              event.preventDefault()
              props.onClick?.(event)
            }}
          />
        )
      }
      render(
        <TabsBar
          mode="navigation"
          items={items}
          activeId="news"
          LinkComponent={Adapter}
          onSelect={onSelect}
        />
      )
    })

    it('should skip selection telemetry', () => {
      fireEvent.click(screen.getByRole('link', { name: 'Guides' }))
      expect(onSelect).not.toHaveBeenCalled()
    })
  })

  describe('and the reader uses the keyboard', () => {
    beforeEach(() => {
      render(<TabsBar mode="navigation" items={items} activeId="guides" onSelect={onSelect} />)
    })

    it('should preserve ordinary Tab and Enter behavior without a custom roving tab index', async () => {
      await userEvent.tab()
      expect(screen.getByRole('link', { name: 'News' })).toHaveFocus()
      await userEvent.keyboard('{Enter}')
      expect(onSelect).toHaveBeenCalledExactlyOnceWith('news')
      await userEvent.tab()
      expect(screen.getByRole('link', { name: 'Guides' })).toHaveFocus()
    })
  })

  describe('and the routes are reordered', () => {
    let rendered: RenderResult

    beforeEach(() => {
      rendered = render(<TabsBar mode="navigation" items={items} activeId="news" />)
    })

    it('should use the controlled active ID', () => {
      rendered.rerender(
        <TabsBar mode="navigation" items={[items[1], items[0]]} activeId="guides" />
      )
      expect(screen.getAllByRole('link')[0]).toHaveAttribute('aria-current', 'page')
      expect(screen.getByRole('link', { name: 'News' })).not.toHaveAttribute('aria-current')
    })
  })

  describe('and there are no results or the current route is unknown', () => {
    let rendered: RenderResult

    beforeEach(() => {
      rendered = render(<TabsBar mode="navigation" items={[]} activeId="unknown" />)
    })

    it('should allow them without inventing a current page', () => {
      expect(screen.queryByRole('link')).toBeNull()
      rendered.rerender(<TabsBar mode="navigation" items={items} activeId="unknown" />)
      expect(document.querySelector('[aria-current]')).toBeNull()
    })
  })

  describe('and route identities are missing or duplicated', () => {
    it.each([
      [{ id: '', label: 'Bad', href: '#bad' }],
      [
        { id: 'same', label: 'One', href: '#one' },
        { id: 'same', label: 'Two', href: '#two' }
      ]
    ])('should reject them', (...invalid) => {
      expect(() =>
        render(<TabsBar mode="navigation" items={invalid} activeId="unknown" />)
      ).toThrow('TabsBar item IDs must be unique non-empty strings')
    })
  })

  describe('and remote configuration supplies an empty route entry', () => {
    it.each([null, undefined])('should reject a %s route with its configuration error', (entry) => {
      expect(() =>
        render(<TabsBar mode="navigation" items={[entry as never]} activeId="unknown" />)
      ).toThrow('TabsBar item IDs must be unique non-empty strings')
    })
  })
})

describe('when a server-rendered parent hydrates with caller-owned controls', () => {
  const view = (activeId: string): ReactElement => (
    <TabsBar
      mode="navigation"
      page="shop"
      items={items}
      activeId={activeId}
      search={<input aria-label="Search catalog" defaultValue="Draft" />}
      actions={<button>Cart, 0</button>}
    />
  )
  let container: HTMLDivElement
  let input: HTMLInputElement | null
  let recoverableErrors: unknown[]
  let root: Root | undefined

  beforeEach(() => {
    container = document.createElement('div')
    container.innerHTML = renderToString(view('news'))
    document.body.append(container)
    input = container.querySelector('input')
    recoverableErrors = []
    root = undefined
  })

  afterEach(async () => {
    const hydrated = root
    if (hydrated) await act(async () => hydrated.unmount())
    container.remove()
  })

  it('should preserve the input node and value while the controlled current route changes', async () => {
    const hydrated = hydrateRoot(container, view('news'), {
      onRecoverableError: (error) => recoverableErrors.push(error)
    })
    root = hydrated
    await act(async () => {})
    expect(recoverableErrors).toEqual([])
    expect(container.querySelector('input')).toBe(input)
    expect(input).toHaveValue('Draft')
    await act(async () => hydrated.render(view('guides')))
    expect(container.querySelector('input')).toBe(input)
    expect(input).toHaveValue('Draft')
    expect(container.querySelector('[aria-current]')).toHaveAttribute('href', '#guides')
    expect(container.querySelectorAll('button')).toHaveLength(1)
  })
})

describe('when the route tray leaves the page', () => {
  let disconnect: Mock
  let rendered: RenderResult

  beforeEach(() => {
    disconnect = vi.fn()
    vi.stubGlobal(
      'ResizeObserver',
      class {
        observe() {}
        disconnect = disconnect
      }
    )
    rendered = render(<TabsBar mode="navigation" items={items} activeId="news" />)
  })

  afterEach(() => {
    vi.unstubAllGlobals()
    vi.resetAllMocks()
  })

  it('should disconnect layout observation on unmount', () => {
    rendered.unmount()
    expect(disconnect).toHaveBeenCalledOnce()
  })
})
