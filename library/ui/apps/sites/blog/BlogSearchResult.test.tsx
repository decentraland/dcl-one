import { render } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
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
  it('highlights the matched words in the title and the description', () => {
    const { container } = render(<BlogSearchResult post={post()} query="decen" />)
    expect([...container.querySelectorAll('mark')].map((m) => m.textContent)).toEqual([
      'Decentraland',
      'Decentraland'
    ])
    expect(container.textContent).toContain('Decentraland Worlds: Your Place Online')
  })

  it('is not interactive and keeps its picture out of the accessible name', () => {
    const { container } = render(<BlogSearchResult post={post()} query="decen" />)
    expect(container.querySelector('a, button, [tabindex]')).toBeNull()
    expect(container.querySelector('img')?.getAttribute('alt')).toBe('')
  })

  it('paints the hue placeholder without a picture and omits an empty description', () => {
    const { container } = render(
      <BlogSearchResult post={post({ image: null, description: '' })} query="zzz" />
    )
    expect(container.querySelector('img')).toBeNull()
    expect(container.querySelector('mark')).toBeNull()
    expect(container.textContent).toBe('Decentraland Worlds: Your Place Online')
  })
})
