import { beforeEach, describe, it, expect, vi } from 'vitest'

const { relay } = vi.hoisted(() => ({ relay: vi.fn() }))
vi.mock('../../bridge-scene/src/system-helpers', () => ({ relay }))
const api = vi.hoisted(() => ({ setNearbyMuted: vi.fn(), getNearbyMuteStream: vi.fn() }))
vi.mock('../../bridge-scene/src/bevy-api', () => ({ BevyApi: api }))

const { isHushed, registerNearbyMutes } = await import('../../bridge-scene/src/domains/nearbyMutes')

type Sent = { kind: string; addresses?: string[] }

/** The domain on a fake bridge: what it sent, its handlers and the engine stream's item handler. */
function bridge(): { sent: Sent[]; on: Map<string, (msg?: unknown) => void>; engine: (addresses: string[]) => void } {
  const sent: Sent[] = []
  const on = new Map<string, (msg?: unknown) => void>()
  relay.mockClear()
  registerNearbyMutes({
    send: (msg) => sent.push(msg as never),
    on: (kind, handler) => on.set(kind, handler as (msg?: unknown) => void),
    push: () => undefined
  })
  return { sent, on, engine: relay.mock.calls[0][2] as (addresses: string[]) => void }
}

const ADDR = '0x5854cce95d5e25817b41f4c41f06b695a83bc495'

describe('saved mutes for nearby voice (bridge)', () => {
  beforeEach(() => api.setNearbyMuted.mockReset())

  it("relays the engine's list to the page and to the nametags", () => {
    const b = bridge()
    b.engine([ADDR.toUpperCase().replace('0X', '0x')])
    expect(b.sent).toEqual([{ kind: 'nearbyMutes', addresses: [ADDR] }])
    expect(isHushed(ADDR.toUpperCase().replace('0X', '0x'))).toBe(true)
    // whole on every change: what the next list leaves out is no longer hushed
    b.engine([])
    expect(isHushed(ADDR)).toBe(false)
    expect(b.sent[1]).toEqual({ kind: 'nearbyMutes', addresses: [] })
  })

  it('sends the list again to a page that starts late', () => {
    const b = bridge()
    b.engine([ADDR])
    b.on.get('hello')?.()
    expect(b.sent).toEqual([
      { kind: 'nearbyMutes', addresses: [ADDR] },
      { kind: 'nearbyMutes', addresses: [ADDR] }
    ])
  })

  it('passes a mute to the engine', () => {
    const b = bridge()
    api.setNearbyMuted.mockResolvedValue(undefined)
    b.on.get('setNearbyMute')?.({ kind: 'setNearbyMute', address: ADDR, muted: true })
    expect(api.setNearbyMuted).toHaveBeenCalledWith(ADDR, true)
  })

  it("puts the engine's list back on the page when the engine refuses", async () => {
    vi.spyOn(console, 'error').mockImplementation(() => undefined)
    const b = bridge()
    b.engine([])
    // a thenable, not a rejected promise: this runner fails a test whose mock returned one, even handled
    const refused = Promise.reject(new Error('Cannot mute yourself'))
    refused.catch(() => undefined)
    api.setNearbyMuted.mockImplementation(() => ({ catch: (fn: (e: unknown) => void) => refused.catch(fn) }))
    b.on.get('setNearbyMute')?.({ kind: 'setNearbyMute', address: ADDR, muted: true })
    for (let i = 0; i < 5; i++) await Promise.resolve()
    expect(b.sent).toEqual([
      { kind: 'nearbyMutes', addresses: [] },
      { kind: 'nearbyMutes', addresses: [] }
    ])
  })
})
