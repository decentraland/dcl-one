// Community membership roles, as the social-api names them.

import type { CommunityMemberAction } from '../../engine/protocol'

export function isMemberRole(role: string): boolean {
  return role === 'owner' || role === 'moderator' || role === 'member'
}

/** The role shown beside a member, or null for a plain member. */
export function roleLabel(role: string | undefined): string | null {
  if (role === 'owner') return 'Owner'
  if (role === 'moderator') return 'Moderator'
  return null
}

/** One entry of the member menu's owner/moderator section; `needsName` greys a transfer to a
 *  member without a claimed NAME. */
export interface MemberActionEntry {
  action: CommunityMemberAction
  needsName?: boolean
}

/** The owner's and moderators' actions on a member, in unity-explorer's order and with its rules
 *  (MembersListView.OnContextMenuButtonClicked): only the owner promotes, demotes and hands over
 *  ownership (to a member or moderator, who must hold a NAME); the owner removes or bans anyone
 *  but the owner, a moderator only plain members. */
export function memberActions(viewerRole: string, member: { role: string; hasClaimedName?: boolean }): MemberActionEntry[] {
  const owner = viewerRole === 'owner'
  const out: MemberActionEntry[] = []
  if (owner && member.role === 'moderator') out.push({ action: 'demote' })
  if (owner && member.role === 'member') out.push({ action: 'promote' })
  if (owner && (member.role === 'member' || member.role === 'moderator')) out.push({ action: 'transfer', needsName: member.hasClaimedName !== true })
  const canRemove = owner ? member.role !== 'owner' : viewerRole === 'moderator' && member.role !== 'owner' && member.role !== 'moderator'
  if (canRemove) out.push({ action: 'kick' }, { action: 'ban' })
  return out
}
