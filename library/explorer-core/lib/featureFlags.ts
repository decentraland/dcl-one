// The Explorer's rollout flags: the Unleash application `explorer` as Decentraland's feature-flags
// service publishes it (`https://feature-flags.<base domain>/explorer.json`). The Unity Explorer reads
// the same file; the service has no application of this client's own, so its flags are told apart
// by name.
//
// - This client's flags are named `explorer-bevy-<flag>` in Unleash. Only those are kept, the prefix
//   dropped, so code names a flag as `<flag>`; every other key in the file is the Unity Explorer's
//   and is ignored.
// - `X-Address-Hash` carries a stable anonymous id so gradual rollouts stick to the player: the
//   landing page's campaign id when the entry url has one, else one minted here and kept in prefs.
//   The sign-in address is never sent.
// - Read once per session, after the prefs load (lib/prefs) and without holding up the HUD. A flag
//   is off until the answer says `true`, and stays off if the file can't be read.
//
// Name a flag in FLAG before reading it, so its uses can be found from one place. React reads them
// through the HUD's useFeatureFlag (library/ui/apps/hud/lib/useFeatureFlag.ts).

import { BASE_DOMAIN } from '@hud-host'
import { launchAnalytics } from './analytics'
import { getPref, PREF, setPref } from './prefs'

export const FLAGS_APP = 'explorer'
/** The prefix of this client's flags in the file; keys without it are left out. */
export const FLAG_PREFIX = `${FLAGS_APP}-bevy-`

/** Every flag the HUD reads, by its name without the `explorer-bevy-` prefix. */
export const FLAG = {} as const satisfies Record<string, string>

export type Flag = (typeof FLAG)[keyof typeof FLAG]

export interface FlagPayload {
  /** Unleash's payload type: `string`, `json`, `csv` or `number`. */
  type: string
  value: string
}

export interface FlagVariant {
  name: string
  enabled: boolean
  payload: FlagPayload | null
}

export interface FeatureFlags {
  flags: Readonly<Record<string, boolean>>
  variants: Readonly<Record<string, FlagVariant>>
}

const READ_TIMEOUT_MS = 5_000

let answer: FeatureFlags | null = null
let settled = false
let reading: Promise<void> | null = null
const listeners = new Set<() => void>()

function notify(): void {
  for (const l of listeners) l()
}

export function featureFlagsUrl(baseDomain: string = BASE_DOMAIN): string {
  return `https://feature-flags.${baseDomain}/${FLAGS_APP}.json`
}

/** The anonymous id rollouts are bucketed by: the campaign id, else the one this client keeps. */
export function featureFlagsUserId(): string {
  const campaign = launchAnalytics().campaignAnonUserId
  if (campaign != null && campaign.trim() !== '') return campaign
  const kept = getPref(PREF.featureFlagsUserId)
  if (kept != null && kept !== '') return kept
  const minted = crypto.randomUUID()
  setPref(PREF.featureFlagsUserId, minted)
  return minted
}

/** A key's flag name without the prefix, or null for a key that is not this client's. */
function ownFlag(key: string): string | null {
  if (!key.toLowerCase().startsWith(FLAG_PREFIX)) return null
  const name = key.slice(FLAG_PREFIX.length)
  return name === '' ? null : name
}

/** This client's part of the service's file: each flag on only where it says `true`, each variant by its name. */
export function parseFeatureFlags(body: unknown): FeatureFlags {
  const file = (body != null && typeof body === 'object' ? body : {}) as { flags?: unknown; variants?: unknown }
  const flags: Record<string, boolean> = {}
  if (file.flags != null && typeof file.flags === 'object') {
    for (const [key, value] of Object.entries(file.flags)) {
      const name = ownFlag(key)
      if (name != null) flags[name] = value === true
    }
  }
  const variants: Record<string, FlagVariant> = {}
  if (file.variants != null && typeof file.variants === 'object') {
    for (const [key, raw] of Object.entries(file.variants as Record<string, unknown>)) {
      const name = ownFlag(key)
      const v = (raw ?? {}) as { name?: unknown; enabled?: unknown; payload?: { type?: unknown; value?: unknown } | null }
      if (name == null || typeof v.name !== 'string') continue
      const p = v.payload
      variants[name] = {
        name: v.name,
        enabled: v.enabled === true,
        payload: p != null && typeof p.type === 'string' && typeof p.value === 'string' ? { type: p.type, value: p.value } : null
      }
    }
  }
  return { flags, variants }
}

/** One read of the file. `X-Address-Hash` is what Unleash buckets a gradual rollout by. */
export async function fetchFeatureFlags(signal?: AbortSignal, url: string = featureFlagsUrl()): Promise<FeatureFlags> {
  const res = await fetch(url, {
    signal,
    headers: { 'X-Debug': 'false', 'X-Address-Hash': featureFlagsUserId() }
  })
  if (!res.ok) {
    void res.body?.cancel().catch(() => undefined)
    throw new Error(`feature flags: HTTP ${res.status}`)
  }
  return parseFeatureFlags(await res.json())
}

/** Starts the session's one read (after loadPrefs); later calls share it. Never rejects. */
export function loadFeatureFlags(): Promise<void> {
  reading ??= fetchFeatureFlags(AbortSignal.timeout(READ_TIMEOUT_MS)).then(
    (flags) => {
      answer = flags
      settled = true
      notify()
    },
    (e: unknown) => {
      console.warn('[featureFlags] read failed; every flag is off this session', e)
      settled = true
      notify()
    }
  )
  return reading
}

/** Holds these flags as the answer, for tests and the mock HUD; null goes back to unread. */
export function primeFeatureFlags(flags: Partial<FeatureFlags> | null): void {
  reading = flags == null ? null : Promise.resolve()
  answer = flags == null ? null : { flags: { ...flags.flags }, variants: { ...flags.variants } }
  settled = flags != null
  notify()
}

/** Whether the read has finished, with an answer or without one. */
export function featureFlagsSettled(): boolean {
  return settled
}

export function subscribeFeatureFlags(listener: () => void): () => void {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

/** A flag as answered: false until the answer says `true`. */
export function featureFlag(flag: Flag): boolean {
  return answer?.flags[flag] === true
}

/** A flag's variant as answered, or null without one. Unleash names a disabled variant `disabled`. */
export function featureFlagVariant(flag: Flag): FlagVariant | null {
  return answer?.variants[flag] ?? null
}

/** A variant's payload, only when the variant is the named one and enabled. */
export function featureFlagPayload(flag: Flag, variant: string): FlagPayload | null {
  const v = featureFlagVariant(flag)
  return v != null && v.enabled && v.name === variant ? v.payload : null
}
