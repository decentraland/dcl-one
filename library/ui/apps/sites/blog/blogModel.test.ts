import { beforeEach, describe, expect, it } from 'vitest'
import {
  blogAuthorHref,
  blogBadgeTone,
  blogCardBadgeTone,
  blogCategoryHref,
  blogInitials,
  blogPostHref,
  blogShareLinks,
  formatBlogDate
} from './blogModel'

describe('formatBlogDate', () => {
  describe('when the date is a valid ISO date', () => {
    it('should format it as "Mon DD, YYYY"', () => {
      expect(formatBlogDate('2026-07-02')).toBe('Jul 02, 2026')
    })
  })

  describe('when the date is empty, malformed or out of range', () => {
    it.each(['', 'JUL 2, 2026', '2026-13-01', '2026-00-01'])(
      'should return an empty string for %j',
      (date) => {
        expect(formatBlogDate(date)).toBe('')
      }
    )
  })
})

describe('links', () => {
  describe('when building a post path', () => {
    it('should encode each segment of a decentraland/sites-shaped path', () => {
      expect(blogPostHref('platform', 'a b')).toBe('/blog/platform/a%20b')
      expect(blogPostHref('x/y', 'p?q')).toBe('/blog/x%2Fy/p%3Fq')
    })
  })

  describe('when building a category path', () => {
    describe('and the category is empty', () => {
      it('should return the blog root', () => {
        expect(blogCategoryHref('')).toBe('/blog')
      })
    })

    describe('and the category is set', () => {
      it('should append the category to the blog root', () => {
        expect(blogCategoryHref('platform')).toBe('/blog/platform')
      })
    })
  })

  describe('when building an author path', () => {
    it('should encode the author name', () => {
      expect(blogAuthorHref('jane doe')).toBe('/blog/author/jane%20doe')
    })
  })

  describe('when building share links', () => {
    let links: ReturnType<typeof blogShareLinks>

    beforeEach(() => {
      links = blogShareLinks('A & B', 'https://example.org/blog/a?b=1')
    })

    it('should build an encoded X share URL', () => {
      expect(links.x).toBe(
        'https://x.com/intent/post?text=A%20%26%20B&url=https%3A%2F%2Fexample.org%2Fblog%2Fa%3Fb%3D1'
      )
    })

    it('should build an encoded LinkedIn share URL', () => {
      expect(links.linkedin).toBe(
        'https://www.linkedin.com/sharing/share-offsite/?url=https%3A%2F%2Fexample.org%2Fblog%2Fa%3Fb%3D1'
      )
    })
  })
})

describe('badges and initials', () => {
  describe('when mapping a category to a badge tone', () => {
    describe('and the category is known', () => {
      it('should return its tone', () => {
        expect(blogBadgeTone('platform')).toBe('violet')
        expect(blogBadgeTone('announcements')).toBe('ruby')
      })
    })

    describe('and the category is unknown', () => {
      it('should fall back to ruby', () => {
        expect(blogBadgeTone('unknown')).toBe('ruby')
      })
    })
  })

  describe('when mapping a live CMS category to a card badge tone', () => {
    describe('and the category is known', () => {
      it('should return the design tone used on cards', () => {
        expect(blogCardBadgeTone('announcements')).toBe('updates')
        expect(blogCardBadgeTone('community-highlights')).toBe('communities')
        expect(blogCardBadgeTone('about-decentraland')).toBe('about')
      })
    })

    describe('and the category is unknown', () => {
      it('should fall back to about', () => {
        expect(blogCardBadgeTone('unknown')).toBe('about')
      })
    })
  })

  describe('when taking initials from a name', () => {
    describe('and the name has words', () => {
      it('should take up to two initials', () => {
        expect(blogInitials('jane  doe smith')).toBe('JD')
        expect(blogInitials('Catalyrst')).toBe('C')
      })
    })

    describe('and the name is blank', () => {
      it('should return an empty string', () => {
        expect(blogInitials('   ')).toBe('')
      })
    })
  })
})
