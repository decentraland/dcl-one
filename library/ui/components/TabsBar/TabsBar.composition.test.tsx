import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { TabsBar } from './TabsBar'
const items = [
  { id: 'overview', label: 'Overview', href: '#overview' },
  { id: 'campaign', label: 'Campaign', href: '#campaign', hiddenOnMobile: true }
]
describe('when a captured page composition owns route and control placement', () => {
  it('should expose the create composition without inventing controls', () => {
    render(<TabsBar page="create" items={items} activeId="overview" />)
    expect(screen.getByRole('navigation')).toHaveAttribute('data-page', 'create')
    expect(document.querySelector('[data-region="controls"]')).toBeNull()
  })
  it.each(['shop', 'learn'] as const)(
    'should keep caller search and actions mounted once for %s',
    (page) => {
      render(
        <TabsBar
          page={page}
          items={items}
          activeId="overview"
          search={<input aria-label="Search" />}
          actions={<button>Action</button>}
        />
      )
      expect(screen.getAllByRole('textbox', { name: 'Search' })).toHaveLength(1)
      expect(screen.getAllByRole('button', { name: 'Action' })).toHaveLength(1)
      expect(document.querySelector('[data-region="controls"]')).toBeInTheDocument()
    }
  )
  it('should attach explicit mobile visibility to the item rather than matching its label', () => {
    render(<TabsBar page="create" items={items} activeId="campaign" />)
    expect(screen.getByRole('link', { name: 'Campaign' }).closest('li')).toHaveAttribute(
      'data-mobile-hidden'
    )
  })
  it('should preserve zero and empty control values without substituting sample balances', () => {
    render(<TabsBar page="shop" items={[]} activeId="missing" search={null} actions={0} />)
    expect(document.querySelector('[data-region="actions"]')).toHaveTextContent('0')
    expect(screen.queryByRole('link')).toBeNull()
  })
  it('should forward native anchor attributes through the router adapter and omit modified telemetry', () => {
    const select = vi.fn()
    render(
      <TabsBar
        page="create"
        items={items}
        activeId="overview"
        onSelect={select}
        LinkComponent={({ prefetch, ...props }) => <a {...props} data-prefetch={prefetch} />}
      />
    )
    const link = screen.getByRole('link', { name: 'Campaign' })
    expect(link).toHaveAttribute('data-prefetch', 'intent')
    expect(fireEvent.click(link, { metaKey: true })).toBe(true)
    expect(select).not.toHaveBeenCalled()
  })
})

describe('when composition inputs cannot identify accessible routes', () => {
  it.each([
    { id: 'empty-label', label: ' ', href: '#valid' },
    { id: 'empty-href', label: 'Named', href: ' ' }
  ])('should reject blank route names or destinations', (item) => {
    expect(() => render(<TabsBar page="create" items={[item]} activeId="none" />)).toThrow(
      'TabsBar routes require a non-empty label and href'
    )
  })
  it('should reject an unsupported page instead of rendering a misleading composition', () => {
    const invalid = { page: 'unknown' } as unknown as { page: 'create' }
    expect(() => render(<TabsBar {...invalid} items={items} activeId="overview" />)).toThrow(
      'Unsupported TabsBar page'
    )
  })
  it('should keep one uncontrolled search node and its value after a controlled route update', () => {
    const { rerender } = render(
      <TabsBar
        page="shop"
        items={items}
        activeId="overview"
        search={<input aria-label="Search" defaultValue="Ada" />}
        actions={null}
      />
    )
    const input = screen.getByRole('textbox', { name: 'Search' })
    fireEvent.change(input, { target: { value: 'Changed' } })
    rerender(
      <TabsBar
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
