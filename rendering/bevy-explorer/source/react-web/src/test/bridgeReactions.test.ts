import { beforeEach, describe, it, expect, vi } from 'vitest'

const { relay } = vi.hoisted(() => ({ relay: vi.fn() }))
vi.mock('../../bridge-scene/src/system-helpers', () => ({ relay }))
const api = vi.hoisted(() => ({
  sendReaction: vi.fn() as ReturnType<typeof vi.fn> | undefined,
  getReactionStream: vi.fn() as ReturnType<typeof vi.fn> | undefined
}))
vi.mock('../../bridge-scene/src/bevy-api', () => ({ BevyApi: api }))

const { registerReactions } = await import('../../bridge-scene/src/domains/reactions')

type Sent = { kind: string; reaction?: { from: string; emoji: string } }

/** The domain on a fake bridge: what it sent to the page and its handlers of page messages. */
function bridge(): { sent: Sent[]; on: Map<string, (msg?: unknown) => void> } {
  const sent: Sent[] = []
  const on = new Map<string, (msg?: unknown) => void>()
  relay.mockClear()
  registerReactions({
    send: (msg) => sent.push(msg as never),
    on: (kind, handler) => on.set(kind, handler as (msg?: unknown) => void),
    push: () => undefined
  })
  return { sent, on }
}

const ADDR = '0x5854cce95d5e25817b41f4c41f06b695a83bc495'

describe('situational reactions (bridge)', () => {
  beforeEach(() => {
    api.sendReaction = vi.fn()
    api.getReactionStream = vi.fn()
  })

  it("passes the page's pick to the engine", () => {
    const b = bridge()
    b.on.get('sendReaction')?.({ kind: 'sendReaction', emoji: '🔥' })
    expect(api.sendReaction).toHaveBeenCalledWith('🔥')
  })

  it('relays each reaction the engine plays to the page', () => {
    const b = bridge()
    expect(relay.mock.calls[0][0]).toBe('reaction')
    const played = relay.mock.calls[0][2] as (reaction: { from: string; emoji: string }) => void
    played({ from: ADDR.toUpperCase().replace('0X', '0x'), emoji: '👍' })
    expect(b.sent).toEqual([{ kind: 'reaction', reaction: { from: ADDR, emoji: '👍' } }])
  })

  it('does nothing on an engine that has no reactions', () => {
    api.sendReaction = undefined
    api.getReactionStream = undefined
    const b = bridge()
    expect(relay).not.toHaveBeenCalled()
    expect(() => b.on.get('sendReaction')?.({ kind: 'sendReaction', emoji: '🔥' })).not.toThrow()
    expect(b.sent).toEqual([])
  })
})
