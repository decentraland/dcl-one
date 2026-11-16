import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { MAP_API_TIMEOUT_MS, resolveMapApi, setMapApiTransport, setPlaceFavorite, setPlaceLike } from '../features/map/mapApi'

beforeEach(() => {
  vi.useFakeTimers()
})

afterEach(() => {
  setMapApiTransport(null)
  vi.useRealTimers()
})

describe('the map API transport', () => {
  it('delivers the reply to the request that waits for it', async () => {
    const send = vi.fn()
    setMapApiTransport(send)
    const like = setPlaceLike('plaza', true)
    const [id] = send.mock.calls[0] as [number]
    resolveMapApi({ kind: 'mapApiResult', id, ok: true, data: { data: { likes: 1, dislikes: 0, user_like: true, user_dislike: false } } })
    await expect(like).resolves.toMatchObject({ user_like: true })
  })

  it('fails a request the bridge never answers, and ignores a reply that comes after', async () => {
    const send = vi.fn()
    setMapApiTransport(send)
    const favorite = setPlaceFavorite('plaza', true)
    const failed = expect(favorite).rejects.toThrow('The map did not answer')
    vi.advanceTimersByTime(MAP_API_TIMEOUT_MS)
    await failed
    const [id] = send.mock.calls[0] as [number]
    expect(() => resolveMapApi({ kind: 'mapApiResult', id, ok: true, data: { data: { favorites: 1, user_favorite: true } } })).not.toThrow()
  })

  it('fails a request it could not send', async () => {
    setMapApiTransport(() => {
      throw new Error('the driver is gone')
    })
    await expect(setPlaceLike('plaza', null)).rejects.toThrow('the driver is gone')
  })

  it('fails what is waiting when the bridge goes away', async () => {
    setMapApiTransport(vi.fn())
    const like = setPlaceLike('plaza', false)
    setMapApiTransport(null)
    await expect(like).rejects.toThrow('The map lost its connection')
  })
})
