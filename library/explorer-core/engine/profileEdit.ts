// What a profile edit looks like once it lands — shared by the session (optimistic update while
// the deploy is in flight) and the mock bridge (its stand-in for the catalyst). Lives beside the
// protocol rather than in features/: it describes the wire's semantics, not a screen.

import type { Profile, ProfileInfo, SaveProfileRequest } from './protocol'

/** What the protocol appends to a non-claimed name: `#` and the last four characters of the address (the
 *  engine builds nametags the same way, `crates/avatar`); empty without an address. */
export const addressSuffix = (address: string): string => (address === '' ? '' : `#${address.slice(-4)}`)

/** A name that isn't one of the wallet's claimed NAMEs is shown with four hex digits of the
 *  address appended — the engine builds nametags the same way (`crates/avatar`) and the bridge
 *  answers with the same, so the passport must not flash a bare name in between. */
export function applyProfileEdit(profile: Profile, edit: Omit<SaveProfileRequest, 'kind'>, ownedNames: readonly string[]): Profile {
  const next = { ...profile }
  if (edit.name) {
    const name = edit.name
    const claimed = ownedNames.some((n) => n.toLowerCase() === name.toLowerCase())
    next.name = claimed ? name : `${name}${addressSuffix(profile.address)}`
    next.hasClaimedName = claimed
  }
  if (edit.description !== undefined) next.description = edit.description.trim() || undefined
  if (edit.links !== undefined) next.links = edit.links.length > 0 ? edit.links : undefined
  const changes = edit.infoPatch ?? edit.info
  if (changes !== undefined) {
    const info: ProfileInfo = edit.infoPatch !== undefined ? { ...profile.info } : {}
    for (const [key, value] of Object.entries(changes) as [keyof ProfileInfo, string | undefined][]) {
      if (value === '') delete info[key]
      else if (value !== undefined) info[key] = value
    }
    next.info = Object.keys(info).length > 0 ? info : undefined
  }
  if (edit.nameColor !== undefined) next.nameColor = edit.nameColor
  return next
}
