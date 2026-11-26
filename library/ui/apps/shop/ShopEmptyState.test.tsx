import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { ShopEmptyState } from './ShopEmptyState'

describe('ShopEmptyState', () => {
  describe('when a state is informational', () => {
    it('should expose its heading without a dead action', () => {
      render(<ShopEmptyState title="No listings" />)
      expect(screen.getByRole('heading', { level: 2 })).toHaveTextContent('No listings')
      expect(screen.queryByRole('button')).toBeNull()
      expect(screen.queryByRole('link')).toBeNull()
    })
  })

  describe('when a Shop route supplies a screen layout and root attributes', () => {
    it('should apply them to the same element that contains the heading', () => {
      render(
        <ShopEmptyState
          title="Sign in"
          role="region"
          aria-label="Sell"
          data-testid="shop-gate"
          variant="screen"
        />
      )
      const region = screen.getByRole('region', { name: 'Sell' })
      expect(region).toHaveAttribute('data-testid', 'shop-gate')
      expect(region).toHaveAttribute('data-variant', 'screen')
      expect(region).toContainElement(screen.getByRole('heading', { name: 'Sign in' }))
    })
  })

  describe('when the action navigates to another page', () => {
    it('should expose its destination as a link', () => {
      render(
        <ShopEmptyState
          title="No listings"
          action={{ label: 'My items', href: '/shop/my-items' }}
        />
      )
      expect(screen.getByRole('link', { name: 'My items' })).toHaveAttribute(
        'href',
        '/shop/my-items'
      )
    })
  })

  describe('when the action changes the current page', () => {
    it('should invoke the action from the keyboard', async () => {
      const onClick = vi.fn()
      const user = userEvent.setup()
      render(<ShopEmptyState title="No matches" action={{ label: 'Clear filters', onClick }} />)
      await user.tab()
      await user.keyboard('{Enter}')
      expect(onClick).toHaveBeenCalledTimes(1)
    })
  })
})
