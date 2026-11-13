import { act, type AnchorHTMLAttributes } from 'react'
import { renderToString } from 'react-dom/server'
import { hydrateRoot } from 'react-dom/client'
import { render, screen, fireEvent } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { TabsBar } from './TabsBar'

const items = [
  { id: 'news', label: 'News', href: '#news' },
  { id: 'guides', label: 'Guides', href: '#guides' }
] as const

describe('when sections are routes', () => {
  it('should expose a named navigation list of native links and the current page', () => {
    render(<TabsBar items={items} activeId="news" label="Blog sections" />)
    expect(screen.getByRole('navigation', { name: 'Blog sections' })).toBeInTheDocument()
    expect(screen.getByRole('list')).toBeInTheDocument()
    expect(screen.getAllByRole('listitem')).toHaveLength(2)
    expect(screen.getByRole('link', { name: 'News' })).toHaveAttribute('href', '#news')
    expect(screen.getByRole('link', { name: 'News' })).toHaveAttribute('aria-current', 'page')
    expect(screen.getByRole('link', { name: 'Guides' })).not.toHaveAttribute('aria-current')
    expect(screen.queryByRole('tablist')).toBeNull()
  })
  it('should notify ordinary selections without preventing native routing', () => {
    const onSelect = vi.fn()
    render(<TabsBar items={items} activeId="news" onSelect={onSelect} />)
    expect(fireEvent.click(screen.getByRole('link', { name: 'Guides' }))).toBe(true)
    expect(onSelect).toHaveBeenCalledExactlyOnceWith('guides')
    expect(screen.getByRole('link', { name: 'News' })).toHaveAttribute('aria-current', 'page')
  })
  it.each(['ctrlKey', 'metaKey', 'shiftKey', 'altKey'] as const)(
    'should omit selection telemetry for %s clicks',
    (modifier) => {
      const onSelect = vi.fn()
      render(<TabsBar items={items} activeId="news" onSelect={onSelect} />)
      expect(
        fireEvent.click(screen.getByRole('link', { name: 'Guides' }), {
          [modifier]: true
        })
      ).toBe(true)
      expect(onSelect).not.toHaveBeenCalled()
    }
  )
  it('should leave middle clicks and context menus native without selection telemetry', () => {
    const onSelect = vi.fn()
    render(<TabsBar items={items} activeId="news" onSelect={onSelect} />)
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
  it('should honor cancellation by an ancestor capture handler', () => {
    const onSelect = vi.fn()
    render(
      <div onClickCapture={(event) => event.preventDefault()}>
        <TabsBar items={items} activeId="news" onSelect={onSelect} />
      </div>
    )
    expect(fireEvent.click(screen.getByRole('link', { name: 'Guides' }))).toBe(false)
    expect(onSelect).not.toHaveBeenCalled()
  })
  it('should delegate intent prefetch and ordinary routing to an app link adapter', () => {
    const route = vi.fn()
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
    render(<TabsBar items={items} activeId="news" LinkComponent={Adapter} />)
    const link = screen.getByRole('link', { name: 'Guides' })
    expect(link).toHaveAttribute('data-prefetch', 'intent')
    expect(fireEvent.click(link, { metaKey: true })).toBe(true)
    expect(route).not.toHaveBeenCalled()
    expect(fireEvent.click(link)).toBe(false)
    expect(route).toHaveBeenCalledExactlyOnceWith('#guides')
  })
  it('should skip telemetry if an adapter cancels before invoking the supplied handler', () => {
    const onSelect = vi.fn()
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
    render(<TabsBar items={items} activeId="news" LinkComponent={Adapter} onSelect={onSelect} />)
    fireEvent.click(screen.getByRole('link', { name: 'Guides' }))
    expect(onSelect).not.toHaveBeenCalled()
  })
  it('should preserve ordinary Tab and Enter behavior without a custom roving tab index', async () => {
    const onSelect = vi.fn()
    render(<TabsBar items={items} activeId="guides" onSelect={onSelect} />)
    await userEvent.tab()
    expect(screen.getByRole('link', { name: 'News' })).toHaveFocus()
    await userEvent.keyboard('{Enter}')
    expect(onSelect).toHaveBeenCalledExactlyOnceWith('news')
    await userEvent.tab()
    expect(screen.getByRole('link', { name: 'Guides' })).toHaveFocus()
  })
  it('should use the controlled active ID after reordering', () => {
    const { rerender } = render(<TabsBar items={items} activeId="news" />)
    rerender(<TabsBar items={[items[1], items[0]]} activeId="guides" />)
    expect(screen.getAllByRole('link')[0]).toHaveAttribute('aria-current', 'page')
    expect(screen.getByRole('link', { name: 'News' })).not.toHaveAttribute('aria-current')
  })
  it('should allow empty results and an unknown current route without inventing a current page', () => {
    const { rerender } = render(<TabsBar items={[]} activeId="unknown" />)
    expect(screen.queryByRole('link')).toBeNull()
    rerender(<TabsBar items={items} activeId="unknown" />)
    expect(document.querySelector('[aria-current]')).toBeNull()
  })
  it.each([
    [{ id: '', label: 'Bad', href: '#bad' }],
    [
      { id: 'same', label: 'One', href: '#one' },
      { id: 'same', label: 'Two', href: '#two' }
    ]
  ])('should reject missing or duplicate route identities', (...invalid) => {
    expect(() => render(<TabsBar items={invalid} activeId="unknown" />)).toThrow(
      'TabsBar item IDs must be unique non-empty strings'
    )
  })
})


describe('when a server-rendered parent hydrates with caller-owned controls', () => {
  it('should preserve the input node and value while the controlled current route changes', async () => {
    const view = (activeId: string) => (
      <TabsBar
        page="shop"
        items={items}
        activeId={activeId}
        search={<input aria-label="Search catalog" defaultValue="Draft" />}
        actions={<button>Cart, 0</button>}
      />
    )
    const container = document.createElement('div')
    container.innerHTML = renderToString(view('news'))
    document.body.append(container)
    const input = container.querySelector('input')
    const recoverableErrors: unknown[] = []
    const root = hydrateRoot(container, view('news'), {
      onRecoverableError: (error) => recoverableErrors.push(error)
    })
    try {
      await act(async () => {})
      expect(recoverableErrors).toEqual([])
      expect(container.querySelector('input')).toBe(input)
      expect(input).toHaveValue('Draft')
      await act(async () => root.render(view('guides')))
      expect(container.querySelector('input')).toBe(input)
      expect(input).toHaveValue('Draft')
      expect(container.querySelector('[aria-current]')).toHaveAttribute('href', '#guides')
      expect(container.querySelectorAll('button')).toHaveLength(1)
    } finally {
      await act(async () => root.unmount())
      container.remove()
    }
  })
})
