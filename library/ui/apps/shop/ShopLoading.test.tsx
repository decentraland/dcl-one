import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { ShopBadgeSkeleton, ShopSkeletonCards } from './ShopLoading'

describe('ShopSkeletonCards', () => {
  it('renders the requested number of decorative catalogue cards', () => {
    render(<ShopSkeletonCards count={3} />)
    const cards = screen.getAllByTestId('skeleton-card')
    expect(cards).toHaveLength(3)
    for (const card of cards) {
      expect(card).toHaveAttribute('aria-hidden', 'true')
      expect(card).toHaveClass('skeletonCatalogue')
    }
  })

  it.each([0, -1])('renders an empty grid for a count of %d', count => {
    const { container } = render(<ShopSkeletonCards count={count} />)
    expect(container).toBeEmptyDOMElement()
  })
})

describe('ShopBadgeSkeleton', () => {
  it('reserves a decorative avatar and name in the neutral tone', () => {
    render(<ShopBadgeSkeleton testId="collection-loading" />)
    const badge = screen.getByTestId('collection-loading')
    expect(badge).toHaveAttribute('aria-hidden', 'true')
    expect(badge.firstElementChild).toHaveClass('avatar', 'neutral')
    expect(badge.lastElementChild).toHaveClass('name', 'neutral')
  })
})
