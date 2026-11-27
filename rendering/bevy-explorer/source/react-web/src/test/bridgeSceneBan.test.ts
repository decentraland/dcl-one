import { afterEach, describe, expect, it, vi } from 'vitest'
import type { Ctx } from '../../bridge-scene/src/bridge'
import type { SceneBanNotice } from '@explorer-core/engine/generated'
import type { PageToScene, SceneToPage } from '@explorer-core/engine/protocol'

const api = vi.hoisted(() => ({ BevyApi: {} as { getSceneBanStream?: () => Promise<AsyncIterable<SceneBanNotice>> } }))
vi.mock('../../bridge-scene/src/bevy-api', () => api)
vi.mock('../../bridge-scene/src/system-helpers', async () => {
  const { relayStream } = await import('../../bridge-scene/src/relay')
  return { relay: (name: string, source: () => Promise<AsyncIterable<unknown>>, onItem: (item: unknown) => void) => {
    relayStream(name, source, onItem, () => new Promise(() => {}))
  } }
})

afterEach(() => { delete api.BevyApi.getSceneBanStream })

async function bridge(): Promise<{ sent: SceneToPage[]; hello: () => void }> {
  const { registerSceneBan } = await import('../../bridge-scene/src/domains/sceneBan')
  const sent: SceneToPage[] = []
  const handlers = new Map<string, (message: PageToScene) => void>()
  const ctx: Ctx = {
    send: message => { sent.push(message) },
    on: (kind, handler) => { handlers.set(kind, handler as (message: PageToScene) => void) },
    push: () => { throw new Error('the scene ban is a stream, not a frame poll') }
  }
  registerSceneBan(ctx)
  return { sent, hello: () => handlers.get('hello')?.({ kind: 'hello' } as PageToScene) }
}

describe('scene ban bridge', () => {
  it('relays the engine notice and sends the last one again to a page that says hello', async () => {
    let next!: (item: IteratorResult<SceneBanNotice>) => void
    const stream: AsyncIterableIterator<SceneBanNotice> = {
      [Symbol.asyncIterator]() { return this },
      next: () => new Promise(resolve => { next = resolve })
    }
    api.BevyApi.getSceneBanStream = async () => stream
    const { sent, hello } = await bridge()
    hello()
    expect(sent).toEqual([])

    await vi.waitFor(() => expect(next).toBeDefined())
    next({ done: false, value: { scope: 'scene' } })
    await vi.waitFor(() => expect(sent).toEqual([{ kind: 'sceneBan', ban: { scope: 'scene' } }]))

    hello()
    expect(sent).toEqual([
      { kind: 'sceneBan', ban: { scope: 'scene' } },
      { kind: 'sceneBan', ban: { scope: 'scene' } }
    ])
  })

  it('sends nothing on an engine without the stream', async () => {
    const { sent, hello } = await bridge()
    hello()
    expect(sent).toEqual([])
  })
})
