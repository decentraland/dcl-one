import { fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, it, expect, vi } from 'vitest'
import { TitleBar } from './TitleBar'
const sections = [
  { id: 'one', label: 'One', icon: <svg /> },
  { id: 'two', label: 'Two', icon: <svg /> }
]
const fixtures = [
  {
    variant: 'backpack',
    title: 'Backpack',
    controls: {
      collection: <button>Collection</button>,
      filters: <button>Filter</button>,
      search: <input aria-label="Search" />
    }
  },
  { variant: 'settings', title: 'Settings' },
  {
    variant: 'events',
    title: 'Hangouts',
    controls: { date: <button>Today</button>, create: <a href="#create">Create hangout</a> }
  },
  {
    variant: 'places',
    title: 'Places',
    controls: { filters: <button>Filter</button>, search: <input aria-label="Search" /> }
  },
  {
    variant: 'shop',
    title: 'Shop',
    controls: {
      filters: <button>Filter</button>,
      sort: <button>Sort</button>,
      search: <input aria-label="Search" />,
      cart: <button>Cart</button>
    }
  }
] as const
describe('when a captured application toolbar is composed', () => {
  for (const fixture of fixtures)
    it(
      'should render ' +
        fixture.variant +
        ' with its real title, controlled sections and specific control regions',
      () => {
        render(<TitleBar {...fixture} sections={sections} activeId="one" onSelect={() => {}} />)
        expect(screen.getByRole('heading', { name: fixture.title })).toBeInTheDocument()
        expect(screen.getByRole('button', { name: 'One' })).toHaveAttribute('aria-pressed', 'true')
        expect(screen.getByRole('button', { name: 'Two' })).toHaveAttribute('aria-pressed', 'false')
        expect(document.querySelector('[data-variant]')).toHaveAttribute(
          'data-variant',
          fixture.variant
        )
        expect(document.querySelectorAll('input')).toHaveLength(
          ['backpack', 'places', 'shop'].includes(fixture.variant) ? 1 : 0
        )
      }
    )
  it('should preserve a controlled selection until its owner updates it', async () => {
    const select = vi.fn()
    const { rerender } = render(
      <TitleBar variant="settings" sections={sections} activeId="one" onSelect={select} />
    )
    await userEvent.setup().click(screen.getByRole('button', { name: 'Two' }))
    expect(select).toHaveBeenCalledExactlyOnceWith('two')
    expect(screen.getByRole('button', { name: 'One' })).toHaveAttribute('aria-pressed', 'true')
    rerender(<TitleBar variant="settings" sections={sections} activeId="two" onSelect={select} />)
    expect(screen.getByRole('button', { name: 'Two' })).toHaveAttribute('aria-pressed', 'true')
  })
  it('should keep route links native and omit modified selection callbacks', () => {
    const select = vi.fn()
    render(
      <TitleBar
        variant="settings"
        sections={[{ id: 'route', label: 'Route', icon: <svg />, href: '#route' }]}
        activeId="none"
        onSelect={select}
      />
    )
    const link = screen.getByRole('link', { name: 'Route' })
    expect(fireEvent.click(link, { metaKey: true })).toBe(true)
    expect(select).not.toHaveBeenCalled()
    expect(fireEvent.click(link)).toBe(true)
    expect(select).toHaveBeenCalledExactlyOnceWith('route')
  })
  it('should preserve a supplied Places popup without clipping or wrapping it in an interactive control', () => {
    render(
      <TitleBar
        variant="places"
        sections={sections}
        activeId="one"
        onSelect={() => {}}
        controls={{
          filters: <button>Filter</button>,
          search: <input aria-label="Search" />,
          filterMenu: (
            <div role="group" aria-label="Sort choices">
              <button>Trending</button>
            </div>
          )
        }}
      />
    )
    expect(screen.getByRole('group', { name: 'Sort choices' })).toBeInTheDocument()
    expect(document.querySelector('button button,a a,button a,a button')).toBeNull()
    expect(document.querySelector('[data-control="filterMenu"]')).toBeInTheDocument()
  })
})
describe('when caller-owned toolbar data is empty or refused', () => {
  it('should omit a boolean-false Places overlay instead of inserting an empty floating box', () => {
    render(
      <TitleBar
        variant="places"
        sections={[]}
        activeId="none"
        onSelect={() => {}}
        controls={{ filters: null, search: null, filterMenu: false }}
      />
    )
    expect(document.querySelector('[data-control="filterMenu"]')).toBeNull()
  })
  it('should preserve a real zero-valued cart without replacing it with captured sample data', () => {
    render(
      <TitleBar
        variant="shop"
        sections={[]}
        activeId="none"
        onSelect={() => {}}
        controls={{ filters: null, sort: null, search: null, cart: 0 }}
      />
    )
    expect(document.querySelector('[data-control="cart"]')).toHaveTextContent('0')
    expect(screen.queryByRole('button')).toBeNull()
  })
  it.each([
    [
      { id: 'same', label: 'One', icon: null },
      { id: 'same', label: 'Two', icon: null }
    ],
    [{ id: '', label: 'One', icon: null }]
  ])('should reject missing or repeated section identities', (...invalid) => {
    expect(() =>
      render(<TitleBar variant="settings" sections={invalid} activeId="none" onSelect={() => {}} />)
    ).toThrow('TitleBar section IDs must be unique non-empty strings')
  })
  it('should reject incomplete controls rather than invent source actions', () => {
    const bad = {
      variant: 'shop',
      controls: { search: null }
    } as unknown as import('./TitleBar').TitleBarProps
    expect(() =>
      render(<TitleBar {...bad} sections={[]} activeId="none" onSelect={() => {}} />)
    ).toThrow('Missing TitleBar control: filters')
  })
  it('should honor cancellation before a native command activation', () => {
    const select = vi.fn()
    render(
      <div onClickCapture={(event) => event.preventDefault()}>
        <TitleBar variant="settings" sections={sections} activeId="one" onSelect={select} />
      </div>
    )
    expect(fireEvent.click(screen.getByRole('button', { name: 'Two' }))).toBe(false)
    expect(select).not.toHaveBeenCalled()
  })
  it('should preserve owned content against raw HTML overrides', () => {
    const raw = { dangerouslySetInnerHTML: { __html: '<b>Injected</b>' } }
    render(
      <TitleBar
        variant="settings"
        {...raw}
        sections={sections}
        activeId="none"
        onSelect={() => {}}
      />
    )
    expect(screen.getByRole('heading', { name: 'Settings' })).toBeInTheDocument()
    expect(screen.queryByText('Injected')).toBeNull()
  })
  it('should use its real fallback title and expose no selected state for an unknown controlled ID', () => {
    render(
      <TitleBar
        variant="events"
        title=" "
        sections={sections}
        activeId="missing"
        onSelect={() => {}}
        controls={{ date: null, create: null }}
      />
    )
    expect(screen.getByRole('heading', { name: 'Hangouts' })).toBeInTheDocument()
    expect(document.querySelector('[data-active]')).toBeNull()
  })
})
describe('when a section is supplied by the app router', () => {
  it.each(['ctrlKey', 'metaKey', 'shiftKey', 'altKey'] as const)(
    'should retain native %s navigation without selecting the current view',
    (modifier) => {
      const select = vi.fn()
      render(
        <TitleBar
          variant="settings"
          sections={[{ id: 'route', label: 'Route', icon: null, href: '#route' }]}
          activeId="none"
          onSelect={select}
        />
      )
      expect(
        fireEvent.click(screen.getByRole('link', { name: 'Route' }), { [modifier]: true })
      ).toBe(true)
      expect(select).not.toHaveBeenCalled()
    }
  )
  it('should honor adapter cancellation and forward its active layout attributes', () => {
    const select = vi.fn()
    render(
      <TitleBar
        variant="settings"
        sections={[{ id: 'route', label: 'Route', icon: null, href: '#route' }]}
        activeId="route"
        onSelect={select}
        LinkComponent={(props) => (
          <a
            {...props}
            onClick={(event) => {
              event.preventDefault()
              props.onClick?.(event)
            }}
          />
        )}
      />
    )
    const link = screen.getByRole('link', { name: 'Route' })
    expect(link).toHaveAttribute('data-active', 'true')
    expect(fireEvent.click(link)).toBe(false)
    expect(select).not.toHaveBeenCalled()
  })
})

describe('when untyped section configuration is malformed', () => {
  it.each([{ label: undefined }, { label: 12 }, { href: null }, { href: 12 }])(
    'should reject %j with its configuration error before reading string methods',
    (invalid) => {
      const props = {
        variant: 'settings',
        sections: [{ id: 'section', label: 'Section', icon: null, ...invalid }],
        activeId: 'section',
        onSelect: () => {}
      } as unknown as import('./TitleBar').TitleBarProps
      expect(() => render(<TitleBar {...props} />)).toThrow(
        'TitleBar sections require a non-empty label and destination'
      )
    }
  )
})
