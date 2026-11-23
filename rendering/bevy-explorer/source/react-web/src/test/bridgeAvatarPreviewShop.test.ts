// The Shop's avatar preview in the bridge (bridge-scene/src/domains/avatarPreview): the camera framed as the Shop's
// page draws the avatar, eased from one page's framing to another's while the stage stays up, and, on a Names page
// only, the nametag following the view from the anchor already read. The engine's ECS is a fake that keeps components
// as plain records.
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { Ctx } from '../../bridge-scene/src/bridge'
import type { PageToScene, SceneToPage } from '@explorer-core/engine/protocol'

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
const { NAMETAG_SETTLE_SECONDS } = await import('../../bridge-scene/src/domains/previewNametagFeed')

/** The engine's nametag anchor for the Shop's avatar: 3.6m over its feet (a 2×-scale head). */
const ANCHOR = { x: 0, y: 3.6, z: 0 }
const WINDOW = { x: 0, y: 138, width: 573, height: 942 }
const STAGE = { x: 0, y: 68, width: 1920, height: 1080 }

type Anchor = { x: number; y: number } | null

type Offset = { x: number; y: number; z: number }

/** One anchor stream the preview opened, as the engine serves it: what the test has it tell, and whether it was closed. */
function anchorStream() {
  const queue: { measured: boolean; x: number; y: number; z: number }[] = []
  let wake: (() => void) | null = null
  const state = { closed: false }
  const iterable = {
    close: () => {
      state.closed = true
      wake?.()
    },
    async *[Symbol.asyncIterator]() {
      while (!state.closed) {
        while (queue.length > 0) yield queue.shift()!
        if (state.closed) return
        await new Promise<void>((resolve) => (wake = resolve))
      }
    }
  }
  return {
    iterable,
    state,
    tell: (offset: Offset) => {
      queue.push({ measured: true, ...offset })
      wake?.()
    }
  }
}

/** A fresh preview, its Shop stage up on a page of `framing` (a Names page by default). On a Names page the engine's
 *  stream for its avatar tells `first` (the anchor by default) as it opens; the test tells it more with `tell`. */
async function openShop(framing: 'names' | 'catalog' = 'names', first: Offset | null = ANCHOR) {
  vi.resetModules()
  ecs.reset()
  const streams: ReturnType<typeof anchorStream>[] = []
  api.getAvatarNametagAnchorStream = vi.fn(async () => {
    const stream = anchorStream()
    streams.push(stream)
    if (first) stream.tell(first)
    return stream.iterable
  })
  const { registerAvatarPreview } = await import('../../bridge-scene/src/domains/avatarPreview')
  const handlers = new Map<string, (msg: PageToScene) => void>()
  const systems: ((dt: number) => void)[] = []
  const sent: SceneToPage[] = []
  const ctx = {
    send: (m: SceneToPage) => sent.push(m),
    on: (kind: string, handler: (msg: PageToScene) => void) => handlers.set(kind, handler),
    push: (s: (dt: number) => void) => systems.push(s)
  } as unknown as Ctx
  registerAvatarPreview(ctx)
  const emit = (msg: PageToScene): void => handlers.get(msg.kind)!(msg)
  const settle = async (): Promise<void> => {
    for (let i = 0; i < 10; i++) await Promise.resolve()
  }
  const frame = async (dt = 0): Promise<void> => {
    systems.forEach((s) => s(dt))
    await settle()
  }
  /** The engine tells the stream open now that the anchor is `offset`. */
  const tell = async (offset: Offset): Promise<void> => {
    streams.at(-1)!.tell(offset)
    await settle()
  }
  const camera = (): number => {
    const [cam] = [...ecs.stores.get('TextureCamera')!.values()] as { mode: { orthographic: { verticalRange: number } } }[]
    return cam.mode.orthographic.verticalRange
  }
  const tags = (): Anchor[] => sent.filter((m) => m.kind === 'previewNametag').map((m) => (m as { anchor: Anchor }).anchor)
  emit({ kind: 'engineViewport', region: 'avatarPreview', rect: WINDOW, dpr: 1 } as PageToScene)
  emit({ kind: 'previewStage', rect: STAGE, framing } as PageToScene)
  await frame()
  await frame()
  return { emit, frame, tell, camera, tags, streams, open: api.getAvatarNametagAnchorStream as ReturnType<typeof vi.fn> }
}

beforeEach(() => {
  for (const k of Object.keys(api)) delete api[k]
})

describe("the Shop's avatar preview", () => {
  it('follows no anchor on a catalogue page, starts once a Names page is on the stage, and stops when it leaves', async () => {
    const shop = await openShop('catalog')
    await shop.frame(1)
    expect(shop.open).not.toHaveBeenCalled()
    expect(shop.tags()).toEqual([])
    shop.emit({ kind: 'previewStage', rect: STAGE, framing: 'names' } as PageToScene)
    await shop.frame(0.05)
    expect(shop.open).toHaveBeenCalledTimes(1)
    expect(shop.tags()).toHaveLength(1)
    // another page: the tag goes at once, and the stream is closed
    shop.emit({ kind: 'previewStage', rect: STAGE, framing: 'catalog' } as PageToScene)
    expect(shop.tags().at(-1)).toBeNull()
    expect(shop.streams[0].state.closed).toBe(true)
  })

  it("eases to Names' framing, a resize midway neither snapping it nor dropping the tag, and back from midway", async () => {
    const shop = await openShop('catalog')
    expect(shop.camera()).toBeCloseTo(SHOP_FRAMING.range, 9)

    shop.emit({ kind: 'previewStage', rect: STAGE, framing: 'names' } as PageToScene)
    await shop.frame(0.15) // a quarter of the ease, and the Names page's stream telling the anchor
    const quarter = shop.camera()
    expect(quarter).toBeGreaterThan(SHOP_FRAMING.range)
    expect(quarter).toBeLessThan(SHOP_NAMES_FRAMING.range)
    const [early] = shop.tags()
    expect(early).not.toBeNull()
    // the window resized: the same page's stage again, elsewhere
    shop.emit({ kind: 'previewStage', rect: { ...STAGE, x: 10 }, framing: 'names' } as PageToScene)
    expect(shop.camera()).toBe(quarter)
    await shop.frame(0.6)
    expect(shop.camera()).toBeCloseTo(SHOP_NAMES_FRAMING.range, 9)
    // the tag followed the view, lower in the window as the avatar is drawn lower, and was never dropped
    const tags = shop.tags()
    expect(tags).not.toContain(null)
    expect(tags.at(-1)!.y).toBeGreaterThan(early!.y)

    // back to the catalogue from midway: the ease starts where the camera is, and the tag goes
    shop.emit({ kind: 'previewStage', rect: STAGE, framing: 'catalog' } as PageToScene)
    await shop.frame(0.15)
    expect(shop.camera()).toBeLessThan(SHOP_NAMES_FRAMING.range)
    expect(shop.tags().at(-1)).toBeNull()
    shop.emit({ kind: 'previewStage', rect: STAGE, framing: 'names' } as PageToScene)
    const turned = shop.camera()
    await shop.frame(0.05)
    expect(shop.camera()).toBeGreaterThan(turned)
    expect(shop.camera()).toBeLessThan(SHOP_NAMES_FRAMING.range)
  })

  it('puts the tag back where the unzoomed view has it as soon as the zoom returns, from the anchor told', async () => {
    const shop = await openShop()
    const [unzoomed] = shop.tags()
    shop.emit({ kind: 'previewZoom', zoom: 2, anchor: 0.3 } as PageToScene)
    await shop.frame()
    expect(shop.tags().at(-1)).not.toEqual(unzoomed)
    shop.emit({ kind: 'previewZoom', zoom: 1, anchor: 0.5 } as PageToScene)
    await shop.frame()
    expect(shop.tags().at(-1)!.x).toBeCloseTo(unzoomed!.x, 9)
    expect(shop.tags().at(-1)!.y).toBeCloseTo(unzoomed!.y, 9)
    expect(shop.open).toHaveBeenCalledTimes(1)
  })

  it("stands the tag on the avatar's axis, still while the head sways and turns, and moves it for a hat", async () => {
    // the head off the axis (told again once the pose has settled), then swaying 3cm up and turned the other way,
    // then under a hat
    const shop = await openShop('names', { x: 0.2, y: 3.6, z: -0.3 })
    const [first] = shop.tags()
    expect(first!.x).toBeCloseTo(0.5, 9)
    await shop.frame(NAMETAG_SETTLE_SECONDS)
    await shop.tell({ x: 0.2, y: 3.6, z: -0.3 }) // the place held from now on
    await shop.tell({ x: -0.15, y: 3.63, z: 0.2 }) // the sway
    expect(shop.tags()).toEqual([first])
    await shop.tell({ x: 0.1, y: 3.95, z: 0 }) // the hat
    const tags = shop.tags()
    expect(tags).toHaveLength(2)
    expect(tags[1]!.x).toBeCloseTo(0.5, 9)
    expect(tags[1]!.y).toBeLessThan(first!.y)
  })

  it('opens on Names in its framing, and takes the tag away and closes the stream when the stage goes', async () => {
    const shop = await openShop()
    shop.emit({ kind: 'previewStage', rect: null } as PageToScene)
    expect(shop.tags().at(-1)).toBeNull()
    expect(shop.streams[0].state.closed).toBe(true)
    shop.emit({ kind: 'previewStage', rect: STAGE, framing: 'names' } as PageToScene)
    expect(shop.camera()).toBeCloseTo(SHOP_NAMES_FRAMING.range, 9)
  })

  it('places the tag afresh after an emote, where it would have held through a sway', async () => {
    // settled at 3.6m, then 3cm higher: held as a sway; then an emote stops and the head comes back 4cm higher
    const shop = await openShop()
    const [first] = shop.tags()
    await shop.frame(NAMETAG_SETTLE_SECONDS)
    await shop.tell(ANCHOR)
    await shop.tell({ x: 0, y: 3.63, z: 0 })
    expect(shop.tags()).toEqual([first])
    shop.emit({ kind: 'previewEmote', urn: null } as PageToScene)
    await shop.tell({ x: 0, y: 3.64, z: 0 })
    const tags = shop.tags()
    expect(tags).toHaveLength(2)
    expect(tags[1]!.y).toBeLessThan(first!.y)
  })

  it('tells a reloaded page where the tag is, though it has not moved', async () => {
    const shop = await openShop()
    const [first] = shop.tags()
    shop.emit({ kind: 'hello' } as PageToScene)
    await shop.frame()
    // followed afresh: the engine tells the new stream the anchor as it opens
    expect(shop.open).toHaveBeenCalledTimes(2)
    expect(shop.tags()).toEqual([first, null, first])
  })
})
