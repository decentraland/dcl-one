import { render, screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { ShopCartDrawer, type ShopCartDrawerProps } from './ShopCartDrawer'

const props: ShopCartDrawerProps = {
  open: false,
  justAdded: 0,
  getContents: () => ({ count: 0, lines: [], total: 0 }),
  credits: (amount) => String(amount),
  cartHref: '/shop/cart',
  itemsHref: '/shop/items',
  onCartPage: false,
  icons: { close: null, remove: null },
  Link: ({ to, children }) => <a href={to}>{children}</a>,
  emptyArt: 'empty.svg',
  close: () => {}
}

describe('when the cart drawer is closed', () => {
  it('should defer reading the cart contents until the drawer opens', () => {
    const getContents = vi.fn(props.getContents)
    const { rerender } = render(<ShopCartDrawer {...props} getContents={getContents} />)
    expect(getContents).not.toHaveBeenCalled()

    rerender(<ShopCartDrawer {...props} open getContents={getContents} />)
    expect(getContents).toHaveBeenCalled()
    expect(within(screen.getByRole('dialog')).getByText('Nothing here... yet!')).toBeVisible()
  })
})
