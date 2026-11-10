import { describe, it, expect, vi, beforeEach } from 'vitest'
import type { Ctx } from '../../bridge-scene/src/bridge'
import type { PageToScene, SceneToPage } from '@hud/engine/protocol'

let player: { userId: string; isGuest: boolean } | null = null
vi.mock('../../bridge-scene/node_modules/@dcl/sdk/players/index.js', () => ({ getPlayer: () => player }))
const { signed } = vi.hoisted(() => ({ signed: vi.fn() }))
vi.mock('../../bridge-scene/src/http', () => ({ serviceBase: async () => 'https://notifications.decentraland.org', signed }))

const { registerNotifications, POLL_INTERVAL_S, POLL_BACKOFF_MAX_S } = await import('../../bridge-scene/src/domains/notifications')
const { identity } = await import('../../bridge-scene/src/identity')

const raw = (id: string, timestamp: string, read = false) => ({ id, type: 'item_sold', timestamp, read, metadata: {} })

function fakeCtx(): { sent: SceneToPage[]; req: (msg: PageToScene) => Promise<void>; tick: (dt: number) => Promise<void> } {
  const sent: SceneToPage[] = []
  const handlers = new Map<string, Array<(m: PageToScene) => unknown>>()
  const pushed: Array<(dt: number) => void> = []
  const ctx: Ctx = {
    send: (m) => sent.push(m),
    on: (kind, h) => handlers.set(kind, [...(handlers.get(kind) ?? []), h as (m: PageToScene) => unknown]),
    push: (s) => pushed.push(s)
  }
  registerNotifications(ctx)
  return {
    sent,
    req: async (msg) => {
      await Promise.all((handlers.get(msg.kind) ?? []).map((h) => h(msg)))
    },
    // One frame of `dt` seconds, then let the poll's promise chain settle.
    tick: async (dt) => {
      pushed.forEach((s) => s(dt))
      await new Promise((r) => setTimeout(r, 0))
    }
  }
}

const polls = (): string[] => signed.mock.calls.map((c) => String(c[0])).filter((u) => u.includes('onlyUnread'))
const news = (sent: SceneToPage[]): string[][] =>
  sent.flatMap((m) => (m.kind === 'notificationsNew' ? [m.notifications.map((n) => n.id)] : []))

beforeEach(() => {
  signed.mockReset()
  player = { userId: '0xa', isGuest: false }
  identity.observe(null)
})

describe('bridge notifications poll', () => {
  it('waits for the page to fetch the list, then polls every interval from the newest timestamp', async () => {
    const h = fakeCtx()
    await h.tick(POLL_INTERVAL_S * 3)
    expect(polls()).toHaveLength(0)

    signed.mockResolvedValueOnce({ notifications: [raw('a', '1000'), raw('b', '3000', true)] })
    await h.req({ kind: 'getNotifications' })
    signed.mockResolvedValue({ notifications: [] })
    await h.tick(POLL_INTERVAL_S - 1)
    expect(polls()).toHaveLength(0)
    await h.tick(1)
    expect(polls()).toEqual(['https://notifications.decentraland.org/notifications?onlyUnread=true&limit=50&from=3000'])
  })

  it('pushes unread new ones and moves the cursor past them', async () => {
    const h = fakeCtx()
    signed.mockResolvedValueOnce({ notifications: [] })
    await h.req({ kind: 'getNotifications' })
    signed.mockResolvedValueOnce({ notifications: [raw('c', '5000'), raw('d', '4000')] })
    await h.tick(POLL_INTERVAL_S)
    expect(news(h.sent)).toEqual([['c', 'd']])
    expect(polls()[0]).not.toContain('from=')

    signed.mockResolvedValueOnce({ notifications: [] })
    await h.tick(POLL_INTERVAL_S)
    expect(polls()[1]).toContain('from=5000')
    expect(news(h.sent)).toHaveLength(1)
  })

  it('does not poll for a guest or after logout', async () => {
    const h = fakeCtx()
    signed.mockResolvedValue({ notifications: [] })
    await h.req({ kind: 'getNotifications' })
    player = { userId: '0xg', isGuest: true }
    await h.tick(POLL_INTERVAL_S)
    player = null
    await h.tick(POLL_INTERVAL_S)
    expect(polls()).toHaveLength(0)
  })

  it('backs off on failures up to the cap, and resets on success', async () => {
    const h = fakeCtx()
    signed.mockResolvedValueOnce({ notifications: [] })
    await h.req({ kind: 'getNotifications' })
    const errors = vi.spyOn(console, 'error').mockImplementation(() => {})
    signed.mockRejectedValue(new Error('HTTP 503'))
    await h.tick(POLL_INTERVAL_S)
    expect(polls()).toHaveLength(1)
    await h.tick(POLL_INTERVAL_S)
    expect(polls()).toHaveLength(1) // now waiting 2× the interval
    await h.tick(POLL_INTERVAL_S)
    expect(polls()).toHaveLength(2)
    // Waits double to the cap: 20 s, 40 s, then 60 s and no longer.
    await h.tick(POLL_BACKOFF_MAX_S / 3)
    expect(polls()).toHaveLength(3)
    await h.tick(POLL_BACKOFF_MAX_S * (2 / 3))
    expect(polls()).toHaveLength(4)
    await h.tick(POLL_BACKOFF_MAX_S - 1)
    expect(polls()).toHaveLength(4)
    await h.tick(1)
    expect(polls()).toHaveLength(5)
    await h.tick(POLL_BACKOFF_MAX_S)
    expect(polls()).toHaveLength(6)
    // A success puts it back on the normal interval.
    signed.mockResolvedValueOnce({ notifications: [] })
    await h.tick(POLL_BACKOFF_MAX_S)
    expect(polls()).toHaveLength(7)
    await h.tick(POLL_INTERVAL_S)
    expect(polls()).toHaveLength(8)
    errors.mockRestore()
  })

  it('drops a poll answer that was in flight across an account switch, even with no cursor', async () => {
    const h = fakeCtx()
    identity.observe('0xa')
    signed.mockResolvedValueOnce({ notifications: [] })
    await h.req({ kind: 'getNotifications' })
    let answerA!: (v: unknown) => void
    signed.mockReturnValueOnce(new Promise((r) => (answerA = r)))
    await h.tick(POLL_INTERVAL_S) // A's poll is now out
    identity.observe('0xb')
    signed.mockResolvedValueOnce({ notifications: [] })
    await h.req({ kind: 'getNotifications' }) // B is armed again, cursor still empty
    answerA({ notifications: [raw('a-only', '9000')] })
    await h.tick(0)
    expect(news(h.sent)).toEqual([])
  })

  it('an account switch resets the backoff, so the next account polls on the normal interval', async () => {
    const h = fakeCtx()
    identity.observe('0xa')
    signed.mockResolvedValueOnce({ notifications: [] })
    await h.req({ kind: 'getNotifications' })
    const errors = vi.spyOn(console, 'error').mockImplementation(() => {})
    signed.mockRejectedValue(new Error('HTTP 503'))
    await h.tick(POLL_INTERVAL_S)
    await h.tick(POLL_INTERVAL_S * 2)
    await h.tick(POLL_INTERVAL_S * 4) // waiting 40 s now
    const before = polls().length
    identity.observe('0xb')
    signed.mockResolvedValue({ notifications: [] })
    await h.req({ kind: 'getNotifications' })
    await h.tick(POLL_INTERVAL_S)
    expect(polls()).toHaveLength(before + 1)
    errors.mockRestore()
  })

  it('polls anyway, with no cursor, when the first list fetch fails', async () => {
    const h = fakeCtx()
    signed.mockRejectedValueOnce(new Error('HTTP 503'))
    await expect(h.req({ kind: 'getNotifications' })).rejects.toThrow('HTTP 503')
    signed.mockResolvedValueOnce({ notifications: [raw('n', '7000')] })
    await h.tick(POLL_INTERVAL_S)
    expect(polls()).toEqual(['https://notifications.decentraland.org/notifications?onlyUnread=true&limit=50'])
    expect(news(h.sent)).toEqual([['n']])
  })

  it('an account switch stops the poll until the new account fetches its list', async () => {
    const h = fakeCtx()
    identity.observe('0xa')
    signed.mockResolvedValue({ notifications: [] })
    await h.req({ kind: 'getNotifications' })
    identity.observe('0xb')
    await h.tick(POLL_INTERVAL_S)
    expect(polls()).toHaveLength(0)
  })
})
