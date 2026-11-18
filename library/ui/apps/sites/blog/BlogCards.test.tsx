import { render, screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { BlogFeaturedCard } from './BlogFeaturedCard'
import { BlogPostCard } from './BlogPostCard'
import { card } from './fixtures.test-utils'

describe('when Sites renders the shared Blog cards', () => {
  it('should use the shared banner with a single link for the featured article', () => {
    render(
      <BlogFeaturedCard post={card({ title: 'Featured article', slug: 'featured', image: null })} />
    )
    const banner = screen.getByRole('article')
    expect(within(banner).getAllByRole('link')).toHaveLength(1)
    expect(within(banner).getByRole('link', { name: 'Featured article' })).toHaveAttribute(
      'href',
      '/blog/platform/featured'
    )
  })

  it('should use the shared standard card and leave the author row out of the related variant', () => {
    render(
      <BlogPostCard variant="related" post={card({ title: 'Related article', publishedAt: '' })} />
    )
    expect(screen.getByRole('link', { name: 'Related article' })).toBeInTheDocument()
    expect(screen.queryByRole('separator')).not.toBeInTheDocument()
  })
})
