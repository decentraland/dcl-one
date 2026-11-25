// "Jump to Location": travel to where a friend is, as unity-explorer's
// FriendListSectionUtilities.JumpToFriendLocation does. Offered only for a friend who is not
// offline. Their parcel in Genesis City is a Genesis teleport (with a realm change first when the
// player is elsewhere); a friend in a World changes realm to that World. A friend found nowhere,
// or a failed read, does nothing. A second jump cancels the one still looking up.

import type { Friend } from '@explorer-core/engine/protocol'
import { locateFriend, type FriendLocation } from '@explorer-core/features/friends/friendLocation'

/** Whether to offer the jump: a friend whose status is not offline. */
export function canJumpToFriend(friends: { list: readonly Friend[] }, address: string): boolean {
  const a = address.toLowerCase()
  const friend = friends.list.find((f) => f.address.toLowerCase() === a)
  return friend != null && friend.status !== 'offline'
}

export interface JumpTravel {
  teleportToPlace: (x: number, y: number) => void
  changeRealm: (realm: string) => void
}

let pending: AbortController | null = null

/** Resolves to where it travelled, or null when it did not (not found, failed, or superseded). */
export async function jumpToFriend(map: JumpTravel, address: string): Promise<FriendLocation | null> {
  pending?.abort()
  const ctl = new AbortController()
  pending = ctl
  const where = await locateFriend(address, ctl.signal)
  if (ctl.signal.aborted) return null
  pending = null
  if (where == null) return null
  if (where.kind === 'parcel') map.teleportToPlace(where.x, where.y)
  else map.changeRealm(where.world)
  return where
}
