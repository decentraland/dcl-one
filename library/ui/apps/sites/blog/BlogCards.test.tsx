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

  it('should show the card category as the medium category pill and the author picture beside the name', () => {
    const { container } = render(
      <BlogPostCard
        post={card({
          title: 'Grid article',
          category: { slug: 'community-highlights', title: 'Community Highlights' },
          author: { slug: 'jane-doe', name: 'Jane Doe', image: '/jane.png' }
        })}
      />
    )
    const pill = screen.getByText('Community Highlights').closest('[data-variant]')!
    expect(pill).toHaveAttribute('data-variant', 'category')
    expect(pill).toHaveAttribute('data-tone', 'communities')
    expect(pill).toHaveAttribute('data-size', 'md')
    const picture = container.querySelector('img[src="/jane.png"]')!
    expect(picture).toHaveAttribute('alt', '')
    expect(picture.closest('[data-size]')).toHaveAttribute('data-size', '28')
    expect(screen.getByText('Jane Doe')).toBeInTheDocument()
  })

  it('should show the related card category as the large brand pill', () => {
    render(<BlogPostCard variant="related" post={card({ title: 'Related article' })} />)
    const pill = screen.getByText('Platform').closest('[data-variant]')!
    expect(pill).toHaveAttribute('data-size', 'lg')
    expect(pill).toHaveAttribute('data-tone', 'violet')
  })

  it('should leave a blank gradient disc for an author without a picture', () => {
    const { container } = render(<BlogPostCard post={card({ title: 'No picture' })} />)
    const disc = container.querySelector('[data-fallback="gradient"]')!
    expect(disc).toHaveAttribute('data-size', '28')
    expect(disc.querySelector('img')).toBeNull()
    expect(disc.textContent).toBe('')
  })
})
