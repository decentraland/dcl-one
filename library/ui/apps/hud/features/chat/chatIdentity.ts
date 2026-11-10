// Who a chat line, tab or member row is: names, colours and the system sender, shared by the
// chat panel's pieces.

import { displayName } from '@explorer-core/engine/mention'
import type { NearbyMember } from '@explorer-core/engine/protocol'
import { knownUserColor, useProfile } from '../session/profileStore'

const ADDRESS_RE = /^0x[0-9a-fA-F]{6,}$/

export const SYSTEM_COLOR = 'var(--chat-system)'

export function isSystem(sender: string): boolean {
  return !sender || sender.toLowerCase() === 'system'
}

export function shortAddr(s: string): string {
  return ADDRESS_RE.test(s) ? `${s.slice(0, 6)}…${s.slice(-4)}` : s
}

export function displaySender(sender: string): string {
  if (isSystem(sender)) return 'DCL System'
  return shortAddr(sender)
}

export function memberLabel(m: NearbyMember): string {
  return m.name.trim() ? displayName(m.name, m.address, m.claimed) : shortAddr(m.address)
}

/** Split "Name#a1b2" into the colored base and a dimmer #tag. */
export function splitName(label: string): { base: string; tag: string } {
  const i = label.indexOf('#')
  return i >= 0 ? { base: label.slice(0, i), tag: label.slice(i) } : { base: label, tag: '' }
}

export function senderColor(sender: string, name: string): string {
  if (isSystem(sender)) return SYSTEM_COLOR
  return knownUserColor(sender, name)
}

/** A DM partner's name and face, from the profile store (the address until it answers). */
export function usePartner(address: string): { name: string; picture?: string; claimed: boolean; color: string } {
  const known = useProfile(address)
  const name = known?.name != null && known.name !== '' ? displayName(known.name, address, known.hasClaimedName) : shortAddr(address)
  return { name, picture: known?.picture, claimed: known?.hasClaimedName === true, color: senderColor(address, name) }
}
