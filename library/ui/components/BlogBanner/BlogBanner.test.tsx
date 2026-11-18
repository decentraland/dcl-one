import { render, screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { BlogBanner } from './BlogBanner'

describe('when the BlogBanner has no cover or metadata', () => {
  it('should expose one native article link named by its title', () => {
    render(
      <BlogBanner title="Latest article" href="/blog/news/latest" actionLabel="Read article" />
    )
    const banner = screen.getByRole('article')
    const heading = within(banner).getByRole('heading', { level: 2, name: 'Latest article' })
    expect(within(banner).getAllByRole('link')).toHaveLength(1)
    expect(within(heading).getByRole('link')).toHaveAttribute('href', '/blog/news/latest')
  })
})
