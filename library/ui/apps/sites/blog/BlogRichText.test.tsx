import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { BlogRichText } from './BlogRichText'

describe('BlogRichText', () => {
  it('renders paragraphs, quotes and lists', () => {
    render(
      <BlogRichText
        blocks={[
          { type: 'p', content: 'Para' },
          { type: 'quote', content: 'Said' },
          { type: 'ul', items: ['One', 'Two'] }
        ]}
      />
    )
    expect(screen.getByText('Para').tagName).toBe('P')
    expect(screen.getByText('Said').closest('blockquote')).not.toBeNull()
    expect(screen.getAllByRole('listitem').map((li) => li.textContent)).toEqual(['One', 'Two'])
  })

  it('shifts headings so the highest body level renders as h2', () => {
    render(
      <BlogRichText
        blocks={[
          { type: 'h3', content: 'Top' },
          { type: 'h4', content: 'Sub' }
        ]}
      />
    )
    expect(screen.getByRole('heading', { level: 2, name: 'Top' })).toBeInTheDocument()
    expect(screen.getByRole('heading', { level: 3, name: 'Sub' })).toBeInTheDocument()
  })

  it('keeps h2-based bodies as they are and renders nothing for an empty body', () => {
    const { container, unmount } = render(
      <BlogRichText
        blocks={[
          { type: 'h2', content: 'A' },
          { type: 'h4', content: 'C' }
        ]}
      />
    )
    expect(screen.getByRole('heading', { level: 2, name: 'A' })).toBeInTheDocument()
    expect(screen.getByRole('heading', { level: 4, name: 'C' })).toBeInTheDocument()
    unmount()
    expect(container.textContent).toBe('')
    const empty = render(<BlogRichText blocks={[]} />)
    expect(empty.container.textContent).toBe('')
  })
})
