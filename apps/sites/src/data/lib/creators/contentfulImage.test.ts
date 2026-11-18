import { describe, expect, it } from 'vitest'
import { contentfulImage } from './contentfulImage'

const ASSET = 'https://images.ctfassets.net/space/asset/hash/Image_1.png'

describe('contentfulImage', () => {
  it('requests a WebP at twice the rendered width', () => {
    const url = new URL(contentfulImage(ASSET, { width: 380 }))
    expect(url.origin + url.pathname).toBe(ASSET)
    expect(url.searchParams.get('w')).toBe('760')
    expect(url.searchParams.get('fm')).toBe('webp')
    expect(url.searchParams.get('q')).toBe('80')
  })

  it('keeps parameters the URL already carries', () => {
    const url = new URL(contentfulImage(`${ASSET}?fit=fill`, { width: 100, quality: 60 }))
    expect(url.searchParams.get('fit')).toBe('fill')
    expect(url.searchParams.get('q')).toBe('60')
  })

  it('leaves images from other hosts untouched', () => {
    const peer = 'https://peer.decentraland.org/content/contents/bafy'
    expect(contentfulImage(peer, { width: 300 })).toBe(peer)
    expect(contentfulImage('/assets/logo.svg', { width: 40 })).toBe('/assets/logo.svg')
  })
})
