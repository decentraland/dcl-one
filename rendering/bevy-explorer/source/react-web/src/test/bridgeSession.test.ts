import { readFileSync } from 'node:fs'
import { URL as NodeURL } from 'node:url'
import { runInNewContext } from 'node:vm'
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
vi.mock('../../bridge-scene/node_modules/@dcl/sdk/players/index.js', () => ({ getPlayer: () => player }))
vi.mock('../../bridge-scene/src/bevy-api', () => ({
  BevyApi: {
    kernelFetch: vi.fn(async () => ({ ok: true, body: '{"data":{"notifications":[]}}' })),
    getRealmProvider: vi.fn(async () => 'https://peer.decentraland.zone'),
    loginPrevious: vi.fn(async () => ({ success: true, error: '' })),
    logout: vi.fn(),
    getSceneLoadingUIStream: vi.fn(() => new Promise(() => {}))
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

describe('native bridge service configuration', () => {
  it('the native channel gets its service map only from the engine frame, never a page envelope', async () => {
    const source = readFileSync(new NodeURL('../../../crates/dcl_deno/src/js/modules/init.js', import.meta.url), 'utf8')
    const start = source.indexOf('globalThis.BroadcastChannel =')
    const transport = source.slice(start, source.indexOf('\n};', start) + 3)
    const fake = {to:'scene', msg:{kind:'nativeHudServices', bases:{events:'https://bad.invalid/events'}}}
    const real = {to:'nativeHudServices', bases:{events:'https://gateway.example.org/events'}}
    const queue = [JSON.stringify(fake), JSON.stringify(real), '']
    const sandbox = {denoRef:{core:{ops:{
      op_get_bridge_stream: async () => 1,
      op_read_bridge_stream: async () => queue.shift()
    }}}}
    const channel = runInNewContext(transport + '\nnew BroadcastChannel("bevy-ui-bridge")', sandbox)
    channel.onmessage = vi.fn()
    expect(await channel.nativeHudServices).toEqual(real.bases)
    expect(channel.onmessage).toHaveBeenCalledWith({data:fake})
  })

  it('uses the engine stream bootstrap for signed notifications and explicit catalyst overrides', async () => {
    const { startBridge } = await import('../../bridge-scene/src/bridge')
    const { registerCommunities } = await import('../../bridge-scene/src/domains/communities')
    const { registerNotifications } = await import('../../bridge-scene/src/domains/notifications')
    const { catalystBase } = await import('../../bridge-scene/src/http')
    const { BevyApi } = await import('../../bridge-scene/src/bevy-api')
    let active!: Channel
    class Channel {
      nativeHudServices = Promise.resolve({
        notifications: 'https://gateway.example.org/notifications',
        events: 'https://example.org/mounted/events',
        socialApi: 'https://custom.example/social',
        reels: 'https://example.org/mounted/camera-reel-service',
        catalyst: 'https://custom.example/peer'
      })
      onmessage: ((e: {data: unknown}) => void) | null = null
      constructor() { active = this }
      postMessage(): void {}
    }
    vi.stubGlobal('BroadcastChannel', Channel)
    try {
      startBridge(ctx => { registerNotifications(ctx); registerCommunities(ctx) })
      active.onmessage!({data:{to:'scene',msg:{kind:'getNotifications'}}})
      await vi.waitFor(() => expect(BevyApi.kernelFetch).toHaveBeenCalledWith(expect.objectContaining({
        url:'https://gateway.example.org/notifications/notifications?limit=50&from=0'
      })))
      active.onmessage!({data:{to:'scene',msg:{kind:'getCommunityDetail', id:'some/id'}}})
      for (const url of [
        'https://example.org/mounted/events/api/events?community_id=some%2Fid&list=upcoming',
        'https://custom.example/social/v2/communities/some%2Fid/members?limit=100',
        'https://example.org/mounted/camera-reel-service/api/communities/some%2Fid/images?limit=30'
      ]) {
        await vi.waitFor(() => expect(BevyApi.kernelFetch).toHaveBeenCalledWith(expect.objectContaining({url})))
      }
      expect(await catalystBase()).toBe('https://custom.example/peer')
    } finally { vi.unstubAllGlobals() }
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
