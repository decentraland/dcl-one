// Where a friend is, for "Jump to Location". As unity-explorer's online-users provider does
// (ArchipelagoHttpOnlineUsersProvider, then WorldInfoOnlineUsersProviderDecorator): ask the
// Archipelago stats for the wallet's position in Genesis City, and when it is not there, ask the
// worlds server which World it is connected to. A friend found nowhere, or a read that fails, is
// null: unity-explorer then does nothing, and so does the caller. Both reads are the map's, so
// the HUD's mock mode answers them from its fixed data.

import { PARCEL_METERS } from '../map/atlas'
import { fetchConnectedWorld, fetchPeers } from '../map/mapApi'

export type FriendLocation = { kind: 'parcel'; x: number; y: number } | { kind: 'world'; world: string }

export async function locateFriend(address: string, signal?: AbortSignal): Promise<FriendLocation | null> {
  try {
    const wanted = address.toLowerCase()
    const peer = (await fetchPeers(signal, address)).find((p) => p.address.toLowerCase() === wanted)
    // the position in world metres, floored to its parcel as unity-explorer's `ToParcel` does
    if (peer != null) return { kind: 'parcel', x: Math.floor(peer.x / PARCEL_METERS), y: Math.floor(peer.z / PARCEL_METERS) }
    const world = await fetchConnectedWorld(address, signal)
    return world != null ? { kind: 'world', world } : null
  } catch {
    return null
  }
}
