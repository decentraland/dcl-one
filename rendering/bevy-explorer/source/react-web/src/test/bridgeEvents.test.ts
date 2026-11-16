import { describe, it, expect, vi, beforeEach } from 'vitest'
import type { Ctx } from '../../bridge-scene/src/bridge'
import type { PageToScene, SceneToPage } from '@explorer-core/engine/protocol'

const { signed } = vi.hoisted(() => ({ signed: vi.fn() }))
vi.mock('../../bridge-scene/src/http', () => ({ serviceBase: async (name: string) => `https://${name}.example.org`, signed }))

const { registerEvents } = await import('../../bridge-scene/src/domains/events')

function fakeCtx(): { sent: SceneToPage[]; req: (msg: PageToScene) => Promise<void> } {
  const sent: SceneToPage[] = []
  const handlers = new Map<string, Array<(m: PageToScene) => unknown>>()
  const ctx: Ctx = {
    send: (m) => sent.push(m),
    on: (kind, h) => handlers.set(kind, [...(handlers.get(kind) ?? []), h as (m: PageToScene) => unknown]),
    push: () => {}
  }
  registerEvents(ctx)
  return {
    sent,
    req: async (msg) => {
      await Promise.all((handlers.get(msg.kind) ?? []).map((h) => h(msg)))
    }
  }
}

beforeEach(() => {
  signed.mockReset()
  vi.spyOn(console, 'error').mockImplementation(() => {})
})

describe('bridge events', () => {
  it('reads a date range with who is there now, and fills what the service leaves out', async () => {
    const h = fakeCtx()
    signed.mockResolvedValueOnce([{ id: 'a', name: 'Show', attending: true, live: true, connected_addresses: ['0x1'] }, { id: 'b' }])
    await h.req({ kind: 'getEvents', request: 7, from: '2026-09-07T22:00:00Z', to: '2026-09-12T21:59:59Z' })
    expect(signed).toHaveBeenCalledWith('https://events.example.org/api/events?from=2026-09-07T22%3A00%3A00Z&to=2026-09-12T21%3A59%3A59Z&with_connected_users=true')
    expect(h.sent).toHaveLength(1)
    const answer = h.sent[0]
    if (answer.kind !== 'events') throw new Error('expected an events answer')
    expect(answer.request).toBe(7)
    expect(answer.events[0]).toMatchObject({ id: 'a', name: 'Show', attending: true, live: true, connected_addresses: ['0x1'] })
    expect(answer.events[1]).toMatchObject({ id: 'b', name: '', attending: false, live: false, connected_addresses: [], recurrent_dates: [], place_id: null })
  })

  it('reads the highlighted and the live lists', async () => {
    const h = fakeCtx()
    signed.mockResolvedValue([])
    await h.req({ kind: 'getEvents', request: 1, list: 'highlight', limit: 10, offset: 0 })
    await h.req({ kind: 'getEvents', request: 2, list: 'live' })
    expect(signed.mock.calls.map((c) => c[0])).toEqual([
      'https://events.example.org/api/events?limit=10&offset=0&list=highlight&with_connected_users=true',
      'https://events.example.org/api/events?list=live&with_connected_users=true'
    ])
  })

  it('answers a failed read, and one with no list, as failed', async () => {
    const h = fakeCtx()
    signed.mockRejectedValueOnce(new Error('HTTP 503'))
    await h.req({ kind: 'getEvents', request: 3, list: 'live' })
    signed.mockResolvedValueOnce({ ok: false })
    await h.req({ kind: 'getEvents', request: 4, list: 'live' })
    expect(h.sent).toEqual([
      { kind: 'events', request: 3, events: [], failed: true },
      { kind: 'events', request: 4, events: [], failed: true }
    ])
  })

  it('sets the reminder with a POST and clears it with a DELETE, and reports a refusal', async () => {
    const h = fakeCtx()
    signed.mockResolvedValue({ ok: true })
    await h.req({ kind: 'setEventAttending', eventId: 'a b', attending: true })
    await h.req({ kind: 'setEventAttending', eventId: 'a b', attending: false })
    expect(signed.mock.calls).toEqual([
      ['https://events.example.org/api/events/a%20b/attendees', 'POST'],
      ['https://events.example.org/api/events/a%20b/attendees', 'DELETE']
    ])
    signed.mockRejectedValueOnce(new Error('HTTP 404'))
    await h.req({ kind: 'setEventAttending', eventId: 'a b', attending: true })
    expect(h.sent.at(-1)).toEqual({ kind: 'eventAttending', eventId: 'a b', attending: true, failed: true })
  })

  it('reads the places by id, and asks nothing for none', async () => {
    const h = fakeCtx()
    signed.mockResolvedValueOnce([{ id: 'p1', title: 'Plaza', base_position: '0,0' }])
    await h.req({ kind: 'getEventPlaces', request: 5, placeIds: ['p1'] })
    expect(signed).toHaveBeenCalledWith('https://places.example.org/api/destinations', 'POST', ['p1'])
    expect(h.sent[0]).toEqual({ kind: 'eventPlaces', request: 5, places: [{ id: 'p1', title: 'Plaza', world_name: null, base_position: '0,0' }] })
    await h.req({ kind: 'getEventPlaces', request: 6, placeIds: [] })
    expect(signed).toHaveBeenCalledTimes(1)
    expect(h.sent[1]).toEqual({ kind: 'eventPlaces', request: 6, places: [] })
  })

  it('reads the places 100 ids at a time, and skips a page answered with no body', async () => {
    const h = fakeCtx()
    const ids = Array.from({ length: 150 }, (_, i) => `p${i}`)
    signed.mockResolvedValueOnce(undefined).mockResolvedValueOnce([{ id: 'p120', title: 'Hall', world_name: 'hall.dcl.eth' }])
    await h.req({ kind: 'getEventPlaces', request: 8, placeIds: ids })
    expect(signed.mock.calls.map((c) => (c[2] as string[]).length)).toEqual([100, 50])
    expect(h.sent[0]).toEqual({ kind: 'eventPlaces', request: 8, places: [{ id: 'p120', title: 'Hall', world_name: 'hall.dcl.eth', base_position: null }] })
  })
})
