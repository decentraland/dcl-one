import { afterEach, describe, expect, it } from 'vitest'
import { canonicalUrl, setCanonical } from './seo'

afterEach(() => {
  document.head.innerHTML = ''
})

describe('canonicalUrl', () => {
  it('points the overview at the production creator home', () => {
    expect(canonicalUrl('')).toBe('https://decentraland.org/create')
    expect(canonicalUrl('/')).toBe('https://decentraland.org/create')
  })

  it('keeps the in-app path, without a trailing slash', () => {
    expect(canonicalUrl('/collections/')).toBe('https://decentraland.org/create/collections')
    expect(canonicalUrl('curation')).toBe('https://decentraland.org/create/curation')
  })
})

describe('setCanonical', () => {
  it('updates the canonical link and og:url of the page', () => {
    document.head.innerHTML =
      '<link rel="canonical" href="https://decentraland.org/create"><meta property="og:url" content="x">'
    setCanonical('/collections')
    expect(document.querySelector('link[rel="canonical"]')).toHaveAttribute(
      'href',
      'https://decentraland.org/create/collections'
    )
    expect(document.querySelector('meta[property="og:url"]')).toHaveAttribute(
      'content',
      'https://decentraland.org/create/collections'
    )
  })
})
