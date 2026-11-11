import { beforeEach, describe, it, expect, vi } from 'vitest'
import type { ActiveCommunityVoiceChat } from '../engine/protocol'

vi.mock('../../bridge-scene/node_modules/@dcl/sdk/ecs.js', () => ({ engine: { addSystem: vi.fn(), removeSystem: vi.fn() } }))
const player = vi.hoisted(() => ({ current: null as { userId: string; position?: undefined } | null }))
vi.mock('../../bridge-scene/node_modules/@dcl/sdk/players/index.js', () => ({ getPlayer: () => player.current }))
const { relay } = vi.hoisted(() => ({ relay: vi.fn() }))
vi.mock('../../bridge-scene/src/system-helpers', () => ({ relay }))
const { kernelFetch } = vi.hoisted(() => ({ kernelFetch: vi.fn() }))
vi.mock('../../bridge-scene/src/bevy-api', () => ({
  BevyApi: {
    kernelFetch,
    voiceChat: { action: vi.fn() },
    getServiceUrls: vi.fn(async () => ({ socialApi: 'https://social.example' }))
  }
}))

const { fetchActiveChats, patchActive, registerVoiceChat } = await import('../../bridge-scene/src/domains/voiceChat')
const { streamHere } = await import('../features/voiceChat/streamHere')

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
    expect(b.sent.at(-1)).toEqual({ kind: 'activeCommunityVoiceChats', chats: [], loaded: false })
  })
})

const chat = (communityId: string, places: { positions?: string[]; worlds?: string[] } = {}): ActiveCommunityVoiceChat => ({
  communityId,
  communityName: communityId,
  isMember: true,
  positions: places.positions ?? [],
  worlds: places.worlds ?? [],
  participantCount: 3,
  moderatorCount: 1
})

describe('the stream where the player stands', () => {
  const genesis = chat('g', { positions: ['0,0'] })
  const world = chat('w', { worlds: ['Boedo.dcl.eth'] })

  it('matches the parcel in Genesis City and the world inside a world', () => {
    expect(streamHere([genesis, world], '0,0', 'main')).toBe(genesis)
    expect(streamHere([genesis, world], '5,5', 'main')).toBeNull()
    expect(streamHere([genesis, world], '3,3', 'boedo.dcl.eth')).toBe(world)
  })

  it("never matches a Genesis place from a world's parcels", () => {
    expect(streamHere([genesis], '0,0', 'other.dcl.eth')).toBeNull()
  })

  it('matches nothing before the realm is known', () => {
    expect(streamHere([genesis, world], '0,0', '')).toBeNull()
  })
})

describe('patching the live list from a broadcast', () => {
  const update = (communityId: string, status: 'started' | 'ended') => ({
    communityId,
    status,
    positions: ['1,1'],
    worlds: [],
    isMember: false,
    communityName: 'New',
    communityImage: null
  })

  it('adds a started stream with its creator and drops an ended one', () => {
    const list = [chat('c1')]
    expect(patchActive(list, update('c2', 'started'))).toEqual([
      chat('c1'),
      { communityId: 'c2', communityName: 'New', communityImage: undefined, isMember: false, positions: ['1,1'], worlds: [], participantCount: 1, moderatorCount: 1 }
    ])
    expect(patchActive(list, update('c1', 'ended'))).toEqual([])
  })

  it('keeps a listed stream as read', () => {
    const list = [chat('c1')]
    expect(patchActive(list, update('c1', 'started'))).toBe(list)
  })
})

describe('a stream ending', () => {
  beforeEach(() => {
    kernelFetch.mockReset()
    relay.mockReset()
  })

  it('leaves the live list without a read per broadcast', async () => {
    player.current = { userId: '0xa' }
    kernelFetch.mockResolvedValueOnce({ ok: true, body: JSON.stringify({ data: { activeChats: [chat('c1', { positions: ['0,0'] })] } }) })
    const sent: { kind: string; chats?: { communityId: string }[] }[] = []
    let system: (dt: number) => void = () => undefined
    registerVoiceChat({ send: (msg) => sent.push(msg as never), on: () => undefined, push: (s) => (system = s) })
    const onUpdate = relay.mock.calls.find(([kind]) => kind === 'communityVoiceUpdate')?.[2] as (u: unknown) => void

    system(0) // sign-in read
    await settle()
    expect(lists(sent).at(-1)).toEqual(['c1'])

    onUpdate({ ...update('c1', 'ended'), createdAt: 0n, endedAt: 1n })
    expect(lists(sent).at(-1)).toEqual([])
    expect(kernelFetch).toHaveBeenCalledTimes(1)
  })

  const update = (communityId: string, status: 'started' | 'ended') => ({
    communityId,
    status,
    positions: [] as string[],
    worlds: [] as string[],
    isMember: true,
    communityName: communityId,
    communityImage: null
  })
})
