import { describe, it, expect, vi, beforeEach } from 'vitest'
import type { Ctx } from '../../bridge-scene/src/bridge'
import type { PageToScene, SceneToPage } from '../engine/protocol'

// A frame loop shaped like @dcl/ecs: systems run from the live array, which removal splices.
const systems: Array<(dt: number) => void> = []
vi.mock('../../bridge-scene/node_modules/@dcl/sdk/ecs.js', () => ({
  engine: {
    addSystem: (fn: (dt: number) => void) => systems.push(fn),
    removeSystem: (fn: (dt: number) => void) => {
      const i = systems.indexOf(fn)
      if (i >= 0) systems.splice(i, 1)
    }
  }
}))
vi.mock('../../bridge-scene/src/domains/profile', () => ({fetchIdentities: vi.fn(async () => () => ({}))}))
let player: { userId: string } | null = null
const { openExternalUrl } = vi.hoisted(() => ({ openExternalUrl: vi.fn() }))
vi.mock('~system/RestrictedActions', () => ({ openExternalUrl }))
vi.mock('../../bridge-scene/node_modules/@dcl/sdk/players/index.js', () => ({ getPlayer: () => player }))
vi.mock('../../bridge-scene/src/bevy-api', () => ({
  BevyApi: {
    kernelFetch: vi.fn(async () => ({ ok: true, body: '{"data":{"notifications":[]}}' })),
    getServiceUrls: vi.fn(async () => ({
      notifications: 'https://gateway.example.org/notifications',
      events: 'https://example.org/mounted/events',
      socialApi: 'https://custom.example/social',
      cameraReel: 'https://example.org/mounted/camera-reel-service',
      reels: 'https://share.example/reels',
      catalyst: 'https://custom.example/peer'
    })),
    loginPrevious: vi.fn(async () => ({ success: true, error: '' })),
    logout: vi.fn(),
    getSceneLoadingUIStream: vi.fn(() => new Promise(() => {})),
    social: { getCommunityConnectivityStream: vi.fn(() => new Promise(() => {})) }
  }
}))

const { registerSession } = await import('../../bridge-scene/src/domains/session')
const { waitMs } = await import('../../bridge-scene/src/system-helpers')

function fakeCtx(): { ctx: Ctx; sent: SceneToPage[]; req: (msg: PageToScene) => Promise<void>; frame: () => void } {
  const sent: SceneToPage[] = []
  const handlers = new Map<string, Array<(m: PageToScene) => unknown>>()
  const pushed: Array<(dt: number) => void> = []
  const ctx: Ctx = {
    send: (m) => sent.push(m),
    on: (kind, h) => handlers.set(kind, [...(handlers.get(kind) ?? []), h as (m: PageToScene) => unknown]),
    push: (s) => pushed.push(s)
  }
  const req = async (msg: PageToScene): Promise<void> => {
    await Promise.all((handlers.get(msg.kind) ?? []).map((h) => h(msg)))
  }
  return { ctx, sent, req, frame: () => pushed.forEach((s) => s(1 / 60)) }
}

it('opens only the fixed support URL through the scene permission boundary', async () => {
  const { ctx, req } = fakeCtx()
  registerSession(ctx)
  await req({ kind: 'navAction', action: 'support' })
  expect(openExternalUrl).toHaveBeenCalledWith({ url: 'https://decentraland.org/help/' })
})

const readies = (sent: SceneToPage[]): number => sent.filter((m) => m.kind === 'event' && m.name === 'playerReady').length

beforeEach(() => {
  systems.length = 0
  player = null
})

describe('bridge session', () => {
  it('logging back into the same account announces playerReady again', async () => {
    const { ctx, sent, req, frame } = fakeCtx()
    registerSession(ctx)
    player = { userId: '0xa' }
    frame()
    expect(readies(sent)).toBe(1)
    await req({ kind: 'rpc:req', id: '1', method: 'logout' })
    frame() // no frame has seen the player gone yet
    expect(readies(sent)).toBe(1)
    await req({ kind: 'rpc:req', id: '2', method: 'loginPrevious' })
    frame()
    expect(readies(sent)).toBe(2)
  })

  it('announces playerReady again once the engine has cleared the player and set the next one', () => {
    const { ctx, sent, frame } = fakeCtx()
    registerSession(ctx)
    player = { userId: '0xa' }
    frame()
    player = null // logout
    frame()
    expect(readies(sent)).toBe(1)
    player = { userId: '0xb' } // a login the bridge did not relay (the web console command)
    frame()
    expect(readies(sent)).toBe(2)
  })
})

describe('bridge service URLs', () => {
  it('uses the engine-resolved APIs for notifications, community detail and profiles', async () => {
    const { registerCommunities } = await import('../../bridge-scene/src/domains/communities')
    const { registerNotifications } = await import('../../bridge-scene/src/domains/notifications')
    const { catalystBase } = await import('../../bridge-scene/src/http')
    const { BevyApi } = await import('../../bridge-scene/src/bevy-api')
    const { ctx, req } = fakeCtx()
    registerNotifications(ctx)
    registerCommunities(ctx)

    await req({ kind: 'getNotifications' })
    await req({ kind: 'getCommunityDetail', id: 'some/id' })
    for (const url of [
      'https://gateway.example.org/notifications/notifications?limit=50&from=0',
      'https://example.org/mounted/events/api/events?community_id=some%2Fid&list=upcoming',
      'https://custom.example/social/v2/communities/some%2Fid/members?limit=100',
      'https://example.org/mounted/camera-reel-service/api/communities/some%2Fid/images?limit=30'
    ]) {
      expect(BevyApi.kernelFetch).toHaveBeenCalledWith(expect.objectContaining({
        url, init: expect.objectContaining({ method: 'GET' })
      }))
    }
    expect(await catalystBase()).toBe('https://custom.example/peer')
    expect(BevyApi.getServiceUrls).toHaveBeenCalledTimes(1)
  })
})

describe('member-of list', () => {
  it('sends nothing for an answer without a results list, and an empty list only for an empty one', async () => {
    const { registerCommunities } = await import('../../bridge-scene/src/domains/communities')
    const { BevyApi } = await import('../../bridge-scene/src/bevy-api')
    const { ctx, req, sent } = fakeCtx()
    registerCommunities(ctx)
    const answer = (body: string): void => {
      vi.mocked(BevyApi.kernelFetch).mockResolvedValueOnce({ ok: true, status: 200, body } as never)
    }
    for (const body of ['', '{}', '{"data":{}}']) {
      answer(body)
      await req({ kind: 'getCommunityChannels' })
    }
    expect(sent.filter((m) => m.kind === 'communityChannels')).toEqual([])
    answer('{"results":[]}')
    await req({ kind: 'getCommunityChannels' })
    expect(sent.filter((m) => m.kind === 'communityChannels')).toEqual([{ kind: 'communityChannels', communities: [] }])
  })
})

describe('waitMs', () => {
  it('does not make other systems skip a frame when it finishes', async () => {
    const other = vi.fn()
    const done = waitMs(10)
    systems.push(other)
    for (const s of systems) s(1)
    await done
    expect(other).toHaveBeenCalledTimes(1)
  })
})
