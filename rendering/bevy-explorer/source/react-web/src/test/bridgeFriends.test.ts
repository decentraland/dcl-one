import { describe, it, expect, vi, beforeEach } from 'vitest'
import type { Ctx } from '../../bridge-scene/src/bridge'
import type { SceneToPage } from '@explorer-core/engine/protocol'

// The friends domain polls BevyApi.social and pushes a `friends` snapshot. These tests cover the
// blocked-by list: who has blocked the local user, read from the engine's blocking status.
const social = vi.hoisted(() => ({
  getSocialInitialized: vi.fn(async () => true),
  getOnlineFriends: vi.fn(async () => []),
  getReceivedFriendRequests: vi.fn(async () => []),
  getSentFriendRequests: vi.fn(async () => []),
  getBlockedUsers: vi.fn(async () => [] as unknown[]),
  getBlockingStatus: vi.fn(async () => ({ blockedUsers: [] as string[], blockedByUsers: [] as string[] })),
  getFriendshipEventStream: vi.fn(),
  getFriendConnectivityStream: vi.fn(),
  getBlockUpdateStream: vi.fn()
}))
const relays = vi.hoisted(() => new Map<string, () => void>())
vi.mock('../../bridge-scene/src/bevy-api', () => ({ BevyApi: { social } }))
vi.mock('../../bridge-scene/src/system-helpers', () => ({
  relay: (name: string, _open: unknown, onItem: () => void) => relays.set(name, onItem)
}))
vi.mock('../../bridge-scene/node_modules/@dcl/sdk/players/index.js', () => ({ getPlayer: () => ({ isGuest: false }) }))

const { registerFriends } = await import('../../bridge-scene/src/domains/friends')

const flush = async (): Promise<void> => {
  for (let i = 0; i < 20; i++) await Promise.resolve()
}

function start(): { sent: SceneToPage[]; poll: () => Promise<void> } {
  const sent: SceneToPage[] = []
  const pushed: Array<(dt: number) => void> = []
  const ctx: Ctx = { send: (m) => sent.push(m), on: () => {}, push: (s) => pushed.push(s) }
  registerFriends(ctx)
  return {
    sent,
    poll: async () => {
      pushed.forEach((s) => s(1))
      await flush()
    }
  }
}

const lastFriends = (sent: SceneToPage[]): Extract<SceneToPage, { kind: 'friends' }> | undefined =>
  sent.filter((m): m is Extract<SceneToPage, { kind: 'friends' }> => m.kind === 'friends').at(-1)

beforeEach(() => {
  vi.clearAllMocks()
  relays.clear()
})

describe('bridge friends: blocked-by', () => {
  it('sends who blocked the local user, lowercased', async () => {
    social.getBlockingStatus.mockResolvedValueOnce({ blockedUsers: [], blockedByUsers: ['0xABC'] })
    const { sent, poll } = start()
    await poll()
    expect(lastFriends(sent)?.blockedBy).toEqual(['0xabc'])
  })

  it('refreshes it when the block update stream fires', async () => {
    const { sent, poll } = start()
    await poll()
    expect(lastFriends(sent)?.blockedBy).toEqual([])
    social.getBlockingStatus.mockResolvedValueOnce({ blockedUsers: [], blockedByUsers: ['0xdef'] })
    relays.get('blockUpdates')?.()
    await poll()
    expect(lastFriends(sent)?.blockedBy).toEqual(['0xdef'])
  })

  it('keeps the blocked list when the blocking status fails', async () => {
    vi.spyOn(console, 'error').mockImplementation(() => {})
    social.getBlockedUsers.mockResolvedValueOnce([{ address: '0x1', name: 'Bob', profilePictureUrl: '', hasClaimedName: false }])
    social.getBlockingStatus.mockRejectedValueOnce(new Error('social down'))
    const { sent, poll } = start()
    await poll()
    expect(lastFriends(sent)?.blocked).toEqual(['0x1'])
    expect(lastFriends(sent)?.blockedBy).toEqual([])
    // The next poll retries the blocking status alone; the blocked list isn't fetched again.
    social.getBlockingStatus.mockResolvedValueOnce({ blockedUsers: [], blockedByUsers: ['0x2'] })
    await poll()
    expect(lastFriends(sent)?.blockedBy).toEqual(['0x2'])
    expect(social.getBlockedUsers).toHaveBeenCalledTimes(1)
  })
})
