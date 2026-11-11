// The Invite to Community submenu: when a person's menu opens, the communities the user owns or
// moderates that the person is not in are read; the option shows only when there are some, and
// picking one invites the person to it.
// Shared by the profile card and the passport.

import { useEffect } from 'react'
import type { ContextMenuItem } from '../../design'
import { useSession } from '../session/SessionContext'

export const INVITE_TO_COMMUNITY = 'Invite to Community'

export function InviteToCommunityIcon(): React.JSX.Element {
  return (
    <svg viewBox="0 0 24 24" width="20" height="20" fill="none" aria-hidden="true">
      <circle cx="8" cy="9" r="2.6" stroke="currentColor" strokeWidth="1.7" />
      <circle cx="15" cy="8" r="2.2" stroke="currentColor" strokeWidth="1.7" />
      <path d="M3 18c0-2.6 2.2-4.2 5-4.2s5 1.6 5 4.2M13.5 13.2c.5-.1 1-.2 1.5-.2 1.4 0 2.6.4 3.4 1.1" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" />
      <path d="M19 16v5M16.5 18.5h5" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" />
    </svg>
  )
}

/** The submenu's items for a person: one per community the user may invite them to, empty while
 *  reading or when there are none. Nothing is read when `enabled` is false (the user's own menu, a
 *  guest, or a host that has no invite). */
export function useInviteToCommunityItems(address: string, enabled: boolean): ContextMenuItem[] {
  const { communities, profile } = useSession()
  const { loadInvitable, invite } = communities
  const guest = profile.data?.isGuest !== false
  const read = enabled && !guest && address !== ''
  useEffect(() => {
    if (read) loadInvitable(address)
  }, [read, address, loadInvitable])
  const list = read && communities.invitable?.address === address ? communities.invitable.communities : []
  return list.map((c) => ({ label: c.name, onClick: () => invite(c.id, address) }))
}
