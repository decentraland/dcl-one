// Community chat channels: how they are named. Shared by the HUD and the bridge scene (pure).
// A community's channel is also its data-packet topic on the engine's private chat room and the
// key its stored history is read by, so every layer spells it the same way.

export const COMMUNITY_PREFIX = 'community:'

/** The channel of a community: `community:<id>`, the id lowercased. */
export function communityChannel(id: string): string {
  return `${COMMUNITY_PREFIX}${id.toLowerCase()}`
}

export function isCommunityChannel(channel: string): boolean {
  return channel.startsWith(COMMUNITY_PREFIX) && channel.length > COMMUNITY_PREFIX.length
}

/** The community id a channel names, lowercased, or null for any other channel. */
export function communityIdOf(channel: string): string | null {
  return isCommunityChannel(channel) ? channel.slice(COMMUNITY_PREFIX.length).toLowerCase() : null
}
