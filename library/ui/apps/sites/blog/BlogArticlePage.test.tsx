import { act, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { BlogArticlePage, type BlogArticlePageProps } from './BlogArticlePage'
import { BlogNotFound, BlogUnavailable } from './BlogNotFound'
import { article, image } from './fixtures.test-utils'

const categories = [{ slug: 'platform', title: 'Platform' }]
const URL_ = 'https://sites.test/blog/post'

const renderArticle = (over: Partial<BlogArticlePageProps> = {}) =>
  render(
    <BlogArticlePage
      post={article()}
      shareUrl={URL_}
      related={[]}
      categories={categories}
      {...over}
    />
  )

afterEach(() => {
  vi.unstubAllGlobals()
  vi.useRealTimers()
  vi.resetAllMocks()
})

describe('BlogArticlePage', () => {
  let over: Partial<BlogArticlePageProps>
  let container: HTMLElement

  describe('when the post is rendered with its defaults', () => {
    beforeEach(() => {
      renderArticle()
    })

    it('should render a single h1 with the post title', () => {
      expect(screen.getAllByRole('heading', { level: 1 }).map((h) => h.textContent)).toEqual([
        'A post'
      ])
    })

    it('should link back to the blog', () => {
      expect(screen.getByRole('link', { name: 'Back to blog' })).toHaveAttribute('href', '/blog')
    })

    it('should render the publication date as a machine-readable time', () => {
      expect(screen.getByText('Jul 01, 2026')).toHaveAttribute('datetime', '2026-07-01')
    })

    it('should render the read time', () => {
      expect(screen.getByText(/2 min read/)).toBeInTheDocument()
    })

    it("should link the byline to the author's page", () => {
      expect(screen.getByRole('link', { name: 'Jane Doe' })).toHaveAttribute(
        'href',
        '/blog/author/jane-doe'
      )
    })

    it('should render the lead', () => {
      expect(screen.getByText('About the post')).toBeInTheDocument()
    })

    it('should render the body', () => {
      expect(screen.getByText('Body text')).toBeInTheDocument()
    })

    it("should highlight the post's category tab as current", () => {
      expect(screen.getByRole('link', { name: 'Platform' })).toHaveAttribute('aria-current', 'true')
    })

    it('should share on X in a new tab with the real share URL', () => {
      const x = screen.getByRole('link', { name: /Share on X/ })
      expect(x.getAttribute('href')).toContain(encodeURIComponent(URL_))
      expect(x).toHaveAttribute('target', '_blank')
      expect(x).toHaveAttribute('rel', 'noopener noreferrer')
    })

    it('should share on LinkedIn with the real share URL', () => {
      expect(screen.getByRole('link', { name: /Share on LinkedIn/ }).getAttribute('href')).toBe(
        `https://www.linkedin.com/sharing/share-offsite/?url=${encodeURIComponent(URL_)}`
      )
    })

    it('should show no related section', () => {
      expect(screen.queryByRole('region')).not.toBeInTheDocument()
    })
  })

  describe('when the post category is absent from the menu', () => {
    beforeEach(() => {
      renderArticle({ categories: [{ slug: 'announcements', title: 'Announcements' }] })
    })

    it('should leave every category link unmarked', () => {
      const nav = screen.getByRole('navigation', { name: 'Blog categories' })
      expect(
        within(nav)
          .getAllByRole('link')
          .every((tab) => !tab.hasAttribute('aria-current'))
      ).toBe(true)
    })
  })

  describe('when the link is copied', () => {
    let writeText: ReturnType<typeof vi.fn>

    describe('and the clipboard accepts the write', () => {
      beforeEach(async () => {
        writeText = vi.fn().mockResolvedValueOnce(undefined)
        vi.stubGlobal('navigator', { clipboard: { writeText } })
        renderArticle()
        await userEvent.click(screen.getByRole('button', { name: 'Copy link' }))
      })

      it('should write the share URL to the clipboard', () => {
        expect(writeText).toHaveBeenCalledWith(URL_)
      })

      it('should announce that the link was copied', async () => {
        expect(await screen.findByRole('status')).toHaveTextContent('Link copied')
      })
    })

    describe('and the clipboard refuses the write', () => {
      beforeEach(async () => {
        writeText = vi.fn().mockRejectedValueOnce(new Error('denied'))
        vi.stubGlobal('navigator', { clipboard: { writeText } })
        renderArticle()
        await userEvent.click(screen.getByRole('button', { name: 'Copy link' }))
      })

      it('should report that the link could not be copied', async () => {
        expect(await screen.findByText('Could not copy the link')).toBeInTheDocument()
      })
    })

    describe('and there is no clipboard', () => {
      beforeEach(() => {
        vi.useFakeTimers()
        vi.stubGlobal('navigator', {})
        renderArticle()
        act(() => screen.getByRole('button', { name: 'Copy link' }).click())
      })

      it('should report that the link could not be copied', () => {
        expect(screen.getByRole('status')).toHaveTextContent('Could not copy the link')
      })

      describe('and the message has been shown for a while', () => {
        beforeEach(() => {
          act(() => vi.advanceTimersByTime(3000))
        })

        it('should clear the message', () => {
          expect(screen.getByRole('status')).toHaveTextContent('')
        })
      })
    })
  })

  describe('when every related post shares the category', () => {
    beforeEach(() => {
      over = {
        related: [
          {
            slug: 'b',
            title: 'B',
            publishedAt: '2026-06-01',
            category: categories[0]!,
            image: null,
            hue: 1
          }
        ]
      }
      renderArticle(over)
    })

    it('should name the related section after the category and link its posts', () => {
      const section = screen.getByRole('region', { name: 'More on Platform' })
      expect(within(section).getByRole('link', { name: 'B' })).toHaveAttribute(
        'href',
        '/blog/platform/b'
      )
    })
  })

  describe('when the related posts have mixed categories', () => {
    beforeEach(() => {
      over = {
        related: [
          {
            slug: 'b',
            title: 'B',
            publishedAt: '',
            category: { slug: 'x', title: 'X' },
            image: null,
            hue: 1
          }
        ]
      }
      renderArticle(over)
    })

    it('should use a neutral related heading', () => {
      expect(screen.getByRole('region', { name: 'More from the blog' })).toBeInTheDocument()
    })
  })

  describe('when the related posts could not be read', () => {
    beforeEach(() => {
      renderArticle({ related: null })
    })

    it('should show no related section', () => {
      expect(screen.queryByRole('region')).not.toBeInTheDocument()
    })

    it('should still render the article', () => {
      expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent('A post')
    })
  })

  describe('when the post has no author', () => {
    beforeEach(() => {
      renderArticle({ post: article({ author: { slug: '', name: '', image: '' } }) })
    })

    it('should omit the author', () => {
      expect(screen.queryByText('Jane Doe')).not.toBeInTheDocument()
    })
  })

  describe('when the author has no page', () => {
    beforeEach(() => {
      renderArticle({ post: article({ author: { slug: '', name: 'Decentraland', image: '' } }) })
    })

    it('should name the author', () => {
      expect(screen.getByText('Decentraland')).toBeInTheDocument()
    })

    it('should not link the author', () => {
      expect(screen.queryByRole('link', { name: 'Decentraland' })).not.toBeInTheDocument()
    })
  })

  describe('when the CMS has a picture of the author', () => {
    beforeEach(() => {
      ;({ container } = renderArticle({
        post: article({
          author: { slug: 'j', name: 'Jane Doe', image: 'https://images.example/j.png' }
        })
      }))
    })

    it("should show the author's picture decoratively", () => {
      const avatar = container.querySelector('img[src="https://images.example/j.png"]')
      expect(avatar).toHaveAttribute('alt', '')
    })

    it('should show it at 56px without a ring, on the brand gradient', () => {
      const avatar = container.querySelector('img[src="https://images.example/j.png"]')!
      const pic = avatar.closest('[data-size]')!
      expect(pic).toHaveAttribute('data-size', '56')
      expect(pic).toHaveAttribute('data-ring', 'none')
      expect(pic).toHaveAttribute('data-fallback', 'gradient')
    })
  })

  describe('when the CMS has no picture of the author', () => {
    beforeEach(() => {
      ;({ container } = renderArticle({
        post: article({ author: { slug: 'j', name: 'Jane Doe', image: '' } })
      }))
    })

    it("should show the author's initials on the gradient, hidden from assistive tech", () => {
      const disc = container.querySelector('[data-fallback="gradient"]')!
      expect(disc.querySelector('img')).toBeNull()
      expect(disc).toHaveTextContent('JD')
      expect(within(disc as HTMLElement).queryByRole('img')).toBeNull()
    })
  })

  describe('when the post category is shown', () => {
    it('should draw it as the large brand pill', () => {
      renderArticle()
      const pill = screen
        .getAllByText('Platform')
        .map((el) => el.closest('[data-variant="category"]'))
        .find(Boolean)!
      expect(pill).toHaveAttribute('data-size', 'lg')
      expect(pill).toHaveAttribute('data-tone', 'violet')
    })
  })

  describe('when the post has a picture', () => {
    let hero: HTMLElement

    beforeEach(() => {
      renderArticle({ post: article({ image: image('A plaza at night') }) })
      hero = screen.getByRole('img', { name: 'A plaza at night' })
    })

    it('should render the picture with its alt and source', () => {
      expect(hero).toHaveAttribute('src', 'https://images.example/hero.png')
    })

    it('should render the picture with its size', () => {
      expect(hero).toHaveAttribute('width', '1200')
      expect(hero).toHaveAttribute('height', '600')
    })

    it('should load the picture eagerly, above the fold', () => {
      expect(hero).toHaveAttribute('loading', 'eager')
    })
  })

  describe('when the post has no picture', () => {
    beforeEach(() => {
      ;({ container } = renderArticle({ post: article({ image: null, hue: 42 }) }))
    })

    it('should render no picture', () => {
      expect(container.querySelector('article img')).toBeNull()
    })

    it('should hide the hue gradient fallback from assistive tech', () => {
      expect(container.querySelector('[style*="--blog-hue"]')).toHaveAttribute(
        'aria-hidden',
        'true'
      )
    })

    it("should paint the fallback with the post's hue", () => {
      const cover = container.querySelector('[style*="--blog-hue"]') as HTMLElement
      expect(cover.style.getPropertyValue('--blog-hue')).toBe('42')
    })
  })
})

describe('BlogNotFound', () => {
  describe.each([
    ['post', 'Post not found'],
    ['category', 'Category not found'],
    ['author', 'Author not found']
  ] as const)('when a %s is missing', (kind, title) => {
    beforeEach(() => {
      render(<BlogNotFound kind={kind} />)
    })

    it(`should name the missing ${kind}`, () => {
      expect(screen.getByRole('heading', { level: 1, name: title })).toBeInTheDocument()
    })

    it('should link back to the blog', () => {
      expect(screen.getByRole('link', { name: 'Back to the Blog' })).toHaveAttribute(
        'href',
        '/blog'
      )
    })
  })
})

describe('BlogUnavailable', () => {
  let container: HTMLElement

  describe('when the blog is unavailable', () => {
    beforeEach(() => {
      ;({ container } = render(<BlogUnavailable />))
    })

    it('should say the blog is temporarily unavailable', () => {
      expect(
        screen.getByRole('heading', { level: 1, name: 'The blog is temporarily unavailable' })
      ).toBeInTheDocument()
    })

    it('should not give any detail of why', () => {
      expect(container.textContent).not.toMatch(/https?:|cms/i)
    })
  })
})
