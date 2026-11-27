// The Shop's avatar preview in the bridge (bridge-scene/src/domains/avatarPreview): the camera framed as the Shop's
// page draws the avatar, and eased from one page's framing to another's while the stage stays up. The engine's ECS is
// a fake that keeps components as plain records.
import { describe, expect, it, vi } from 'vitest'
import type { Ctx } from '../../bridge-scene/src/bridge'
import type { PageToScene } from '@explorer-core/engine/protocol'

const ecs = vi.hoisted(() => {
  type Record_ = Record<string, unknown>
  const stores = new Map<string, Map<number, Record_>>()
  const component = (name: string) => {
    const store = new Map<number, Record_>()
    stores.set(name, store)
    const put = (e: number, value: Record_ = {}): Record_ => {
      store.set(e, { ...value })
      return store.get(e)!
    }
    return {
      create: put,
      createOrReplace: put,
      get: (e: number) => store.get(e),
      getOrNull: (e: number) => store.get(e) ?? null,
      getMutable: (e: number) => {
        if (!store.has(e)) store.set(e, {})
        return store.get(e)!
      },
      getMutableOrNull: (e: number) => store.get(e) ?? null,
      has: (e: number) => store.has(e),
      deleteFrom: (e: number) => store.delete(e)
    }
  }
  let next = 512
  const module = {
    engine: { RootEntity: 0, addEntity: () => next++, removeEntity: () => undefined },
    AssetLoadLoadingState: component('AssetLoadLoadingState'),
    AvatarShape: component('AvatarShape'),
    CameraLayer: component('CameraLayer'),
    CameraLayers: component('CameraLayers'),
    Material: { setPbrMaterial: () => undefined },
    MeshRenderer: { setCylinder: () => undefined },
    PrimaryPointerInfo: component('PrimaryPointerInfo'),
    TextureCamera: component('TextureCamera'),
    Transform: component('Transform')
  }
  const reset = (): void => stores.forEach((s) => s.clear())
  return { stores, module, reset }
})

vi.mock('../../bridge-scene/node_modules/@dcl/sdk/ecs.js', () => ecs.module)
vi.mock('../../bridge-scene/node_modules/@dcl/sdk/math.js', () => {
  const v = (x: number, y: number, z: number) => ({ x, y, z })
  return {
    Vector3: { create: v, Zero: () => v(0, 0, 0), One: () => v(1, 1, 1) },
    Color4: { create: (r: number, g: number, b: number, a: number) => ({ r, g, b, a }), lerp: (a: unknown) => a },
    Quaternion: {
      create: (x = 0, y = 0, z = 0, w = 1) => ({ x, y, z, w }),
      fromEulerDegrees: () => ({ x: 0, y: 0, z: 0, w: 1 }),
      fromAngleAxis: () => ({ x: 0, y: 0, z: 0, w: 1 }),
      multiply: (a: unknown) => a
    }
  }
})
vi.mock('../../bridge-scene/node_modules/@dcl/sdk/players/index.js', () => ({ getPlayer: () => null }))
vi.mock('../../bridge-scene/node_modules/@dcl/react-ecs/dist/index.js', () => ({ default: { createElement: () => null }, UiEntity: () => null }))
vi.mock('../../bridge-scene/src/domains/avatarDraft', () => ({ currentLook: () => null }))
const { api } = vi.hoisted(() => ({ api: {} as Record<string, unknown> }))
vi.mock('../../bridge-scene/src/bevy-api', () => ({ BevyApi: api }))

const { SHOP_FRAMING, SHOP_NAMES_FRAMING } = await import('../../bridge-scene/src/domains/previewView')

const WINDOW = { x: 0, y: 138, width: 573, height: 942 }
const STAGE = { x: 0, y: 68, width: 1920, height: 1080 }

/** A fresh preview, its Shop stage up on a page of `framing`. */
async function openShop(framing: 'names' | 'catalog') {
  vi.resetModules()
  ecs.reset()
  const { registerAvatarPreview } = await import('../../bridge-scene/src/domains/avatarPreview')
  const handlers = new Map<string, (msg: PageToScene) => void>()
  const systems: ((dt: number) => void)[] = []
  const ctx = {
    send: () => undefined,
    on: (kind: string, handler: (msg: PageToScene) => void) => handlers.set(kind, handler),
    push: (s: (dt: number) => void) => systems.push(s)
  } as unknown as Ctx
  registerAvatarPreview(ctx)
  const emit = (msg: PageToScene): void => handlers.get(msg.kind)!(msg)
  const frame = (dt = 0): void => systems.forEach((s) => s(dt))
  /** The vertical range of the preview's orthographic camera: the framing's, zoomed. */
  const camera = (): number => {
    const [cam] = [...ecs.stores.get('TextureCamera')!.values()] as { mode: { orthographic: { verticalRange: number } } }[]
    return cam.mode.orthographic.verticalRange
  }
  emit({ kind: 'engineViewport', region: 'avatarPreview', rect: WINDOW, dpr: 1 } as PageToScene)
  emit({ kind: 'previewStage', rect: STAGE, framing } as PageToScene)
  frame()
  frame()
  return { emit, frame, camera }
}

describe("the Shop's avatar preview", () => {
  it("eases to Names' framing, a resize midway not snapping it, and back from midway", async () => {
    const shop = await openShop('catalog')
    expect(shop.camera()).toBeCloseTo(SHOP_FRAMING.range, 9)

    shop.emit({ kind: 'previewStage', rect: STAGE, framing: 'names' } as PageToScene)
    shop.frame(0.15) // a quarter of the ease
    const quarter = shop.camera()
    expect(quarter).toBeGreaterThan(SHOP_FRAMING.range)
    expect(quarter).toBeLessThan(SHOP_NAMES_FRAMING.range)
    // the window resized: the same page's stage again, elsewhere, and the ease runs on from where it is, ending
    // when it would have without the resize (a restarted ease would still be under way)
    shop.emit({ kind: 'previewStage', rect: { ...STAGE, x: 10 }, framing: 'names' } as PageToScene)
    expect(shop.camera()).toBe(quarter)
    shop.frame(0.45)
    expect(shop.camera()).toBeCloseTo(SHOP_NAMES_FRAMING.range, 9)

    // back to the catalogue, and to Names again from midway: each ease starts where the camera is
    shop.emit({ kind: 'previewStage', rect: STAGE, framing: 'catalog' } as PageToScene)
    shop.frame(0.15)
    expect(shop.camera()).toBeLessThan(SHOP_NAMES_FRAMING.range)
    expect(shop.camera()).toBeGreaterThan(SHOP_FRAMING.range)
    shop.emit({ kind: 'previewStage', rect: STAGE, framing: 'names' } as PageToScene)
    const turned = shop.camera()
    shop.frame(0.05)
    expect(shop.camera()).toBeGreaterThan(turned)
    expect(shop.camera()).toBeLessThan(SHOP_NAMES_FRAMING.range)
  })

  it('frames the avatar as the catalogue does when the page names no framing', async () => {
    const shop = await openShop('names')
    shop.emit({ kind: 'previewStage', rect: STAGE } as PageToScene)
    shop.frame(0.6)
    expect(shop.camera()).toBeCloseTo(SHOP_FRAMING.range, 9)
  })

  it('opens on Names in its framing at once, and gives the Backpack its framing back when the stage goes', async () => {
    const shop = await openShop('names')
    expect(shop.camera()).toBeCloseTo(SHOP_NAMES_FRAMING.range, 9)
    shop.emit({ kind: 'previewStage', rect: null } as PageToScene)
    expect(shop.camera()).not.toBeCloseTo(SHOP_NAMES_FRAMING.range, 3)
    shop.emit({ kind: 'previewStage', rect: STAGE, framing: 'names' } as PageToScene)
    expect(shop.camera()).toBeCloseTo(SHOP_NAMES_FRAMING.range, 9)
  })
})
