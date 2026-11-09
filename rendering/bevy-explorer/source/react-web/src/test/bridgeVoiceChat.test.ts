import { beforeEach, describe, it, expect, vi } from 'vitest'

vi.mock('../../bridge-scene/node_modules/@dcl/sdk/ecs.js', () => ({ engine: { addSystem: vi.fn(), removeSystem: vi.fn() } }))
const player = vi.hoisted(() => ({ current: null as { userId: string; position?: undefined } | null }))
vi.mock('../../bridge-scene/node_modules/@dcl/sdk/players/index.js', () => ({ getPlayer: () => player.current }))
vi.mock('../../bridge-scene/src/system-helpers', () => ({ relay: vi.fn() }))
vi.mock('~system/Runtime', () => ({ getRealm: vi.fn(async () => ({})) }))
const { kernelFetch } = vi.hoisted(() => ({ kernelFetch: vi.fn() }))
vi.mock('../../bridge-scene/src/bevy-api', () => ({
  BevyApi: {
    kernelFetch,
    voiceChat: { action: vi.fn() },
    getServiceUrls: vi.fn(async () => ({ socialApi: 'https://social.example' }))
  }
}))

const { fetchActiveChats, registerVoiceChat } = await import('../../bridge-scene/src/domains/voiceChat')

const chats = (...ids: string[]): { ok: true; body: string } => ({
  ok: true,
  body: JSON.stringify({ data: { activeChats: ids.map((communityId) => ({ communityId })) } })
})

/** The domain on a fake bridge: what it sent, its handlers and its per-frame system. */
function bridge(): { sent: { kind: string; chats?: { communityId: string }[] }[]; on: Map<string, () => void>; frame: () => void } {
  const sent: { kind: string; chats?: { communityId: string }[] }[] = []
  const on = new Map<string, () => void>()
  let system: (dt: number) => void = () => undefined
  registerVoiceChat({
    send: (msg) => sent.push(msg as never),
    on: (kind, handler) => on.set(kind, handler as () => void),
    push: (s) => {
      system = s
    }
  })
  return { sent, on, frame: () => system(0) }
}

const lists = (sent: { kind: string; chats?: { communityId: string }[] }[]): string[][] =>
  sent.filter((m) => m.kind === 'activeCommunityVoiceChats').map((m) => (m.chats ?? []).map((c) => c.communityId))

const settle = async (): Promise<void> => {
  for (let i = 0; i < 10; i++) await Promise.resolve()
}

describe('active community streams', () => {
  beforeEach(() => kernelFetch.mockReset())
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

  it('a failed read says nothing about the streams', async () => {
    vi.spyOn(console, 'error').mockImplementation(() => undefined)
    kernelFetch.mockResolvedValueOnce({ ok: false, status: 500, body: '' })
    expect(await fetchActiveChats()).toBeNull()
  })

  it('keeps the last list when a read fails, and only the latest of overlapping reads', async () => {
    vi.spyOn(console, 'error').mockImplementation(() => undefined)
    player.current = { userId: '0xa' }
    const b = bridge()
    kernelFetch.mockResolvedValueOnce(chats('c1'))
    b.frame() // the sign-in read
    await settle()
    expect(lists(b.sent).at(-1)).toEqual(['c1'])

    kernelFetch.mockResolvedValueOnce({ ok: false, status: 500, body: '' })
    b.on.get('hello')?.()
    await settle()
    expect(lists(b.sent).at(-1)).toEqual(['c1'])

    // the older read answers last: its stale list is dropped
    let older: (v: unknown) => void = () => undefined
    kernelFetch.mockReturnValueOnce(new Promise((resolve) => (older = resolve)))
    kernelFetch.mockResolvedValueOnce(chats('c2'))
    b.on.get('hello')?.()
    b.on.get('hello')?.()
    await settle()
    older(chats('c1', 'c3'))
    await settle()
    expect(lists(b.sent).at(-1)).toEqual(['c2'])
  })

  it('reads nothing while signed out', async () => {
    player.current = null
    const b = bridge()
    b.frame()
    b.on.get('hello')?.()
    await settle()
    expect(kernelFetch).not.toHaveBeenCalled()
    expect(lists(b.sent)).toEqual([])
  })

  it('reads the list on sign-in and clears it on logout', async () => {
    player.current = null
    const b = bridge()
    b.frame()
    expect(kernelFetch).not.toHaveBeenCalled()

    kernelFetch.mockResolvedValueOnce(chats('c1'))
    player.current = { userId: '0xa' }
    b.frame()
    await settle()
    expect(lists(b.sent).at(-1)).toEqual(['c1'])

    player.current = null
    b.frame()
    expect(lists(b.sent).at(-1)).toEqual([])
    expect(b.sent.at(-1)).toEqual({ kind: 'sceneVoiceChat', chat: null })
  })
})
