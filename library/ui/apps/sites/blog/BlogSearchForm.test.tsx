import { fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { BlogSearchForm } from './BlogSearchForm'

describe('when the desktop Blog search provides suggestions', () => {
  const post = {
    slug: 'update',
    href: '/blog/news/update',
    title: 'Decentraland update',
    description: 'News from Decentraland',
    image: null,
    hue: 0
  }
  const onQueryChange = vi.fn()

  beforeEach(() => {
    onQueryChange.mockClear()
  })

  function renderSearch(total = 5) {
    render(
      <BlogSearchForm
        variant="bar"
        query=""
        preview={{
          onQueryChange,
          loading: false,
          failed: false,
          suggestions: { query: 'dece', total, posts: [post] }
        }}
      />
    )
    const input = screen.getByRole('combobox', { name: 'Search articles' })
    fireEvent.change(input, { target: { value: 'dece' } })
    return input
  }

  it('should activate the highlighted article and dismiss the preview', async () => {
    const user = userEvent.setup()
    const input = renderSearch()
    input.focus()
    const article = screen.getByRole('option', { name: /Decentraland update/ })
    expect(article).toHaveAttribute('href', '/blog/news/update')
    expect(article.querySelector('mark')).toHaveTextContent('Dece')
    const clicked = vi.fn((event: Event) => event.preventDefault())
    article.addEventListener('click', clicked)
    await user.keyboard('{ArrowDown}{Enter}')
    expect(clicked).toHaveBeenCalledOnce()
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
  })

  it('should navigate to all results with the typed query from the final option', async () => {
    const user = userEvent.setup()
    const input = renderSearch()
    input.focus()
    const more = screen.getByRole('option', { name: 'See more results' })
    expect(more).toHaveAttribute('href', '/blog/search?q=dece')
    const clicked = vi.fn((event: Event) => event.preventDefault())
    more.addEventListener('click', clicked)
    await user.keyboard('{ArrowUp}{Enter}')
    expect(clicked).toHaveBeenCalledOnce()
  })

  it('should close on Escape and keep the entered query and focus', async () => {
    const user = userEvent.setup()
    const input = renderSearch()
    input.focus()
    await user.keyboard('{Escape}')
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
    expect(input).toHaveValue('dece')
    expect(input).toHaveFocus()
  })

  it('should clear the query and return focus to the field', async () => {
    const user = userEvent.setup()
    const input = renderSearch()
    await user.click(screen.getByRole('button', { name: 'Clear search' }))
    expect(input).toHaveValue('')
    expect(input).toHaveFocus()
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
    expect(onQueryChange).toHaveBeenLastCalledWith('')
  })

  it('should allow Tab to leave the field and close the preview', async () => {
    const user = userEvent.setup()
    const input = renderSearch()
    input.focus()
    await user.tab()
    expect(screen.getByRole('button', { name: 'Clear search' })).toHaveFocus()
    await user.tab()
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
  })

  it.each([
    [{ loading: true, failed: false, suggestions: null }, 'Searching…'],
    [{ loading: false, failed: true, suggestions: null }, 'Search suggestions are unavailable.'],
    [
      { loading: false, failed: false, suggestions: { query: 'dece', total: 0, posts: [] } },
      'No matching articles.'
    ]
  ] as const)('should announce the current search status', (state, message) => {
    render(<BlogSearchForm variant="bar" query="" preview={{ onQueryChange, ...state }} />)
    fireEvent.change(screen.getByRole('combobox'), { target: { value: 'dece' } })
    expect(screen.getByRole('status')).toHaveTextContent(message)
    expect(screen.queryByRole('option')).not.toBeInTheDocument()
  })

  it('should hide matches for an earlier input immediately', () => {
    const input = renderSearch()
    fireEvent.change(input, { target: { value: 'land' } })
    expect(screen.queryByRole('option')).not.toBeInTheDocument()
    expect(onQueryChange).toHaveBeenLastCalledWith('land')
  })

  it('should omit See more results if all four matches fit in the preview', () => {
    renderSearch(1)
    expect(screen.queryByRole('option', { name: 'See more results' })).not.toBeInTheDocument()
  })
})
