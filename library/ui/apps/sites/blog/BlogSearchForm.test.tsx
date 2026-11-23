import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from 'vitest'
import { BlogSearchForm } from './BlogSearchForm'
import type { BlogSearchSuggestion, BlogSearchSuggestions } from './blogModel'

type PreviewState = { loading: boolean; failed: boolean; suggestions: BlogSearchSuggestions | null }

describe('BlogSearchForm', () => {
  let onQueryChange: Mock<(query: string) => void>
  let user: ReturnType<typeof userEvent.setup>

  beforeEach(() => {
    onQueryChange = vi.fn()
    user = userEvent.setup()
  })

  afterEach(() => {
    vi.resetAllMocks()
  })

  describe('when the desktop Blog search provides suggestions', () => {
    let post: BlogSearchSuggestion
    let input: HTMLElement

    async function renderSearch(total: number) {
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
      const field = screen.getByRole('combobox', { name: 'Search articles' })
      await userEvent.setup().type(field, 'dece')
      return field
    }

    beforeEach(() => {
      post = {
        slug: 'update',
        href: '/blog/news/update',
        title: 'Decentraland update',
        description: 'News from Decentraland',
        image: null,
        hue: 0
      }
    })

    describe('and there are more matches than fit in the preview', () => {
      beforeEach(async () => {
        input = await renderSearch(5)
      })

      describe('and the highlighted article is activated', () => {
        let article: HTMLElement
        let clicked: Mock<(event: Event) => void>

        beforeEach(() => {
          input.focus()
          article = screen.getByRole('option', { name: /Decentraland update/ })
          clicked = vi.fn((event: Event) => event.preventDefault())
          article.addEventListener('click', clicked)
        })

        it('should link the article option to the article', () => {
          expect(article).toHaveAttribute('href', '/blog/news/update')
        })

        it('should highlight the typed query in the article option', () => {
          expect(article.querySelector('mark')).toHaveTextContent('Dece')
        })

        it('should activate the highlighted article and dismiss the preview', async () => {
          await user.keyboard('{ArrowDown}{Enter}')
          expect(clicked).toHaveBeenCalledOnce()
          expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
        })
      })

      describe('and the final option is activated', () => {
        let more: HTMLElement
        let clicked: Mock<(event: Event) => void>

        beforeEach(() => {
          input.focus()
          more = screen.getByRole('option', { name: 'See more results' })
          clicked = vi.fn((event: Event) => event.preventDefault())
          more.addEventListener('click', clicked)
        })

        it('should link to all results with the typed query', () => {
          expect(more).toHaveAttribute('href', '/blog/search?q=dece')
        })

        it('should navigate to all results', async () => {
          await user.keyboard('{ArrowUp}{Enter}')
          expect(clicked).toHaveBeenCalledOnce()
        })
      })

      describe('and Escape is pressed', () => {
        beforeEach(async () => {
          input.focus()
          await user.keyboard('{Escape}')
        })

        it('should close the preview', () => {
          expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
        })

        it('should keep the entered query', () => {
          expect(input).toHaveValue('dece')
        })

        it('should keep the focus on the field', () => {
          expect(input).toHaveFocus()
        })
      })

      describe('and Arrow Down is pressed after Escape', () => {
        beforeEach(async () => {
          await user.keyboard('{Escape}{ArrowDown}')
        })

        it('should reopen the preview', () => {
          expect(input).toHaveAttribute('aria-expanded', 'true')
        })

        it('should select the first article', () => {
          expect(screen.getByRole('option', { selected: true })).toHaveAccessibleName(
            /Decentraland update/
          )
        })

        it('should not edit the field', () => {
          expect(onQueryChange).toHaveBeenLastCalledWith('dece')
        })
      })

      describe('and the search is cleared', () => {
        beforeEach(async () => {
          await user.click(screen.getByRole('button', { name: 'Clear search' }))
        })

        it('should clear the query', () => {
          expect(input).toHaveValue('')
        })

        it('should return focus to the field', () => {
          expect(input).toHaveFocus()
        })

        it('should close the preview', () => {
          expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
        })

        it('should report the empty query', () => {
          expect(onQueryChange).toHaveBeenLastCalledWith('')
        })
      })

      describe('and Tab is pressed', () => {
        beforeEach(async () => {
          input.focus()
          await user.tab()
        })

        it('should move focus to the clear button', () => {
          expect(screen.getByRole('button', { name: 'Clear search' })).toHaveFocus()
        })

        describe('and Tab is pressed again', () => {
          beforeEach(async () => {
            await user.tab()
          })

          it('should leave the field and close the preview', () => {
            expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
          })
        })
      })

      describe('and the input changes to a new query', () => {
        beforeEach(async () => {
          await user.clear(input)
          await user.type(input, 'land')
        })

        it('should hide matches for the earlier input immediately', () => {
          expect(screen.queryByRole('option')).not.toBeInTheDocument()
        })

        it('should report the new query', () => {
          expect(onQueryChange).toHaveBeenLastCalledWith('land')
        })
      })
    })

    describe('and all matches fit in the preview', () => {
      beforeEach(async () => {
        await renderSearch(1)
      })

      it('should omit See more results', () => {
        expect(screen.queryByRole('option', { name: 'See more results' })).not.toBeInTheDocument()
      })
    })
  })

  describe.each<[string, PreviewState, string]>([
    ['searching', { loading: true, failed: false, suggestions: null }, 'Searching…'],
    [
      'failing',
      { loading: false, failed: true, suggestions: null },
      'Search suggestions are unavailable.'
    ],
    [
      'finding no matches',
      { loading: false, failed: false, suggestions: { query: 'dece', total: 0, posts: [] } },
      'No matching articles.'
    ]
  ])('when the desktop Blog search is %s', (_, state, message) => {
    let status: HTMLElement
    let input: HTMLElement

    beforeEach(() => {
      render(<BlogSearchForm variant="bar" query="" preview={{ onQueryChange, ...state }} />)
      status = screen.getByRole('status')
      input = screen.getByRole('combobox')
    })

    it('should start with an empty status', () => {
      expect(status).toBeEmptyDOMElement()
    })

    describe('and a query is typed', () => {
      beforeEach(async () => {
        await user.type(input, 'dece')
      })

      it('should announce the current search status', () => {
        expect(status).toHaveTextContent(message)
      })

      it('should keep the preview collapsed', () => {
        expect(input).toHaveAttribute('aria-expanded', 'false')
      })

      it('should show no options', () => {
        expect(screen.queryByRole('option')).not.toBeInTheDocument()
      })
    })
  })
})
