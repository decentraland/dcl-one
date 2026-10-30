import { describe, expect, it } from 'vitest'
import { secp256k1 } from '@noble/curves/secp256k1.js'
import { hexToBytes } from '@noble/hashes/utils.js'
import { publicKeyAddress } from '../features/auth/guest'
import { createLocalGuestRootKey, localGuestAddress, localGuestIdentity } from '../shell/localGuest'
import { recoverSigner as signer } from './recoverSigner'

describe('the autopilot guest (shell/localGuest.ts)', () => {
  it('makes a root key of its own each time', () => {
    const a = createLocalGuestRootKey()
    expect(a).toMatch(/^0x[0-9a-f]{64}$/)
    expect(createLocalGuestRootKey()).not.toBe(a)
    expect(localGuestAddress(a)).toMatch(/^0x[0-9a-f]{40}$/)
  })

  it('signs an identity as the account the root key is: the root delegates a fresh ephemeral key for 30 days', () => {
    const rootKey = createLocalGuestRootKey()
    const identity = localGuestIdentity(rootKey)
    const root = localGuestAddress(rootKey)

    const [signerLink, delegate] = identity.authChain
    expect(signerLink).toEqual({ type: 'SIGNER', payload: root, signature: '' })
    const { address, privateKey } = identity.ephemeralIdentity
    expect(address).toBe(publicKeyAddress(secp256k1.getPublicKey(hexToBytes(privateKey?.slice(2) ?? ''), false)))
    expect(delegate.type).toBe('ECDSA_EPHEMERAL')
    expect(delegate.payload).toBe(`Decentraland Login\nEphemeral address: ${address}\nExpiration: ${identity.expiration}`)
    // what the delegate link says the root signed, the root did
    expect(signer(delegate.payload, delegate.signature)).toBe(root)
    const days = (new Date(identity.expiration).getTime() - Date.now()) / 86_400_000
    expect(days).toBeGreaterThan(29.9)
    expect(days).toBeLessThanOrEqual(30)
  })

  it('signs again for the same account with a new ephemeral key', () => {
    const rootKey = createLocalGuestRootKey()
    const first = localGuestIdentity(rootKey)
    const second = localGuestIdentity(rootKey)
    expect(second.authChain[0]).toEqual(first.authChain[0])
    expect(second.ephemeralIdentity.address).not.toBe(first.ephemeralIdentity.address)
    expect(signer(second.authChain[1].payload, second.authChain[1].signature)).toBe(localGuestAddress(rootKey))
  })
})
