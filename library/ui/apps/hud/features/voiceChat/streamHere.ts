import type { ActiveCommunityVoiceChat } from '@explorer-core/engine/protocol'

/** The stream whose places include where the player is (the first wins): in a world (an ENS name,
 *  `*.eth`), a place in that world; in Genesis City, the parcel (`"x,y"`). A world's parcels are
 *  its own, so they never match a Genesis place. With the realm not known yet, nothing matches. */
export function streamHere(chats: ActiveCommunityVoiceChat[], parcel: string, realm: string): ActiveCommunityVoiceChat | null {
  const name = realm.toLowerCase()
  if (name === '') return null
  if (name.endsWith('.eth')) return chats.find((c) => c.worlds.some((w) => w.toLowerCase() === name)) ?? null
  return chats.find((c) => c.positions.includes(parcel)) ?? null
}
