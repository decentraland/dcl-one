// The autopilot's guest: the only identity a benchmark run signs in with, never an account stored in
// this profile. The shell makes it itself, with no network call, and keeps its root key in this
// profile; every run signs a fresh identity with that key, so the account, its address and its
// profile, never changes. The identity is a delegation like any sign-in's (it lasts 30 days) and is
// held for the page, not stored, so a run replaces nothing the profile holds.
import { secp256k1 } from '@noble/curves/secp256k1.js'
import { bytesToHex, hexToBytes } from '@noble/hashes/utils.js'
import { newEphemeralDelegation, publicKeyAddress, signedIdentity } from '../features/auth/guest'
import type { AuthIdentity } from '@explorer-core/features/auth/sso'
import { personalSign } from '@explorer-core/shell/sign'

// The root key of the autopilot's local guest, which signs each of its identities. Shell-only, like the
// identity's ephemeral key: the app never reads this storage. Kept apart from the sign-ins and from the
// guest account: signing in, signing out or making another guest never removes it, so the benchmark's
// address stays the same.
const LOCAL_GUEST_ROOT_KEY = 'dcl-local-guest-root-key'

/** The local guest's root key, or null when none is kept. A value that is there but is no key throws:
 *  making a new one over it would be a different account. */
export function getLocalGuestRootKey(): string | null {
  const key = localStorage.getItem(LOCAL_GUEST_ROOT_KEY)
  if (key == null) return null
  if (!/^0x[0-9a-f]{64}$/.test(key)) throw new Error('the local guest key kept in this profile is not a key')
  return key
}

export function storeLocalGuestRootKey(key: string): void {
  localStorage.setItem(LOCAL_GUEST_ROOT_KEY, key)
}

/** A new root key, as the 0x-prefixed hex the shell stores. */
export function createLocalGuestRootKey(): string {
  return '0x' + bytesToHex(secp256k1.utils.randomSecretKey())
}

/** The root key's address, in lower case: the identity's SIGNER. */
export function localGuestAddress(rootKey: string): string {
  return publicKeyAddress(secp256k1.getPublicKey(hexToBytes(rootKey.replace(/^0x/, '')), false)).toLowerCase()
}

/** A fresh identity of the account `rootKey` is: a new ephemeral key, delegated by the root key. */
export function localGuestIdentity(rootKey: string): AuthIdentity {
  const delegation = newEphemeralDelegation()
  return signedIdentity(delegation, localGuestAddress(rootKey), personalSign(rootKey, delegation.payload))
}
