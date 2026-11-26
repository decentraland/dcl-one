import { fireEvent, render, type RenderResult, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, it, expect, type Mock, vi } from 'vitest'
import { TitleBar, type TitleBarProps } from './TitleBar'
const sections = [
  { id: 'one', label: 'One', icon: <svg /> },
  { id: 'two', label: 'Two', icon: <svg /> }
]
const fixtures = [
  {
    variant: 'backpack',
    title: 'Backpack',
    controls: {
      filters: <button>Filter</button>,
      search: <input aria-label="Search" />
    }
  },
  { variant: 'settings', title: 'Settings' },
  {
    variant: 'events',
    title: 'Events',
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
        expect(screen.getByRole('button', { name: 'One' })).toHaveAttribute('aria-current', 'true')
        expect(screen.getByRole('button', { name: 'Two' })).not.toHaveAttribute('aria-current')
        expect(document.querySelector('[data-variant]')).toHaveAttribute(
          'data-variant',
          fixture.variant
        )
        expect(document.querySelectorAll('input')).toHaveLength(
          ['backpack', 'places', 'shop'].includes(fixture.variant) ? 1 : 0
        )
      }
    )
  describe('and its owner controls the selection', () => {
    let select: Mock
    let rerender: RenderResult['rerender']

    beforeEach(() => {
      select = vi.fn()
      rerender = render(
        <TitleBar variant="settings" sections={sections} activeId="one" onSelect={select} />
      ).rerender
    })

    afterEach(() => {
      vi.resetAllMocks()
    })

    it('should preserve a controlled selection until its owner updates it', async () => {
      await userEvent.setup().click(screen.getByRole('button', { name: 'Two' }))
      expect(select).toHaveBeenCalledExactlyOnceWith('two')
      expect(screen.getByRole('button', { name: 'One' })).toHaveAttribute('aria-current', 'true')
      rerender(<TitleBar variant="settings" sections={sections} activeId="two" onSelect={select} />)
      expect(screen.getByRole('button', { name: 'Two' })).toHaveAttribute('aria-current', 'true')
      await userEvent.setup().click(screen.getByRole('button', { name: 'Two' }))
      expect(select).toHaveBeenCalledTimes(2)
      expect(select).toHaveBeenLastCalledWith('two')
      expect(screen.getByRole('button', { name: 'Two' })).toHaveAttribute('aria-current', 'true')
    })
  })
  describe('and a section is a route link', () => {
    let select: Mock

    beforeEach(() => {
      select = vi.fn()
      render(
        <TitleBar
          variant="settings"
          sections={[{ id: 'route', label: 'Route', icon: <svg />, href: '#route' }]}
          activeId="none"
          onSelect={select}
        />
      )
    })

    afterEach(() => {
      vi.resetAllMocks()
    })

    it('should keep route links native and omit modified selection callbacks', () => {
      const link = screen.getByRole('link', { name: 'Route' })
      expect(fireEvent.click(link, { metaKey: true })).toBe(true)
      expect(select).not.toHaveBeenCalled()
      expect(fireEvent.click(link)).toBe(true)
      expect(select).toHaveBeenCalledExactlyOnceWith('route')
    })
  })
  it('should preserve a supplied Places popup outside any interactive control', () => {
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
  describe('and the controls are incomplete', () => {
    let bad: TitleBarProps

    beforeEach(() => {
      bad = {
        variant: 'shop',
        controls: { search: null }
      } as unknown as TitleBarProps
    })

    it('should reject incomplete controls rather than invent source actions', () => {
      expect(() =>
        render(<TitleBar {...bad} sections={[]} activeId="none" onSelect={() => {}} />)
      ).toThrow('Missing TitleBar control: filters')
    })
  })
  describe('and an ancestor cancels the click', () => {
    let select: Mock

    beforeEach(() => {
      select = vi.fn()
      render(
        <div onClickCapture={(event) => event.preventDefault()}>
          <TitleBar variant="settings" sections={sections} activeId="one" onSelect={select} />
        </div>
      )
    })

    afterEach(() => {
      vi.resetAllMocks()
    })

    it('should honor cancellation before a native command activation', () => {
      expect(fireEvent.click(screen.getByRole('button', { name: 'Two' }))).toBe(false)
      expect(select).not.toHaveBeenCalled()
    })
  })
  describe('and an untyped caller provides raw HTML overrides', () => {
    let raw: { dangerouslySetInnerHTML: { __html: string } }

    beforeEach(() => {
      raw = { dangerouslySetInnerHTML: { __html: '<b>Injected</b>' } }
      render(
        <TitleBar
          variant="settings"
          {...raw}
          sections={sections}
          activeId="none"
          onSelect={() => {}}
        />
      )
    })

    it('should preserve owned content against raw HTML overrides', () => {
      expect(screen.getByRole('heading', { name: 'Settings' })).toBeInTheDocument()
      expect(screen.queryByText('Injected')).toBeNull()
    })
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
    expect(screen.getByRole('heading', { name: 'Events' })).toBeInTheDocument()
    expect(document.querySelector('[data-active]')).toBeNull()
  })
})
describe('when a section is supplied by the app router', () => {
  let select: Mock

  beforeEach(() => {
    select = vi.fn()
  })

  afterEach(() => {
    vi.resetAllMocks()
  })

  it.each(['ctrlKey', 'metaKey', 'shiftKey', 'altKey'] as const)(
    'should retain native %s navigation without selecting the current view',
    (modifier) => {
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
    expect(link).toHaveAttribute('aria-current', 'page')
    expect(fireEvent.click(link)).toBe(false)
    expect(select).not.toHaveBeenCalled()
  })
})

describe('when untyped section configuration is malformed', () => {
  describe.each([{ label: undefined }, { label: 12 }, { href: null }, { href: 12 }])(
    'and the section carries %j',
    (invalid) => {
      let props: TitleBarProps

      beforeEach(() => {
        props = {
          variant: 'settings',
          sections: [{ id: 'section', label: 'Section', icon: null, ...invalid }],
          activeId: 'section',
          onSelect: () => {}
        } as unknown as TitleBarProps
      })

      it('should reject it with its configuration error before reading string methods', () => {
        expect(() => render(<TitleBar {...props} />)).toThrow(
          'TitleBar sections require a non-empty label and destination'
        )
      })
    }
  )
})

describe('when adopting a title bar as a page or nested heading', () => {
  it.each([1, 3, 6] as const)(
    'should expose its requested level %s without changing its visual class',
    (headingLevel) => {
      render(
        <TitleBar
          variant="settings"
          headingLevel={headingLevel}
          sections={sections}
          activeId="graphics"
          onSelect={() => {}}
        />
      )
      expect(
        screen.getByRole('heading', { name: 'Settings', level: headingLevel }).className
      ).toContain('title')
    }
  )
  it('should expose current view buttons without toggle semantics', () => {
    render(
      <TitleBar
        variant="settings"
        sections={sections}
        activeId={sections[0]!.id}
        onSelect={() => {}}
      />
    )
    const current = screen.getByRole('button', { name: sections[0]!.label })
    expect(current).toHaveAttribute('aria-current', 'true')
    expect(current).not.toHaveAttribute('aria-pressed')
  })
  describe('and an adapter handles routing', () => {
    let select: Mock
    let prefetch: Mock

    beforeEach(() => {
      select = vi.fn()
      prefetch = vi.fn()
    })

    afterEach(() => {
      vi.resetAllMocks()
    })

    it('should forward app routing hints and select once before the adapter handles routing', () => {
      render(
        <TitleBar
          variant="settings"
          sections={[{ id: 'route', label: 'Route', href: '#route', icon: null }]}
          activeId="route"
          onSelect={select}
          LinkComponent={({ prefetch: hint, ...props }) => {
            prefetch(hint)
            return (
              <a
                {...props}
                onClick={(event) => {
                  props.onClick?.(event)
                  event.preventDefault()
                }}
              />
            )
          }}
        />
      )
      expect(fireEvent.click(screen.getByRole('link', { name: 'Route' }))).toBe(false)
      expect(select).toHaveBeenCalledExactlyOnceWith('route')
      expect(prefetch).toHaveBeenCalledWith('intent')
    })
  })
})

describe('when a heading level configuration is invalid', () => {
  it.each([0, 7, 1.5, 'h1'])('should reject unsupported level %s', (headingLevel) => {
    expect(() =>
      render(
        <TitleBar
          variant="settings"
          headingLevel={headingLevel as never}
          sections={sections}
          activeId="one"
          onSelect={() => {}}
        />
      )
    ).toThrow('TitleBar heading level must be an integer from 1 to 6')
  })
})

describe('when sections are page routes', () => {
  let routes: { id: string; label: string; icon: null; href: string }[]

  beforeEach(() => {
    routes = [
      { id: 'one', label: 'One', icon: null, href: '/one' },
      { id: 'two', label: 'Two', icon: null, href: '#two' }
    ]
  })

  it('should mark only the current native anchor as the page inside a named navigation landmark', () => {
    render(
      <TitleBar
        variant="shop"
        sections={routes}
        activeId="two"
        controls={{ filters: null, sort: null, search: null, cart: null }}
      />
    )
    const nav = screen.getByRole('navigation', { name: 'Shop sections' })
    expect(nav).toContainElement(screen.getByRole('link', { name: 'Two' }))
    expect(screen.getByRole('link', { name: 'Two' })).toHaveAttribute('aria-current', 'page')
    expect(screen.getByRole('link', { name: 'One' })).not.toHaveAttribute('aria-current')
  })
  it('should keep button-only sections in a group instead of a navigation landmark', () => {
    render(<TitleBar variant="settings" sections={sections} activeId="one" onSelect={() => {}} />)
    expect(screen.queryByRole('navigation')).toBeNull()
    expect(screen.getByRole('group', { name: 'Settings sections' })).toBeInTheDocument()
  })
  it('should follow a plain route click without a selection callback', () => {
    render(<TitleBar variant="settings" sections={routes} activeId="one" />)
    expect(fireEvent.click(screen.getByRole('link', { name: 'Two' }))).toBe(true)
  })
  it('should require a selection callback once any section is a view button', () => {
    expect(() =>
      render(
        <TitleBar
          variant="settings"
          sections={[...routes, { id: 'view', label: 'View', icon: null }]}
          activeId="one"
        />
      )
    ).toThrow('TitleBar requires a controlled section callback')
  })
  it('should reject a selection callback that is not a function', () => {
    expect(() =>
      render(
        <TitleBar
          variant="settings"
          sections={routes}
          activeId="one"
          onSelect={'select' as never}
        />
      )
    ).toThrow('TitleBar requires a controlled section callback')
  })
  it.each([
    'javascript:alert(1)',
    'data:text/html,hi',
    'ftp://example.org',
    'https://user:pass@example.org',
    ' '
  ])('should reject the unsafe destination %j before rendering a link', (href) => {
    expect(() =>
      render(
        <TitleBar
          variant="settings"
          sections={[{ id: 'route', label: 'Route', icon: null, href }]}
          activeId="route"
        />
      )
    ).toThrow('TitleBar sections require a non-empty label and destination')
  })
  it.each([null, undefined, 5])(
    'should reject a %s section entry with the configuration error',
    (entry) => {
      expect(() =>
        render(
          <TitleBar
            variant="settings"
            sections={[entry as never]}
            activeId="none"
            onSelect={() => {}}
          />
        )
      ).toThrow('TitleBar section IDs must be unique non-empty strings')
    }
  )
})

describe('when the bar is named without a caller label', () => {
  it('should name Settings by its title alone, since it has no controls region', () => {
    render(<TitleBar variant="settings" sections={sections} activeId="one" onSelect={() => {}} />)
    expect(screen.getByRole('group', { name: 'Settings' })).toHaveAttribute(
      'data-variant',
      'settings'
    )
  })
  it('should name a variant with controls after them', () => {
    render(
      <TitleBar
        variant="backpack"
        sections={sections}
        activeId="one"
        onSelect={() => {}}
        controls={{ filters: null, search: null }}
      />
    )
    expect(screen.getByRole('group', { name: 'Backpack controls' })).toHaveAttribute(
      'data-variant',
      'backpack'
    )
  })
})
