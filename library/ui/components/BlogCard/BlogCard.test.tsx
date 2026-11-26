import { render, screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { BlogCard } from './BlogCard'

describe('when the BlogCard receives an author credit', () => {
  it('should render the byline and one native article link', () => {
    render(<BlogCard title="News" href="/blog/news/latest" author="Decentraland Team" />)
    const card = screen.getByRole('article')
    const heading = within(card).getByRole('heading', { level: 3, name: 'News' })
    expect(within(heading).getByRole('link')).toHaveAttribute('href', '/blog/news/latest')
    expect(within(card).getByText('Decentraland Team')).toBeVisible()
  })
})

describe('when the BlogCard is loading', () => {
  it('should be one busy progressbar named Loading, with no link and no text of a post', () => {
    render(<BlogCard loading />)
    const card = screen.getByRole('progressbar', { name: 'Loading' })
    expect(card).toHaveAttribute('aria-busy', 'true')
    expect(within(card).queryByRole('link')).toBeNull()
    expect(card).toHaveTextContent('')
  })

  it('should take the name the caller gives it, in the related variant too', () => {
    render(<BlogCard variant="related" loading loadingLabel="Loading a related post" />)
    expect(screen.getByRole('progressbar', { name: 'Loading a related post' })).toHaveAttribute(
      'aria-busy',
      'true'
    )
  })

  it('should leave the loaded card unbusy and named by its link', () => {
    render(<BlogCard title="News" href="/blog/news/latest" />)
    expect(screen.queryByRole('progressbar')).toBeNull()
    expect(screen.getByRole('article')).not.toHaveAttribute('aria-busy')
  })
})
