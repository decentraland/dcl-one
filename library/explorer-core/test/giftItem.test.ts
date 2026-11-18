// A gift notification's item (features/gifting/giftItem): read from the token's metadata document, as the
// gift-received popup shows it.
import { describe, expect, it, vi } from 'vitest'
import { loadGiftItem, parseGiftItem } from '../features/gifting/giftItem'

const DOC = {
  name: 'Cool Hat',
  description: 'A hat',
  image: 'https://example.com/image.png',
  thumbnail: 'https://example.com/thumb.png',
  attributes: [
    { trait_type: 'Rarity', value: 'Epic' },
    { trait_type: 'Category', value: 'Hat' }
  ]
}

const json = (body: unknown, status = 200): Response => new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } })

describe('parseGiftItem', () => {
  it('reads the name, the thumbnail and the rarity and category attributes, lowercased', () => {
    expect(parseGiftItem(DOC)).toEqual({ name: 'Cool Hat', description: 'A hat', imageUrl: 'https://example.com/thumb.png', rarity: 'epic', category: 'hat', kind: 'wearable' })
  })

  it('falls back to the image, a common rarity and the wearable category', () => {
    expect(parseGiftItem({ name: 'X', image: 'https://example.com/i.png' })).toMatchObject({ imageUrl: 'https://example.com/i.png', rarity: 'common', category: 'wearable', kind: 'wearable' })
  })

  it('tells an emote by its category', () => {
    expect(parseGiftItem({ ...DOC, attributes: [{ trait_type: 'category', value: 'Dance' }] })?.kind).toBe('emote')
  })

  it('reads nothing from something that is not a document', () => {
    expect(parseGiftItem(null)).toBeNull()
    expect(parseGiftItem([DOC])).toBeNull()
  })
})

describe('loadGiftItem', () => {
  it('fetches the token uri and parses it', async () => {
    const fetchImpl = vi.fn(async () => json(DOC))
    await expect(loadGiftItem('https://peer.example.com/erc721/137/0xabc/1/2', fetchImpl as unknown as typeof fetch)).resolves.toMatchObject({ name: 'Cool Hat' })
    expect(fetchImpl).toHaveBeenCalledWith('https://peer.example.com/erc721/137/0xabc/1/2', expect.anything())
  })

  it('reads nothing without a uri, from a non-https one, or when the fetch fails', async () => {
    const fetchImpl = vi.fn(async () => json({ error: 'no' }, 404))
    await expect(loadGiftItem(undefined, fetchImpl as unknown as typeof fetch)).resolves.toBeNull()
    await expect(loadGiftItem('http://peer.example.com/x', fetchImpl as unknown as typeof fetch)).resolves.toBeNull()
    await expect(loadGiftItem('not a url', fetchImpl as unknown as typeof fetch)).resolves.toBeNull()
    expect(fetchImpl).not.toHaveBeenCalled()
    await expect(loadGiftItem('https://peer.example.com/x', fetchImpl as unknown as typeof fetch)).resolves.toBeNull()
    const throwing = vi.fn(async () => {
      throw new Error('offline')
    })
    await expect(loadGiftItem('https://peer.example.com/x', throwing as unknown as typeof fetch)).resolves.toBeNull()
  })
})
