// The creator's friends, for picking a sale beneficiary. Read through the social service RPC (the
// client the sites profile pages use); the connection is opened for the read and closed right after.
import { config } from '~/config'
import { type Session } from '~/lib/auth'

export type Friend = {
  address: string
  name: string
  hasClaimedName: boolean
  avatarUrl: string
}

const PAGE_SIZE = 200

/** Every friend of the signed-in creator, sorted by name. */
export async function fetchFriends(identity: Session['identity']): Promise<Friend[]> {
  // The RPC client pulls protobuf + websocket code nobody else needs; load it on demand.
  const { createSocialClientV2 } = await import('@dcl/social-rpc-client')
  const client = createSocialClientV2(config.get('SOCIAL_RPC_URL'))
  await client.connect(identity)
  try {
    const friends: Friend[] = []
    for (let offset = 0; ; offset += PAGE_SIZE) {
      const page = await client.getFriends({ limit: PAGE_SIZE, offset })
      for (const friend of page.friends) {
        friends.push({
          address: friend.address.toLowerCase(),
          name: friend.name,
          hasClaimedName: friend.hasClaimedName,
          avatarUrl: friend.profilePictureUrl
        })
      }
      if (page.friends.length < PAGE_SIZE) break
    }
    return friends.sort((a, b) => a.name.localeCompare(b.name))
  } finally {
    client.disconnect()
  }
}
