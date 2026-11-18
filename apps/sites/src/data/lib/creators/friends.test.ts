import { afterEach, describe, expect, it, vi } from 'vitest'
import { type AuthIdentity } from '@dcl/crypto'

const client = { connect: vi.fn(), disconnect: vi.fn(), getFriends: vi.fn() }
vi.mock('@dcl/social-rpc-client', () => ({ createSocialClientV2: vi.fn(() => client) }))

import { createSocialClientV2 } from '@dcl/social-rpc-client'
import { fetchFriends } from './friends'

const identity = { authChain: [] } as unknown as AuthIdentity
const friend = (address: string, name: string) => ({
  address,
  name,
  hasClaimedName: true,
  profilePictureUrl: `${name}.png`
})

afterEach(() => {
  client.connect.mockReset()
  client.disconnect.mockReset()
  client.getFriends.mockReset()
})

describe('fetchFriends', () => {
  it('connects to the social service, pages through every friend, sorts by name and disconnects', async () => {
    const page1 = Array.from({ length: 200 }, (_, i) =>
      friend(`0x${String(i).padStart(40, '0')}`, `Zed${String(i).padStart(3, '0')}`)
    )
    client.getFriends
      .mockResolvedValueOnce({ friends: page1 })
      .mockResolvedValueOnce({ friends: [friend('0xABC', 'Amy')] })

    const friends = await fetchFriends(identity)

    expect(createSocialClientV2).toHaveBeenCalledWith('wss://rpc-social-service-ea.decentraland.zone')
    expect(client.connect).toHaveBeenCalledWith(identity)
    expect(client.getFriends).toHaveBeenNthCalledWith(1, { limit: 200, offset: 0 })
    expect(client.getFriends).toHaveBeenNthCalledWith(2, { limit: 200, offset: 200 })
    expect(friends).toHaveLength(201)
    expect(friends[0]).toEqual({ address: '0xabc', name: 'Amy', hasClaimedName: true, avatarUrl: 'Amy.png' })
    expect(client.disconnect).toHaveBeenCalledTimes(1)
  })

  it('disconnects even when the read fails', async () => {
    client.getFriends.mockRejectedValue(new Error('offline'))
    await expect(fetchFriends(identity)).rejects.toThrow('offline')
    expect(client.disconnect).toHaveBeenCalledTimes(1)
  })
})
