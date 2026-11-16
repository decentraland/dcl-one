// Where a friend is, for "Jump to Location". As unity-explorer's online-users provider does
// (ArchipelagoHttpOnlineUsersProvider, then WorldInfoOnlineUsersProviderDecorator): ask the
// Archipelago stats for the wallet's position in Genesis City, and when it is not there, ask the
// worlds server which World it is connected to. A friend found nowhere, or a read that fails, is
// null: unity-explorer then does nothing, and so does the caller.

import { serviceUrl } from '@hud-host'
import { PARCEL_METERS } from '../map/atlas'

export type FriendLocation = { kind: 'parcel'; x: number; y: number } | { kind: 'world'; world: string }

async function readJson(url: string, signal?: AbortSignal): Promise<unknown> {
  const res = await fetch(url, { signal })
  if (!res.ok) {
    await res.body?.cancel()
    return null
  }
  return res.json()
}

/** The friend's parcel in Genesis City (`/comms/peers?id=`): the position in world metres, floored
 *  to its parcel as unity-explorer's `ToParcel` does. */
async function genesisParcel(address: string, signal?: AbortSignal): Promise<FriendLocation | null> {
  const body = (await readJson(`${serviceUrl('archipelagoStats')}/comms/peers?id=${encodeURIComponent(address)}`, signal)) as { peers?: unknown } | null
  const peers = Array.isArray(body?.peers) ? body.peers : []
  const wanted = address.toLowerCase()
  for (const peer of peers) {
    const { address: a, position } = (peer ?? {}) as { address?: unknown; position?: unknown }
    if (typeof a !== 'string' || a.toLowerCase() !== wanted || !Array.isArray(position) || position.length < 3) continue
    const x = Math.floor(Number(position[0]) / PARCEL_METERS)
    const y = Math.floor(Number(position[2]) / PARCEL_METERS)
    if (Number.isFinite(x) && Number.isFinite(y)) return { kind: 'parcel', x, y }
  }
  return null
}

/** The World the friend is connected to (`/wallet/{wallet}/connected-world`; 404 when none). */
async function connectedWorld(address: string, signal?: AbortSignal): Promise<FriendLocation | null> {
  const body = (await readJson(`${serviceUrl('worldsServer')}/wallet/${encodeURIComponent(address)}/connected-world`, signal)) as { world?: unknown } | null
  const world = body?.world
  return typeof world === 'string' && world !== '' ? { kind: 'world', world } : null
}

export async function locateFriend(address: string, signal?: AbortSignal): Promise<FriendLocation | null> {
  try {
    return (await genesisParcel(address, signal)) ?? (await connectedWorld(address, signal))
  } catch {
    return null
  }
}
