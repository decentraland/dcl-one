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
