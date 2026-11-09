import { describe, it, expect, vi } from 'vitest'

vi.mock('../../bridge-scene/node_modules/@dcl/sdk/ecs.js', () => ({ engine: { addSystem: vi.fn(), removeSystem: vi.fn() } }))
vi.mock('../../bridge-scene/node_modules/@dcl/sdk/players/index.js', () => ({ getPlayer: () => null }))
vi.mock('~system/Runtime', () => ({ getRealm: vi.fn(async () => ({})) }))
const { kernelFetch } = vi.hoisted(() => ({ kernelFetch: vi.fn() }))
vi.mock('../../bridge-scene/src/bevy-api', () => ({
  BevyApi: {
    kernelFetch,
    getServiceUrls: vi.fn(async () => ({ socialApi: 'https://social.example' }))
  }
}))

const { fetchActiveChats } = await import('../../bridge-scene/src/domains/voiceChat')

describe('active community streams', () => {
  it('reads the streams out of the social-api envelope', async () => {
    // the answer's shape as captured from social-api (services/catalyst conformance fixture)
    kernelFetch.mockResolvedValueOnce({
      ok: true,
      body: JSON.stringify({
        data: {
          activeChats: [
            { communityId: 'c1', communityName: 'Decentraland Foundation', isMember: true, positions: [], worlds: [], participantCount: 12, moderatorCount: 1 }
          ],
          total: 1
        }
      })
    })
    expect(await fetchActiveChats()).toEqual([
      { communityId: 'c1', communityName: 'Decentraland Foundation', communityImage: undefined, isMember: true, positions: [], worlds: [], participantCount: 12, moderatorCount: 1 }
    ])
    expect(kernelFetch).toHaveBeenCalledWith(expect.objectContaining({ url: 'https://social.example/v1/community-voice-chats/active' }))
  })

  it('a failed read is no streams', async () => {
    vi.spyOn(console, 'error').mockImplementation(() => undefined)
    kernelFetch.mockResolvedValueOnce({ ok: false, status: 500, body: '' })
    expect(await fetchActiveChats()).toEqual([])
  })
})
