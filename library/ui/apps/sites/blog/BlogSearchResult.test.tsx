import { render } from '@testing-library/react'
import { beforeEach, describe, expect, it } from 'vitest'
import { BlogSearchResult } from './BlogSearchResult'
import type { BlogSearchSuggestion } from './blogModel'

const post = (over: Partial<BlogSearchSuggestion> = {}): BlogSearchSuggestion => ({
  slug: 'worlds',
  href: '/blog/platform/worlds',
  title: 'Decentraland Worlds: Your Place Online',
  description: 'Here is how Decentraland Worlds work',
  image: { url: 'https://img.example/w.png', width: 80, height: 80, alt: 'Worlds' },
  hue: 120,
  ...over
})

describe('BlogSearchResult', () => {
  let container: HTMLElement

  describe('when the query matches words in the title and the description', () => {
    beforeEach(() => {
      ;({ container } = render(<BlogSearchResult post={post()} query="decen" />))
    })

    it('should highlight the matched words in the title and the description', () => {
      expect([...container.querySelectorAll('mark')].map((m) => m.textContent)).toEqual([
        'Decentraland',
        'Decentraland'
      ])
    })

    it('should keep the full title text', () => {
      expect(container.textContent).toContain('Decentraland Worlds: Your Place Online')
    })

    it('should not be interactive', () => {
      expect(container.querySelector('a, button, [tabindex]')).toBeNull()
    })

    it('should keep its picture out of the accessible name', () => {
      expect(container.querySelector('img')?.getAttribute('alt')).toBe('')
    })
  })

  describe('when the post has no picture and an empty description', () => {
    beforeEach(() => {
      ;({ container } = render(
        <BlogSearchResult post={post({ image: null, description: '' })} query="zzz" />
      ))
    })

    it('should paint the hue placeholder instead of a picture', () => {
      expect(container.querySelector('img')).toBeNull()
    })

    it('should mark nothing for a query that does not match', () => {
      expect(container.querySelector('mark')).toBeNull()
    })

    it('should omit the empty description', () => {
      expect(container.textContent).toBe('Decentraland Worlds: Your Place Online')
    })
  })
})
