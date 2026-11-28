// The passport's avatar in the bridge (bridge-scene/src/domains/passportPreview): the look it is given on a
// stage of its own, what it tells the page about the load, the wave, the turns, and the other stage's camera
// held while it is up. The engine's ECS is a fake that keeps components as plain records.
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
    engine: { RootEntity: 0, addEntity: () => next++, removeEntity: (e: number) => stores.forEach((s) => s.delete(e)) },
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
      fromEulerDegrees: (x: number, y: number, z: number) => ({ euler: { x, y, z } }),
      fromAngleAxis: () => ({ x: 0, y: 0, z: 0, w: 1 }),
      multiply: (a: unknown) => a
    }
  }
})
vi.mock('../../bridge-scene/node_modules/@dcl/sdk/players/index.js', () => ({ getPlayer: () => null }))
vi.mock('../../bridge-scene/node_modules/@dcl/react-ecs/dist/index.js', () => ({ default: { createElement: () => null }, UiEntity: () => null }))
vi.mock('../../bridge-scene/src/domains/avatarDraft', () => ({ currentLook: () => null }))
vi.mock('../../bridge-scene/src/bevy-api', () => ({ BevyApi: {} }))

const LOOK = {
  bodyShape: 'urn:decentraland:off-chain:base-avatars:BaseFemale',
  eyes: { r: 0.1, g: 0.2, b: 0.3 },
  hair: null,
  skin: null,
  wearables: ['urn:decentraland:matic:collections-v2:0xabc:1:42'],
  emotes: ['urn:decentraland:off-chain:base-emotes:clap'],
  forceRender: ['hat']
}
const COLUMN = { x: 135, y: 76, width: 514, height: 928 }
const FINISHED = 4
const FAILED = 3

type Shape = { wearables: string[]; bodyShape?: string; forceRender: string[]; emotes: string[]; eyeColor?: unknown; expressionTriggerId?: string; expressionTriggerTimestamp?: number }

async function stage() {
  vi.resetModules()
  ecs.reset()
  const { registerAvatarPreview } = await import('../../bridge-scene/src/domains/avatarPreview')
  const { registerPassportPreview } = await import('../../bridge-scene/src/domains/passportPreview')
  const handlers = new Map<string, Array<(msg: PageToScene) => void>>()
  const systems: ((dt: number) => void)[] = []
  const sent: SceneToPage[] = []
  const ctx = {
    send: (msg: SceneToPage) => void sent.push(msg),
    on: (kind: string, handler: (msg: PageToScene) => void) => handlers.set(kind, [...(handlers.get(kind) ?? []), handler]),
    push: (s: (dt: number) => void) => systems.push(s)
  } as unknown as Ctx
  registerAvatarPreview(ctx)
  registerPassportPreview(ctx)
  const emit = (msg: PageToScene): void => handlers.get(msg.kind)!.forEach((handler) => handler(msg))
  const frames = (count: number, dt = 1 / 60): void => {
    for (let i = 0; i < count; i++) systems.forEach((s) => s(dt))
  }
  const open = (id: number, over: Record<string, unknown> = {}): void =>
    emit({ kind: 'passportPreview', open: { id, look: LOOK, rect: COLUMN, dpr: 1, background: 'overview', wave: true, ...over } } as PageToScene)
  const avatars = (): Array<[number, Shape]> => [...ecs.stores.get('AvatarShape')!.entries()] as unknown as Array<[number, Shape]>
  const cameras = (): Array<[number, { layer: number; width: number; height: number; mode: { $case: string } }]> =>
    [...ecs.stores.get('TextureCamera')!.entries()] as never
  /** The engine's word on the passport avatar's load. */
  const loaded = (state: number): void => {
    const [entity] = avatars().at(-1)!
    ecs.stores.get('AssetLoadLoadingState')!.set(entity, [{ timestamp: 10, currentState: state }] as never)
  }
  const states = () => sent.filter((m) => m.kind === 'passportPreviewState')
  const yaw = (): number => {
    const [entity] = avatars().at(-1)!
    return (ecs.stores.get('Transform')!.get(entity) as { rotation: { euler: { y: number } } }).rotation.euler.y
  }
  return { emit, frames, open, avatars, cameras, loaded, states, yaw }
}

describe('when the page opens the passport avatar', () => {
  let s: Awaited<ReturnType<typeof stage>>
  beforeEach(async () => {
    s = await stage()
  })

  it("should stand an avatar in the profile's look, another player's items included, on a camera of its own", () => {
    s.open(1)
    const [[, shape]] = s.avatars()
    expect(shape).toMatchObject({ bodyShape: LOOK.bodyShape, wearables: LOOK.wearables, forceRender: ['hat'], eyeColor: LOOK.eyes, emotes: [] })
    const [[, camera]] = s.cameras()
    expect(camera.mode.$case).toBe('perspective')
    expect(camera.layer).toBe(14)
    // the column's width of a picture 960 tall on the 928 canvas
    expect([camera.width, camera.height]).toEqual([514, 960])
  })

  it('should tell the page it is ready once the engine has loaded it and its parts are drawn, and wave', () => {
    s.open(1)
    s.frames(30)
    expect(s.states()).toEqual([])
    s.loaded(FINISHED)
    s.frames(44)
    expect(s.states()).toEqual([])
    s.frames(2)
    expect(s.states()).toEqual([{ kind: 'passportPreviewState', id: 1, state: 'ready' }])
    expect(s.avatars()[0][1].expressionTriggerId).toBe('wave')
  })

  it('should not wave when the page does not ask', () => {
    s.open(1, { wave: false })
    s.loaded(FINISHED)
    s.frames(50)
    expect(s.states()).toHaveLength(1)
    expect(s.avatars()[0][1].expressionTriggerId).toBeUndefined()
  })

  it('should tell the page it failed when the load ends in an error, or never ends', () => {
    s.open(1)
    s.loaded(FAILED)
    s.frames(1)
    expect(s.states()).toEqual([{ kind: 'passportPreviewState', id: 1, state: 'failed' }])
    s.open(2)
    s.frames(19, 0.5)
    expect(s.states()).toHaveLength(1)
    s.frames(2, 0.5)
    expect(s.states().at(-1)).toEqual({ kind: 'passportPreviewState', id: 2, state: 'failed' })
  })

  it('should give up on a load after the same time at a low frame rate', () => {
    s.open(1)
    s.frames(290, 1 / 30)
    expect(s.states()).toEqual([])
    s.frames(20, 1 / 30)
    expect(s.states()).toEqual([{ kind: 'passportPreviewState', id: 1, state: 'failed' }])
  })

  it('should keep the avatar and resize its picture when the column moves', () => {
    s.open(1)
    const [[entity]] = s.avatars()
    s.open(1, { rect: { ...COLUMN, width: 257, height: 464 }, dpr: 2 })
    expect(s.avatars()).toHaveLength(1)
    expect(s.avatars()[0][0]).toBe(entity)
    expect([s.cameras()[0][1].width, s.cameras()[0][1].height]).toEqual([514, 960])
  })

  it('should start another passport over, facing front', () => {
    s.open(1)
    s.emit({ kind: 'passportPreviewDrag', id: 1, dx: -10, seconds: 1 / 60 } as PageToScene)
    s.frames(1)
    expect(s.yaw()).not.toBe(180)
    s.open(2)
    expect(s.avatars()).toHaveLength(1)
    expect(s.yaw()).toBe(180)
  })

  it('should take it down when the page closes it', () => {
    s.open(1)
    s.emit({ kind: 'passportPreview', open: null } as PageToScene)
    expect(s.avatars()).toEqual([])
    expect(s.cameras()).toEqual([])
  })
})

describe('when the passport avatar is turned and played', () => {
  it('should turn with a drag, coast, and stop', async () => {
    const s = await stage()
    s.open(1)
    for (let i = 0; i < 30; i++) {
      s.emit({ kind: 'passportPreviewDrag', id: 1, dx: -10, seconds: 1 / 60 } as PageToScene)
      s.frames(1)
    }
    const dragged = s.yaw()
    expect(dragged).toBeGreaterThan(180)
    s.frames(20)
    expect(s.yaw()).toBeGreaterThan(dragged)
    s.frames(120)
    const rest = s.yaw()
    s.frames(10)
    expect(s.yaw()).toBe(rest)
  })

  it('should ignore a drag or an emote meant for another avatar', async () => {
    const s = await stage()
    s.open(2)
    s.emit({ kind: 'passportPreviewDrag', id: 1, dx: -10, seconds: 1 / 60 } as PageToScene)
    s.frames(1)
    s.emit({ kind: 'passportPreviewEmote', id: 1, urn: 'urn:x' } as PageToScene)
    expect(s.yaw()).toBe(180)
    expect(s.avatars()[0][1].expressionTriggerId).toBeUndefined()
  })

  it('should play an emote by its urn, again when asked again', async () => {
    const s = await stage()
    s.open(1)
    s.emit({ kind: 'passportPreviewEmote', id: 1, urn: 'urn:emote:clap' } as PageToScene)
    const first = s.avatars()[0][1].expressionTriggerTimestamp
    s.emit({ kind: 'passportPreviewEmote', id: 1, urn: 'urn:emote:clap' } as PageToScene)
    expect(s.avatars()[0][1].expressionTriggerId).toBe('urn:emote:clap')
    expect(s.avatars()[0][1].expressionTriggerTimestamp).toBeGreaterThan(first!)
  })
})

describe('when the passport opens over another preview', () => {
  it("should take the other stage's camera off while it is up, and put it back after", async () => {
    const s = await stage()
    s.emit({ kind: 'engineViewport', region: 'avatarPreview', rect: { x: 0, y: 0, width: 960, height: 960 }, dpr: 1 } as PageToScene)
    expect(s.cameras().map(([, c]) => c.layer)).toEqual([12])
    s.open(1)
    expect(s.cameras().map(([, c]) => c.layer)).toEqual([14])
    s.emit({ kind: 'passportPreview', open: null } as PageToScene)
    expect(s.cameras().map(([, c]) => c.layer)).toEqual([12])
  })

  const STAGE = { x: 0, y: 0, width: 960, height: 960 }
  const viewport = (s: Awaited<ReturnType<typeof stage>>, region: 'lobby' | 'avatarPreview', rect: typeof STAGE | null): void =>
    s.emit({ kind: 'engineViewport', region, rect, dpr: 1 } as PageToScene)
  const layers = (s: Awaited<ReturnType<typeof stage>>): number[] => s.cameras().map(([, c]) => c.layer).sort((a, b) => a - b)

  it("should hold the Backpack's camera when it opens over the lobby under the passport, and give the lobby its own back", async () => {
    const s = await stage()
    viewport(s, 'lobby', STAGE)
    expect(layers(s)).toEqual([10])
    s.open(1)
    expect(layers(s)).toEqual([14])
    viewport(s, 'avatarPreview', STAGE)
    s.frames(1)
    expect(layers(s)).toEqual([14])
    s.emit({ kind: 'passportPreview', open: null } as PageToScene)
    expect(layers(s)).toEqual([12])
    viewport(s, 'avatarPreview', null)
    expect(layers(s)).toEqual([10])
  })

  it("should hold the lobby's camera when the Backpack closes under the passport, and put only that one back", async () => {
    const s = await stage()
    viewport(s, 'lobby', STAGE)
    viewport(s, 'avatarPreview', STAGE)
    expect(layers(s)).toEqual([12])
    s.open(1)
    expect(layers(s)).toEqual([14])
    viewport(s, 'avatarPreview', null)
    s.frames(1)
    expect(layers(s)).toEqual([14])
    s.emit({ kind: 'passportPreview', open: null } as PageToScene)
    expect(layers(s)).toEqual([10])
  })
})
