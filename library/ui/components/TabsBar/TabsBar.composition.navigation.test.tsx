import { fireEvent, render, type RenderResult, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, type Mock, vi } from 'vitest'
import { TabsBar, type TabsBarNavigationProps } from './TabsBar'

function createItems(): TabsBarNavigationProps['items'] {
  return [
    { id: 'overview', label: 'Overview', href: '#overview' },
    {
      id: 'campaign',
      label: 'Campaign',
      href: '#campaign',
      hiddenOnMobile: true
    }
  ]
}

describe('when a captured page composition owns route and control placement', () => {
  let items: TabsBarNavigationProps['items']

  beforeEach(() => {
    items = createItems()
  })

  describe('and the page is create', () => {
    beforeEach(() => {
      render(<TabsBar mode="navigation" page="create" items={items} activeId="overview" />)
    })

    it('should expose the create composition without inventing controls', () => {
      expect(screen.getByRole('navigation')).toHaveAttribute('data-page', 'create')
      expect(document.querySelector('[data-region="controls"]')).toBeNull()
    })
  })

  describe.each(['shop', 'learn'] as const)('and the page is %s', (page) => {
    beforeEach(() => {
      render(
        <TabsBar
          mode="navigation"
          page={page}
          items={items}
          activeId="overview"
          search={<input aria-label="Search" />}
          actions={<button>Action</button>}
        />
      )
    })

    it(`should keep caller search and actions mounted once for ${page}`, () => {
      expect(screen.getAllByRole('textbox', { name: 'Search' })).toHaveLength(1)
      expect(screen.getAllByRole('button', { name: 'Action' })).toHaveLength(1)
      expect(document.querySelector('[data-region="controls"]')).toBeInTheDocument()
    })
  })

  it('renders only the section links when the Shop host owns the controls', () => {
    render(<TabsBar mode="navigation" page="shop" layout="links" items={items} activeId="overview" />)
    const nav = screen.getByRole('navigation')
    expect(nav.querySelector('[data-region="controls"]')).toBeNull()
    expect(screen.getByRole('link', { name: 'Overview' })).toHaveAttribute('aria-current', 'page')
  })

  it('keeps a narrow-only route and a campaign label separate from the current link marker', () => {
    const [overview, campaign] = items
    render(<TabsBar mode="navigation" page="shop" layout="links" items={[
      { ...overview!, mobileOnly: true }, { ...campaign!, accent: 'campaign' }
    ]} activeId="campaign" />)
    expect(screen.getByRole('link', { name: 'Overview' }).closest('li')).toHaveAttribute('data-mobile-only')
    expect(screen.getByRole('link', { name: 'Campaign' }).closest('li')).toHaveAttribute('data-accent', 'campaign')
  })

  it('keeps the current mobile-only route visible on desktop', () => {
    const [overview, campaign] = items
    render(<TabsBar mode="navigation" page="shop" layout="links" items={[
      { ...overview!, mobileOnly: true }, campaign!
    ]} activeId="overview" />)
    expect(screen.getByRole('link', { current: 'page' }).closest('li')).not.toHaveAttribute('data-mobile-only')
  })

  describe('and the page is blog', () => {
    beforeEach(() => {
      render(
        <TabsBar
          mode="navigation"
          page="blog"
          label="Blog categories"
          items={items}
          activeId="overview"
          search={<input type="search" aria-label="Search articles" />}
        />
      )
    })

    it('should show the caller search field', () => {
      expect(screen.getByRole('searchbox', { name: 'Search articles' })).toBeInTheDocument()
    })

    it('should omit the actions region', () => {
      expect(
        screen
          .getByRole('navigation', { name: 'Blog categories' })
          .querySelector('[data-region="actions"]')
      ).toBeNull()
    })
  })

  describe('and a route is hidden on mobile', () => {
    let rendered: RenderResult

    beforeEach(() => {
      rendered = render(
        <TabsBar mode="navigation" page="create" items={items} activeId="overview" />
      )
    })

    it('should hide only inactive mobile routes and retain the controlled current page', () => {
      expect(screen.getByRole('link', { name: 'Campaign' }).closest('li')).toHaveAttribute(
        'data-mobile-hidden'
      )
      rendered.rerender(
        <TabsBar mode="navigation" page="create" items={items} activeId="campaign" />
      )
      expect(screen.getByRole('link', { name: 'Campaign' }).closest('li')).not.toHaveAttribute(
        'data-mobile-hidden'
      )
    })
  })

  describe.each([
    { id: 'bad', label: undefined, href: '#valid' },
    { id: 'bad', label: 12, href: '#valid' },
    { id: 'bad', label: 'Named', href: undefined },
    { id: 'bad', label: 'Named', href: 12 }
  ])('and untyped route data is malformed as %j', (item) => {
    let invalid: TabsBarNavigationProps['items']

    beforeEach(() => {
      invalid = [item] as unknown as TabsBarNavigationProps['items']
    })

    it('should reject it with its public configuration error', () => {
      expect(() => render(<TabsBar mode="navigation" items={invalid} activeId="bad" />)).toThrow(
        'TabsBar routes require a non-empty label and href'
      )
    })
  })

  describe('and control values are zero or empty', () => {
    beforeEach(() => {
      render(
        <TabsBar
          mode="navigation"
          page="shop"
          items={[]}
          activeId="missing"
          search={null}
          actions={0}
        />
      )
    })

    it('should preserve them without substituting sample balances', () => {
      expect(document.querySelector('[data-region="actions"]')).toHaveTextContent('0')
      expect(screen.queryByRole('link')).toBeNull()
    })
  })

  describe('and a router adapter renders the links', () => {
    let select: Mock

    beforeEach(() => {
      select = vi.fn()
      render(
        <TabsBar
          mode="navigation"
          page="create"
          items={items}
          activeId="overview"
          onSelect={select}
          LinkComponent={({ prefetch, ...props }) => <a {...props} data-prefetch={prefetch} />}
        />
      )
    })

    afterEach(() => {
      vi.resetAllMocks()
    })

    it('should forward native anchor attributes and omit modified telemetry', () => {
      const link = screen.getByRole('link', { name: 'Campaign' })
      expect(link).toHaveAttribute('data-prefetch', 'intent')
      expect(fireEvent.click(link, { metaKey: true })).toBe(true)
      expect(select).not.toHaveBeenCalled()
    })
  })
})

describe('when composition inputs cannot identify accessible routes', () => {
  let items: TabsBarNavigationProps['items']

  beforeEach(() => {
    items = createItems()
  })

  describe('and a route name or destination is blank', () => {
    it.each([
      { id: 'empty-label', label: ' ', href: '#valid' },
      { id: 'empty-href', label: 'Named', href: ' ' }
    ])('should reject it', (item) => {
      expect(() =>
        render(<TabsBar mode="navigation" page="create" items={[item]} activeId="none" />)
      ).toThrow('TabsBar routes require a non-empty label and href')
    })
  })

  describe('and the page is unsupported', () => {
    let invalid: { page: 'create' }

    beforeEach(() => {
      invalid = { page: 'unknown' } as unknown as { page: 'create' }
    })

    it('should reject it instead of rendering a misleading composition', () => {
      expect(() =>
        render(<TabsBar mode="navigation" {...invalid} items={items} activeId="overview" />)
      ).toThrow('Unsupported TabsBar page')
    })
  })

  describe('and the caller owns an uncontrolled search', () => {
    let rendered: RenderResult

    beforeEach(() => {
      rendered = render(
        <TabsBar
          mode="navigation"
          page="shop"
          items={items}
          activeId="overview"
          search={<input aria-label="Search" defaultValue="Ada" />}
          actions={null}
        />
      )
    })

    it('should keep one search node and its value after a controlled route update', () => {
      const input = screen.getByRole('textbox', { name: 'Search' })
      fireEvent.change(input, { target: { value: 'Changed' } })
      rendered.rerender(
        <TabsBar
          mode="navigation"
          page="shop"
          items={items}
          activeId="campaign"
          search={<input aria-label="Search" defaultValue="Ada" />}
          actions={null}
        />
      )
      expect(screen.getByRole('textbox', { name: 'Search' })).toBe(input)
      expect(input).toHaveValue('Changed')
    })
  })
})
