import { beforeEach, describe, it, expect, vi } from 'vitest'

const api = vi.hoisted(() => ({
  sendReaction: vi.fn() as ReturnType<typeof vi.fn> | undefined
}))
vi.mock('../../bridge-scene/src/bevy-api', () => ({ BevyApi: api }))

const { registerReactions } = await import('../../bridge-scene/src/domains/reactions')

/** The domain on a fake bridge: its handlers of page messages. */
function bridge(): { on: Map<string, (msg?: unknown) => void> } {
  const on = new Map<string, (msg?: unknown) => void>()
  registerReactions({
    send: () => undefined,
    on: (kind, handler) => on.set(kind, handler as (msg?: unknown) => void),
    push: () => undefined
  })
  return { on }
}

describe('situational reactions (bridge)', () => {
  beforeEach(() => {
    api.sendReaction = vi.fn()
  })

  it("passes the page's pick to the engine", () => {
    const b = bridge()
    b.on.get('sendReaction')?.({ kind: 'sendReaction', emoji: '🔥' })
    expect(api.sendReaction).toHaveBeenCalledWith('🔥')
  })

  it('does nothing on an engine that has no reactions', () => {
    api.sendReaction = undefined
    const b = bridge()
    expect(() => b.on.get('sendReaction')?.({ kind: 'sendReaction', emoji: '🔥' })).not.toThrow()
  })
})
