// The autopilot's guest: the only identity a benchmark run signs in with, never an account stored in
// this profile. The shell makes it itself, with no network call, and keeps its root key in this
// profile; every run signs a fresh identity with that key, so the account, its address and its
// profile, never changes. The identity is a delegation like any sign-in's (it lasts 30 days) and is
// held for the page, not stored, so a run replaces nothing the profile holds.
import { secp256k1 } from '@noble/curves/secp256k1.js'
import { bytesToHex, hexToBytes } from '@noble/hashes/utils.js'
import { newEphemeralDelegation, publicKeyAddress, signedIdentity } from '../features/auth/guest'
import type { AuthIdentity } from '../features/auth/sso'
import { personalSign } from './sign'

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
