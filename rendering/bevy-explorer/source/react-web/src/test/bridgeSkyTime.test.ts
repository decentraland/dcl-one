// The bridge scene's sky time domain: while the page watches, the time of day the engine reports
// with `getWorldTime` reaches it at once and then once a second.
import { beforeEach, describe, it, expect, vi } from 'vitest'

const runtime = vi.hoisted(() => ({ getWorldTime: vi.fn() }))
vi.mock('~system/Runtime', () => runtime)
vi.mock('@dcl/sdk/ecs', () => ({ engine: { addSystem: vi.fn() } }))
vi.mock('../../bridge-scene/src/relay', () => ({ relayStream: vi.fn() }))

const { registerSkyTime } = await import('../../bridge-scene/src/domains/skyTime')

type Bridge = { sent: unknown[]; watch: (on: boolean) => void; hello: () => void; frame: (dt: number) => Promise<void> }

function bridge(): Bridge {
  const sent: unknown[] = []
  const on = new Map<string, (msg: unknown) => void>()
  let system: (dt: number) => void = () => undefined
  registerSkyTime({
    send: (msg) => sent.push(msg),
    on: (kind, handler) => on.set(kind, handler as (msg: unknown) => void),
    push: (s) => {
      system = s
    }
  })
  return {
    sent,
    watch: (open) => on.get('watchSkyTime')?.({ kind: 'watchSkyTime', on: open }),
    hello: () => on.get('hello')?.({ kind: 'hello' }),
    frame: async (dt) => {
      system(dt)
      await Promise.resolve()
      await Promise.resolve()
    }
  }
}

const LOCKED = { seconds: 79200, locked: true, speed: 12, followsClock: false }
const LOCKED_STATE = { kind: 'skyTime', state: { hours: 22, locked: true, speed: 12, followsClock: false } }

describe('bridge sky time', () => {
  beforeEach(() => {
    runtime.getWorldTime.mockReset()
    runtime.getWorldTime.mockResolvedValue(LOCKED)
  })

  it('reads nothing until the page watches', async () => {
    const b = bridge()
    await b.frame(5)
    expect(runtime.getWorldTime).not.toHaveBeenCalled()
    expect(b.sent).toEqual([])
  })

  it('reports at once when the menu opens, then once a second', async () => {
    const b = bridge()
    b.watch(true)
    await b.frame(0.016)
    expect(b.sent).toEqual([LOCKED_STATE])
    await b.frame(0.5)
    expect(b.sent).toHaveLength(1)
    runtime.getWorldTime.mockResolvedValue({ seconds: 36000, locked: false, speed: 0, followsClock: true })
    await b.frame(0.5)
    expect(b.sent).toEqual([LOCKED_STATE, { kind: 'skyTime', state: { hours: 10, locked: false, speed: 0, followsClock: true } }])
  })

  it('stops when the menu closes and when the page starts again', async () => {
    const b = bridge()
    b.watch(true)
    await b.frame(0.016)
    b.watch(false)
    await b.frame(2)
    b.watch(true)
    await b.frame(0.016)
    b.hello()
    await b.frame(2)
    expect(b.sent).toEqual([LOCKED_STATE, LOCKED_STATE])
  })

  it('drops a read that answers after the menu closed', async () => {
    const b = bridge()
    b.watch(true)
    b.watch(false)
    await b.frame(0.016)
    expect(b.sent).toEqual([])
  })

  it('reports nothing from an engine that tells the hour alone', async () => {
    runtime.getWorldTime.mockResolvedValue({ seconds: 36000 })
    const b = bridge()
    b.watch(true)
    await b.frame(0.016)
    expect(b.sent).toEqual([])
  })
})
