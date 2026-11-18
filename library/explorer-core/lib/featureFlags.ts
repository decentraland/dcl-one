// The Explorer's rollout flags: the Unleash application `explorer` as Decentraland's feature-flags
// service publishes it (`https://feature-flags.<base domain>/explorer.json`). The Unity Explorer reads
// the same file; the service has no application of this client's own, so its flags are told apart
// by name.
//
// - This client's flags are named `explorer-bevy-<flag>` in Unleash and read as `<flag>`. A flag
//   both clients share keeps the Unity Explorer's name, `explorer-<flag>`, and is read only when
//   FLAG lists it with `shared()`. Every other key in the file is ignored.
// - `X-Address-Hash` carries a stable anonymous id so gradual rollouts stick to the player: a
//   random one minted here on first launch and kept in prefs. It is this client's alone: neither
//   the sign-in address nor an id a link carries is sent, so a shared link cannot put its
//   visitors in one rollout bucket.
// - Read once per session, after the prefs load (lib/prefs) and without holding up the HUD. A flag
//   is off until the answer says `true`, and stays off if the file can't be read.
// - Only Decentraland's own service is asked (FLAG_SERVICE_DOMAINS). A page whose base domain is
//   any other (a link's `?baseDomain=`, another deployment) sends nothing, so the anonymous id
//   never reaches a host a link chose. The native HUD does not read either: the service refuses its
//   page origin. Both run with every flag off unless `?ff=` forces one.
// - The entry url's `?ff=` overrides the answer, for testing and for running a reported session
//   again: a comma list of `name` (forced on), `name:variant` (forced on with that variant) or
//   `-name` (forced off); `-*` ignores the service entirely (nothing is fetched, every flag not
//   forced on is off). `?ff=-*,photo-mode` runs with only that flag on. Only names in FLAG and
//   short plain variants are kept, so a link cannot grow what analytics records. A link can set
//   it: a flag hides or shows a feature, it is never a permission.
// - Once the read settles the session's flags go to analytics as `Feature Flags Resolved`
//   (`feature_flags`: the flags that are on, as a `?ff=` value that reproduces them;
//   `feature_flags_override`: the entry url's own overrides). The engine keeps the pair for its
//   crash report (crates/analytics/src/crash.rs).
//
// Name a flag in FLAG before reading it, so its uses can be found from one place. React reads them
// through the HUD's useFeatureFlag (library/ui/apps/hud/lib/useFeatureFlag.ts).

import { BASE_DOMAIN, isNativeHud } from '@hud-host'
import { track } from './analytics'
import { getPref, PREF, setPref } from './prefs'

const APP_PREFIX = 'explorer-'
const FLAG_PREFIX = 'explorer-bevy-'

/** The Unity Explorer's flags this client reads too, by their name without `explorer-`. */
const SHARED = new Set<string>()

/** Marks a FLAG entry as a flag shared with the Unity Explorer: `shared('x')` reads `explorer-x`. */
export function shared<const T extends string>(name: T): T {
  SHARED.add(name.toLowerCase())
  return name
}

/** Every flag the HUD reads: a plain name is this client's own (`explorer-bevy-<name>`), a
 *  `shared()` one is the Unity Explorer's (`explorer-<name>`). */
export const FLAG = {
  /** A flag with no feature behind it, for checking the read end to end. */
  test: 'test'
} as const satisfies Record<string, string>

export type Flag = (typeof FLAG)[keyof typeof FLAG]

/** FLAG's names as the service's keys and `?ff=` are compared: lowercase. */
function knownFlags(): ReadonlySet<string> {
  return new Set(Object.values(FLAG).map((name) => name.toLowerCase()))
}

/** A variant name that survives a `?ff=` list, a url query and an analytics field unchanged. */
const SAFE_VARIANT = /^[A-Za-z0-9._-]{1,64}$/

/** The base domains whose feature-flags service this client asks. */
const FLAG_SERVICE_DOMAINS: readonly string[] = ['decentraland.org', 'decentraland.zone']

/** The entry url param that overrides the service's answer. */
const FLAG_OVERRIDE_PARAM = 'ff'

/** The analytics event carrying the session's flags; the engine reads it too (page_events.rs). */
const FLAGS_RESOLVED_EVENT = 'Feature Flags Resolved'

export interface FlagOverrides {
  /** Flags forced on, each with the variant it is forced to or null. */
  on: ReadonlyMap<string, string | null>
  off: ReadonlySet<string>
  /** `-*`: the service is not asked, and every flag not forced on is off. */
  ignoreAll: boolean
}

/** The `?ff=` overrides in a query string; repeated params add up, a name's last mention wins.
 *  Only names in `known` (FLAG's) are kept, and a variant only when it is a SAFE_VARIANT. */
export function parseFlagOverrides(search: string, known: ReadonlySet<string> = knownFlags()): FlagOverrides {
  const on = new Map<string, string | null>()
  const off = new Set<string>()
  let ignoreAll = false
  for (const value of new URLSearchParams(search).getAll(FLAG_OVERRIDE_PARAM)) {
    for (const raw of value.split(',')) {
      const item = raw.trim()
      if (item === '-*') ignoreAll = true
      else if (item.startsWith('-')) {
        const name = item.slice(1).toLowerCase()
        if (!known.has(name)) continue
        off.add(name)
        on.delete(name)
      } else {
        const colon = item.indexOf(':')
        const name = (colon < 0 ? item : item.slice(0, colon)).toLowerCase()
        if (!known.has(name)) continue
        const variant = colon < 0 ? '' : item.slice(colon + 1)
        on.set(name, SAFE_VARIANT.test(variant) ? variant : null)
        off.delete(name)
      }
    }
  }
  return { on, off, ignoreAll }
}

// Read from the entry url once, before the engine's url sync can rewrite the query.
let overrides = parseFlagOverrides(typeof location === 'undefined' ? '' : location.search)

/** Replaces the entry url's overrides, for tests. */
export function setFlagOverrides(next: FlagOverrides): void {
  overrides = next
  notify()
}

/** A flag as `?ff=` forces it, or undefined when the url leaves it to the service. */
export function featureFlagOverride(flag: Flag): boolean | undefined {
  const name = flag.toLowerCase()
  if (overrides.on.has(name)) return true
  if (overrides.off.has(name) || overrides.ignoreAll) return false
  return undefined
}

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
  return `https://feature-flags.${baseDomain}/explorer.json`
}

/** The anonymous id rollouts are bucketed by: minted on first launch and kept, so a player stays
 *  in the same bucket from one session to the next. */
export function featureFlagsUserId(): string {
  const kept = getPref(PREF.featureFlagsUserId)
  if (kept != null && kept !== '') return kept
  const minted = crypto.randomUUID()
  setPref(PREF.featureFlagsUserId, minted)
  return minted
}

/** A key's flag name: this client's own without its prefix, a shared one without the application's,
 *  or null for a key this client does not read. */
function ownFlag(key: string): string | null {
  const lower = key.toLowerCase()
  if (lower.startsWith(FLAG_PREFIX)) return lower.slice(FLAG_PREFIX.length) || null
  if (!lower.startsWith(APP_PREFIX)) return null
  const name = lower.slice(APP_PREFIX.length)
  return SHARED.has(name) ? name : null
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

/** The flags that are on, as a `?ff=` value that reproduces them whatever the service answers
 *  later: `-*`, then each flag on, with its variant when one is enabled. */
export function featureFlagsString(): string {
  const on = new Map<string, string | null>()
  if (!overrides.ignoreAll) {
    for (const [name, enabled] of Object.entries(answer?.flags ?? {})) {
      if (!enabled || overrides.off.has(name)) continue
      const variant = answer?.variants[name]
      on.set(name, variant?.enabled === true ? variant.name : null)
    }
  }
  for (const [name, variant] of overrides.on) on.set(name, variant ?? on.get(name) ?? null)
  return ['-*', ...[...on].sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)).map(flagItem)].join(',')
}

/** The entry url's overrides as a `?ff=` value, empty without any. */
export function featureFlagOverridesString(): string {
  return [...(overrides.ignoreAll ? ['-*'] : []), ...[...overrides.on].map(flagItem), ...[...overrides.off].map((name) => `-${name}`)].join(',')
}

// A variant whose name would not come back the same from a pasted `?ff=` is left out.
function flagItem([name, variant]: [string, string | null]): string {
  return variant != null && SAFE_VARIANT.test(variant) ? `${name}:${variant}` : name
}

/** Where the session's flags came from: the service's answer; a read that failed; `?ff=-*`; the
 *  native HUD or a base domain with no service to ask, where nothing is read. */
export type FlagsSource = 'service' | 'unavailable' | 'ignored' | 'native' | 'other-domain'

/** Why this page does not ask the service, or null when it does. */
function notRead(host: { baseDomain: string; native: boolean }): FlagsSource | null {
  if (overrides.ignoreAll) return 'ignored'
  if (host.native) return 'native'
  return FLAG_SERVICE_DOMAINS.includes(host.baseDomain) ? null : 'other-domain'
}

function settle(source: FlagsSource): void {
  settled = true
  notify()
  track(FLAGS_RESOLVED_EVENT, {
    feature_flags: featureFlagsString(),
    feature_flags_override: featureFlagOverridesString(),
    feature_flags_source: source
  })
}

/** Starts the session's one read (after loadPrefs); later calls share it. Never rejects. `host`
 *  is this page's, given by tests. */
export function loadFeatureFlags(host: { baseDomain: string; native: boolean } = { baseDomain: BASE_DOMAIN, native: isNativeHud() }): Promise<void> {
  if (reading != null) return reading
  const skipped = notRead(host)
  if (skipped != null) {
    settle(skipped)
    return (reading = Promise.resolve())
  }
  reading = fetchFeatureFlags(AbortSignal.timeout(READ_TIMEOUT_MS), featureFlagsUrl(host.baseDomain)).then(
    (flags) => {
      answer = flags
      settle('service')
    },
    (e: unknown) => {
      console.warn('[featureFlags] read failed; every flag is off this session', e)
      settle('unavailable')
    }
  )
  return reading
}

/** Holds these flags as the answer, for tests; null goes back to unread. */
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

/** A flag as `?ff=` forces it, else as answered: false until the answer says `true`. */
export function featureFlag(flag: Flag): boolean {
  return featureFlagOverride(flag) ?? answer?.flags[flag.toLowerCase()] === true
}

/** A flag's variant: the one `?ff=name:variant` forces, else as answered; null without one or
 *  when `?ff=` forces the flag off. A forced variant has the answer's payload only when the answer
 *  is that same variant. Unleash names a disabled variant `disabled`. */
export function featureFlagVariant(flag: Flag): FlagVariant | null {
  if (featureFlagOverride(flag) === false) return null
  const name = flag.toLowerCase()
  const answered = answer?.variants[name] ?? null
  const forced = overrides.on.get(name)
  if (forced == null) return answered
  return { name: forced, enabled: true, payload: answered?.name === forced ? answered.payload : null }
}

/** A variant's payload, only when the variant is the named one and enabled. */
export function featureFlagPayload(flag: Flag, variant: string): FlagPayload | null {
  const v = featureFlagVariant(flag)
  return v != null && v.enabled && v.name === variant ? v.payload : null
}
