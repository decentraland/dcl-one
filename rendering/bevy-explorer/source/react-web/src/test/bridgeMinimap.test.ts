import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { Ctx } from '../../bridge-scene/src/bridge'
import type { PageToScene, SceneToPage } from '@explorer-core/engine/protocol'

const world = vi.hoisted(() => ({ entities: 0, removed: [] as number[], player: { position: { x: 8, y: 0, z: 8 } } as { position: { x: number; y: number; z: number } } | null }))
vi.mock('../../bridge-scene/node_modules/@dcl/sdk/ecs.js', () => ({
  engine: { CameraEntity: 2, addEntity: () => ++world.entities, removeEntity: (e: number) => world.removed.push(e) },
  CameraLayer: { create: vi.fn() },
  TextureCamera: { create: vi.fn(), getMutableOrNull: () => null },
  Transform: { create: vi.fn(), getOrNull: () => null, getMutableOrNull: () => ({}) }
}))
vi.mock('../../bridge-scene/node_modules/@dcl/sdk/players/index.js', () => ({ getPlayer: () => world.player }))

const { registerMinimap } = await import('../../bridge-scene/src/domains/minimap')

const RECT = { x: 12, y: 60, width: 220, height: 220 }
const config = (style: 'satellite' | 'imposters'): PageToScene => ({ kind: 'minimapConfig', style, rotation: 'north', visibleMeters: 256 })
const viewport = (rect: typeof RECT | null): PageToScene => ({ kind: 'engineViewport', region: 'map', rect })

// One scene for the file, as in the engine: each test starts from no camera.
const sent: SceneToPage[] = []
const handlers = new Map<string, (m: PageToScene) => void>()
let tick = (): void => {}
const ctx: Ctx = {
  send: (m) => sent.push(m),
  on: (kind, h) => handlers.set(kind, h as (m: PageToScene) => void),
  push: (frame) => {
    tick = () => frame(1 / 30)
  }
}
registerMinimap(ctx)
const page = (msg: PageToScene): void => handlers.get(msg.kind)!(msg)
const frames = (n: number): void => {
  for (let i = 0; i < n; i++) tick()
}
const ready = (): number => sent.filter((m) => m.kind === 'minimapReady').length

beforeEach(() => {
  page(viewport(null))
  page(config('satellite'))
  sent.length = 0
  world.player = { position: { x: 8, y: 0, z: 8 } }
})

describe('bridge minimap', () => {
  it('tells the page its camera is drawing a few frames after placing it, once', () => {
    page(viewport(RECT))
    page(config('imposters'))
    frames(14)
    expect(ready()).toBe(0)
    frames(1)
    expect(ready()).toBe(1)
    frames(60)
    expect(ready()).toBe(1)
  })

  it('counts from the frame the camera is placed over the player', () => {
    world.player = null
    page(viewport(RECT))
    page(config('imposters'))
    frames(60)
    expect(ready()).toBe(0)
    world.player = { position: { x: 8, y: 0, z: 8 } }
    frames(15)
    expect(ready()).toBe(1)
  })

  it('says nothing for a camera taken down first, and starts over for the next', () => {
    page(viewport(RECT))
    page(config('imposters'))
    frames(10)
    page(config('satellite'))
    frames(60)
    expect(ready()).toBe(0)
    page(config('imposters'))
    frames(14)
    expect(ready()).toBe(0)
    frames(1)
    expect(ready()).toBe(1)
  })

  it('answers at once for a camera already drawing: a page that reloaded asks again', () => {
    page(viewport(RECT))
    page(config('imposters'))
    frames(15)
    sent.length = 0
    page(viewport(RECT))
    page(config('imposters'))
    expect(ready()).toBe(2)
  })
})
