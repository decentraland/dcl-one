// The bridge scene's sky time relay: what the engine's stream reports reaches the page, and a
// page that starts late is sent the last report again.
import { beforeEach, describe, it, expect, vi } from 'vitest'
import type { SkyTimeState } from '@explorer-core/engine/generated'

const { relay } = vi.hoisted(() => ({ relay: vi.fn() }))
vi.mock('../../bridge-scene/src/system-helpers', () => ({ relay }))
const api = vi.hoisted(() => ({ getSkyTimeStream: vi.fn() as ReturnType<typeof vi.fn> | undefined }))
vi.mock('../../bridge-scene/src/bevy-api', () => ({ BevyApi: api }))

const { registerSkyTime } = await import('../../bridge-scene/src/domains/skyTime')

const RUNNING: SkyTimeState = { locked: false, speed: 12, fixedHours: null }
const LOCKED: SkyTimeState = { locked: true, speed: 12, fixedHours: 22 }

function bridge(): { sent: unknown[]; on: Map<string, () => void> } {
  const sent: unknown[] = []
  const on = new Map<string, () => void>()
  registerSkyTime({
    send: (msg) => sent.push(msg),
    on: (kind, handler) => on.set(kind, handler as () => void),
    push: () => undefined
  })
  return { sent, on }
}

describe('bridge sky time', () => {
  beforeEach(() => {
    relay.mockClear()
    api.getSkyTimeStream = vi.fn()
  })

  it('relays each report of the engine stream', async () => {
    const b = bridge()
    const [name, open, onItem] = relay.mock.calls[0] as [string, () => Promise<unknown>, (state: SkyTimeState) => void]
    expect(name).toBe('skyTime')
    await open()
    expect(api.getSkyTimeStream).toHaveBeenCalledTimes(1)
    onItem(RUNNING)
    onItem(LOCKED)
    expect(b.sent).toEqual([
      { kind: 'skyTime', state: RUNNING },
      { kind: 'skyTime', state: LOCKED }
    ])
  })

  it('sends the last report again to a page that starts late', () => {
    const b = bridge()
    const onItem = relay.mock.calls[0][2] as (state: SkyTimeState) => void
    onItem(LOCKED)
    b.on.get('hello')?.()
    expect(b.sent).toEqual([
      { kind: 'skyTime', state: LOCKED },
      { kind: 'skyTime', state: LOCKED }
    ])
  })

  it('sends nothing to a page that starts before the first report', () => {
    const b = bridge()
    b.on.get('hello')?.()
    expect(b.sent).toEqual([])
  })

  it('registers nothing on an engine without the stream', () => {
    api.getSkyTimeStream = undefined
    const b = bridge()
    expect(relay).not.toHaveBeenCalled()
    expect(b.on.size).toBe(0)
  })
})
