// The player's invite link: the deployment site's invite page for their wallet, which a new player
// opens to be counted as their referral.

import { siteUrl } from '@hud-host'
import { isAddress } from '@explorer-core/lib/identity'
import type { Profile } from '@explorer-core/engine/protocol'

/** The link INVITE FRIENDS copies, or none for a guest, whose address is never kept, or an
 *  unknown one. */
export function inviteLink(profile: Pick<Profile, 'address' | 'isGuest'> | null): string | undefined {
  if (profile == null || profile.isGuest !== false || !isAddress(profile.address)) return undefined
  return siteUrl(`/invite/${profile.address}`)
}
