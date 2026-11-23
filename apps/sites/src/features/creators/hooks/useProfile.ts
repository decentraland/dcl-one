import { useQueries, useQuery } from '@tanstack/react-query'
import { peerUrl } from '@/data/lib/creators/catalyst'

// The subset of the Catalyst profile the ui2 Navbar `avatar` prop consumes.
export type ProfileAvatar = {
  name?: string
  hasClaimedName?: boolean
  ethAddress?: string
  email?: string
  avatar?: {
    snapshots?: { face256?: string; body?: string }
  }
}

// Returns the first avatar, or null when the wallet has no profile (404) or no avatar in it. Never
// undefined: react-query rejects a query whose data is undefined ("data is undefined"), which turned
// every wallet without a profile into an error. The hooks below hand callers undefined for "no
// profile", the same as "not loaded yet".
async function fetchProfile(address: string): Promise<ProfileAvatar | null> {
  const res = await fetch(`${peerUrl()}/lambdas/profiles/${address.toLowerCase()}`)
  if (!res.ok) {
    await res.body?.cancel()
    if (res.status === 404) return null
    throw new Error(`Could not fetch the profile: the Catalyst answered ${res.status}`)
  }
  const profile = (await res.json()) as { avatars?: ProfileAvatar[] } | null
  return profile?.avatars?.[0] ?? null
}

export const profileQuery = (address: string | undefined) => ({
  queryKey: ['profile', address],
  enabled: !!address,
  staleTime: 5 * 60_000,
  queryFn: (): Promise<ProfileAvatar | null> => fetchProfile(address!)
})

const withoutNull = (profile: ProfileAvatar | null | undefined): ProfileAvatar | undefined => profile ?? undefined

export function useProfile(address?: string) {
  return useQuery({ ...profileQuery(address), select: withoutNull })
}

/** Profiles of several wallets at once, in the same order; entries stay undefined until each one loads. */
export function useProfiles(addresses: string[]): (ProfileAvatar | undefined)[] {
  return useQueries({
    queries: addresses.map(profileQuery),
    combine: results => results.map(result => withoutNull(result.data))
  })
}
