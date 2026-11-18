// The creator's friends, for picking a sale beneficiary. Read through the social service RPC (the
// client the sites profile pages use); the connection is opened for the read and closed right after.
import { config } from '@/data/lib/creators/config'
import { identityOf } from '@/data/lib/creators/auth'

export type Friend = {
  address: string
  name: string
  hasClaimedName: boolean
  avatarUrl: string
}

const PAGE_SIZE = 200

/** Every friend of the signed-in creator, sorted by name. */
export async function fetchFriends(address: string): Promise<Friend[]> {
  const identity = identityOf(address)
  if (!identity) throw new Error('Not signed in')
  const { privateKeyToAccount } = await import('viem/accounts')
  const { privateKey } = identity.ephemeral
  // The RPC client pulls protobuf + websocket code nobody else needs; load it on demand.
  const { createSocialClientV2 } = await import('@dcl/social-rpc-client')
  const client = createSocialClientV2(config.get('SOCIAL_RPC_URL'))
  await client.connect({
    ephemeralIdentity: {
      address: identity.ephemeral.address,
      privateKey,
      publicKey: identity.ephemeral.publicKey ?? privateKeyToAccount(privateKey).publicKey
    },
    expiration: new Date(identity.expiration),
    authChain: identity.authChain as Parameters<typeof client.connect>[0]['authChain']
  })
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
