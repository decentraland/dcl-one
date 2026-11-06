import { it, expect, vi } from 'vitest'
import type { Ctx } from '../../bridge-scene/src/bridge'
import type { SceneToPage } from '../engine/protocol'
import type { TextureOptimizationStatus } from '../engine/generated'

vi.mock('../../bridge-scene/node_modules/@dcl/sdk/ecs.js', () => ({
  engine: { addSystem: () => undefined, removeSystem: () => undefined }
}))

const statuses: TextureOptimizationStatus[] = [
  { sceneId: 's', state: 'ready', done: 3, total: 3, failed: 0, bytesBefore: 300, bytesAfter: 100, enabled: true, replay: true },
  { sceneId: 's', state: 'running', done: 1, total: 4, failed: 0, bytesBefore: 0, bytesAfter: 0, enabled: true, replay: false }
]
const { api } = vi.hoisted(() => ({ api: {} as Record<string, unknown> }))
vi.mock('../../bridge-scene/src/bevy-api', () => ({ BevyApi: api }))

const { registerTextureOptimization } = await import('../../bridge-scene/src/domains/textureOptimization')

function fakeCtx(): { ctx: Ctx; sent: SceneToPage[] } {
  const sent: SceneToPage[] = []
  return { sent, ctx: { send: (m) => sent.push(m), on: () => undefined, push: () => undefined } }
}

const flush = async (): Promise<void> => {
  for (let i = 0; i < 20; i++) await Promise.resolve()
}

it('relays every engine status to the page as it came, replay flag included', async () => {
  api.getTextureOptimizationStream = vi.fn(async () => {
    async function* items(): AsyncIterable<TextureOptimizationStatus> {
      yield* statuses
      await new Promise(() => {})
    }
    return items()
  })
  const { ctx, sent } = fakeCtx()
  registerTextureOptimization(ctx)
  await flush()
  expect(sent).toEqual(statuses.map((s) => ({ kind: 'textureOptimization', ...s })))
})

it('stays quiet on a runtime without the stream', async () => {
  delete api.getTextureOptimizationStream
  const { ctx, sent } = fakeCtx()
  registerTextureOptimization(ctx)
  await flush()
  expect(sent).toEqual([])
})
