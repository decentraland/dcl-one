// Community membership roles, as the social-api names them.

export function isMemberRole(role: string): boolean {
  return role === 'owner' || role === 'moderator' || role === 'member'
}

/** The role shown beside a member, or null for a plain member. */
export function roleLabel(role: string | undefined): string | null {
  if (role === 'owner') return 'Owner'
  if (role === 'moderator') return 'Moderator'
  return null
}
