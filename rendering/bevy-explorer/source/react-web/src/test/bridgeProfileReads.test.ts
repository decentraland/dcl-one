import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { Ctx } from '../../bridge-scene/src/bridge'
import { expectConsoleOutput } from '@explorer-core/test/console'
import type { GetUserProfileRequest, SceneToPage } from '@explorer-core/engine/protocol'

const api = vi.hoisted(() => ({
  getPlayer: vi.fn(),
  resolveEquippedSet: vi.fn().mockResolvedValue([]),
  resolveEquippedEmotes: vi.fn().mockResolvedValue([]),
  getUserProfile: vi.fn<(address: string) => Promise<unknown>>(),
  getJson: vi.fn<(url: string) => Promise<unknown>>()
}))

vi.mock('../../bridge-scene/src/bevy-api', () => ({ BevyApi: api }))
vi.mock('../../bridge-scene/src/http', () => ({ serviceBase: async () => 'https://badges.test', getJson: api.getJson }))
vi.mock('../../bridge-scene/src/domains/wearables', () => ({ resolveWearables: async () => [], resolveEquippedSet: api.resolveEquippedSet }))
vi.mock('../../bridge-scene/src/domains/emotes', () => ({ resolveEquippedEmotes: api.resolveEquippedEmotes }))
vi.mock('../../bridge-scene/src/domains/avatarDraft', () => ({}))
vi.mock('../../bridge-scene/src/system-helpers', () => ({ relay: () => {} }))
vi.mock('../../bridge-scene/src/identity', () => ({ identity: { onChange: () => {} } }))
vi.mock('../../bridge-scene/node_modules/@dcl/sdk/players/index.js', () => ({ getPlayer: api.getPlayer }))

vi.mock('../../bridge-scene/node_modules/@dcl/sdk/ecs.js', () => ({ engine: {} }))
const { startBridge } = await import('../../bridge-scene/src/bridge')
const { registerProfile } = await import('../../bridge-scene/src/domains/profile')

async function read(request: GetUserProfileRequest): Promise<SceneToPage[]> {
  const sent: SceneToPage[] = []
  const handlers = new Map<string, (msg: GetUserProfileRequest) => Promise<void>>()
  registerProfile({
    on: (kind: string, handler: (msg: GetUserProfileRequest) => Promise<void>) => handlers.set(kind, handler),
    send: (message: SceneToPage) => sent.push(message),
    push: () => {}
  } as unknown as Ctx)
  await handlers.get(request.kind)?.(request)
  return sent
}

beforeEach(() => {
  api.getUserProfile.mockReset().mockResolvedValue({ name: 'Tester', version: 3 })
  api.getJson.mockReset()
  api.getPlayer.mockReset().mockReturnValue(null)
})

describe('bridge profile reads', () => {
  it('uses committed own equipment even while the live player previews an undeployed look', async () => {
    api.getPlayer.mockReturnValue({ userId: '0xABC', wearables: ['optimistic-hat'], emotes: ['optimistic-emote'] })
    api.getUserProfile.mockResolvedValue({ name: 'Tester', avatar: {
      wearables: ['committed-shirt'], emotes: [{ urn: 'committed-emote', slot: 5 }]
    } })
    await read({ kind: 'getUserProfile', address: '0xabc', requestId: 19, extras: true })
    expect(api.resolveEquippedSet).toHaveBeenLastCalledWith(['committed-shirt'], { shopUrls: true, dropFailed: true })
    expect(api.resolveEquippedEmotes).toHaveBeenLastCalledWith([{ urn: 'committed-emote', slot: 5 }])
  })

  it('echoes the request ID on the passport and the badge tiers that follow it', async () => {
    api.getJson
      .mockResolvedValueOnce({ data: { achieved: [{ id: 'badge', name: 'Badge', isTier: true }] } })
      .mockResolvedValueOnce({ data: { tiers: [{ tierId: 'tier', tierName: 'Tier' }] } })
    const replied = await read({ kind: 'getUserProfile', address: '0xabc', requestId: 17, extras: true })
    expect(replied).toMatchObject([
      { kind: 'userProfile', address: '0xabc', requestId: 17, profile: { badges: [{ id: 'badge' }] } },
      { kind: 'userProfile', address: '0xabc', requestId: 17, profile: { badges: [{ id: 'badge', tiers: [{ id: 'tier' }] }] } }
    ])
  })

  it('echoes the request ID even if a profile is unavailable', async () => {
    api.getUserProfile.mockRejectedValueOnce(new Error('missing'))
    expect(await read({ kind: 'getUserProfile', address: '0xabc', requestId: 42 })).toEqual([
      { kind: 'userProfile', address: '0xabc', requestId: 42, profile: null }
    ])
  })

  it('reports a thrown profile read with its address and request ID', () => {
    const failure = new Error('profile read failed')
    expectConsoleOutput('error', [['[bridge] getUserProfile failed', failure]])
    const channel = {
      postMessage: vi.fn(),
      onmessage: undefined as ((event: { data: unknown }) => void) | undefined
    }
    vi.stubGlobal('BroadcastChannel', function () { return channel })
    try {
      startBridge((ctx) => ctx.on('getUserProfile', () => { throw failure }))
      channel.onmessage?.({ data: { to: 'scene', msg: { kind: 'getUserProfile', address: '0xabc', requestId: 17 } } })
      expect(channel.postMessage).toHaveBeenCalledWith({
        to: 'page',
        msg: {
          kind: 'requestFailed', request: 'getUserProfile', error: String(failure),
          profileRead: { address: '0xabc', requestId: 17 }
        }
      })
    } finally {
      vi.unstubAllGlobals()
    }
  })

  it('still answers a read from a HUD that sends no request ID', async () => {
    expect(await read({ kind: 'getUserProfile', address: '0xabc' })).toMatchObject([
      { kind: 'userProfile', address: '0xabc', profile: { version: 3 } }
    ])
  })
})
