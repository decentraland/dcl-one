// The stored account's name + full-body snapshot, for the screens shown before the engine draws
// the live avatar, and the look itself for the stage to wear. The by-address image redirects to a
// broken S3 path, so we resolve the real (hash-based) body URL from the profile (lambdas send CORS
// → fetch works under credentialless).

import type { AvatarLook } from '../../engine/avatarEquip'
import type { Color3 } from '../../engine/generated'
import { nativeProfileImage, serviceUrl } from '@hud-host'

type LookColor = { color?: Color3 }
interface ProfileResponse {
  avatars?: Array<{
    name?: string
    avatar?: { bodyShape?: string; wearables?: string[]; eyes?: LookColor; hair?: LookColor; skin?: LookColor; snapshots?: { body?: string } }
  }>
}
const color = (c: LookColor | undefined): Color3 | null => (c?.color == null ? null : { r: c.color.r, g: c.color.g, b: c.color.b })

const FETCH_TIMEOUT_MS = 10_000
// a missing profile is asked for again this often, while its wait lasts


export interface StoredProfile {
  name?: string
  body?: string
  /** The profile's look, when it has a body shape. */
  look?: AvatarLook
  /** The catalyst's answer: a profile, or none at the address; unset while unanswered or unreachable. */
  exists?: boolean
  /** The fetch is over, answered or not (useStoredProfile). */
  settled?: boolean
}

/** The profile at `address` (an account's, or a curated default look's id `default<N>`), or nothing
 *  when it can't be fetched. */
export async function fetchStoredProfile(address: string): Promise<StoredProfile | undefined> {
  if (!/^(0x[0-9a-fA-F]{40}|default\d+)$/.test(address)) return undefined
  try {
    // bounded: a stored sign-in waits for this answer (useEngineSession)
    const r = await fetch(`${serviceUrl('catalyst')}/lambdas/profiles/${address}`, { signal: AbortSignal.timeout(FETCH_TIMEOUT_MS) })
    if (r.status === 404) return { exists: false }
    if (!r.ok) throw new Error(`profile fetch failed: ${r.status}`)
    const j = (await r.json()) as ProfileResponse
    const a = j.avatars?.[0]
    const look: AvatarLook | undefined =
      a?.avatar?.bodyShape == null
        ? undefined
        : {
            bodyShape: a.avatar.bodyShape,
            wearables: a.avatar.wearables ?? [],
            emotes: [],
            eyes: color(a.avatar.eyes),
            hair: color(a.avatar.hair),
            skin: color(a.avatar.skin),
            forceRender: []
          }
    return { name: a?.name, body: nativeProfileImage(a?.avatar?.snapshots?.body), look, exists: true }
  } catch {
    return undefined
  }
}
