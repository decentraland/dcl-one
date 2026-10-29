import { describe, expect, it } from 'vitest'
import { secp256k1 } from '@noble/curves/secp256k1.js'
import { keccak_256 } from '@noble/hashes/sha3.js'
import {
  createGuestIdentity,
  localGuestBackend,
  publicKeyAddress,
  webGuestBackend,
  webGuestBackendFor,
  type GuestBackend
} from '../features/auth/guest'
import { personalSign } from '../shell/sign'

function personalHash(message: string): Uint8Array {
  const body = new TextEncoder().encode(message)
  const prefix = new TextEncoder().encode(`\x19Ethereum Signed Message:\n${body.length}`)
  return keccak_256(new Uint8Array([...prefix, ...body]))
}

const hexToBytes = (hex: string): Uint8Array => Uint8Array.from(hex.match(/../g) ?? [], (b) => parseInt(b, 16))

describe('guest identity', () => {
  it('derives the checksummed address of a public key', () => {
    const publicKey = secp256k1.getPublicKey(hexToBytes('00'.repeat(31) + '01'), false)
    expect(publicKeyAddress(publicKey)).toBe('0x7E5F4552091A69125d5DfCb7b8C2659029395Bdf')
  })

  it('builds an identity the guest wallet signed for a fresh ephemeral key', async () => {
    const calls: string[][] = []
    const backend: GuestBackend = {
      login: async (sessionId) => {
        calls.push(['login', sessionId])
        return { token: 'jwt', walletAddress: '0x1e9a6ddCb5D0B8C2Ffd79372e022d63d053aB1bF' }
      },
      sign: async (token, message) => {
        calls.push(['sign', token, message])
        return '0xsigned'
      }
    }

    const identity = await createGuestIdentity(backend)

    const { address, privateKey } = identity.ephemeralIdentity
    expect(address).toBe(publicKeyAddress(secp256k1.getPublicKey(hexToBytes(privateKey?.slice(2) ?? ''), false)))
    const payload = `Decentraland Login\nEphemeral address: ${address}\nExpiration: ${identity.expiration}`
    expect(identity.authChain).toEqual([
      { type: 'SIGNER', payload: '0x1e9a6ddcb5d0b8c2ffd79372e022d63d053ab1bf', signature: '' },
      { type: 'ECDSA_EPHEMERAL', payload, signature: '0xsigned' }
    ])
    expect(calls).toEqual([
      ['login', expect.stringMatching(/^[0-9a-f]{64}$/)],
      ['sign', 'jwt', payload]
    ])
    const days = (new Date(identity.expiration).getTime() - Date.now()) / 86_400_000
    expect(days).toBeGreaterThan(29.9)
    expect(days).toBeLessThanOrEqual(30)
  })

  it('uses thirdweb on Decentraland domains and a browser-held key everywhere else', () => {
    for (const host of ['decentraland.org', 'decentraland.zone', 'decentraland.today', 'play.decentraland.org']) {
      expect(webGuestBackendFor(host)).toBe(webGuestBackend)
    }
    for (const host of ['127.0.0.1', 'localhost', 'example.com', 'decentraland.org.example.com', 'notdecentraland.org']) {
      expect(webGuestBackendFor(host)).toBe(localGuestBackend)
    }
  })

  it('a browser-held guest key signs its own identity', async () => {
    const identity = await createGuestIdentity(localGuestBackend)
    const [signer, delegate] = identity.authChain
    expect(signer.payload).toMatch(/^0x[0-9a-f]{40}$/)
    expect(signer.payload).not.toBe(identity.ephemeralIdentity.address.toLowerCase())
    const sig = delegate.signature.slice(2)
    const recovered = secp256k1.Signature.fromHex(sig.slice(0, 128))
      .addRecoveryBit(parseInt(sig.slice(128), 16) - 27)
      .recoverPublicKey(personalHash(delegate.payload))
      .toBytes(false)
    expect(publicKeyAddress(recovered).toLowerCase()).toBe(signer.payload)
  })

  it('the local backend signs with the session id as the key', async () => {
    const key = '4c0883a69102937d6231471b5dbb6204fe5129617082792ae468d01a3f362318'
    expect(await localGuestBackend.login(key)).toEqual({ token: key, walletAddress: '0x2c7536E3605D9C16a7a3D7b1898e529396a65c23' })
    expect(await localGuestBackend.sign(key, 'Some data')).toBe(personalSign(key, 'Some data'))
  })
})
