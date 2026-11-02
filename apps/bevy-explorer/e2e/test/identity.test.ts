import assert from 'node:assert/strict'
import { describe, test } from 'node:test'
import { addressOf, buildAuthIdentity, encodeIdentity, newPrivateKey, personalSign, recoverPersonalSigner } from '../driver/identity.ts'

// The address of private key 1 is a published test vector.
const KEY_ONE = '0x0000000000000000000000000000000000000000000000000000000000000001'
const ADDRESS_ONE = '0x7E5F4552091A69125d5DfCb7b8C2659029395Bdf'

describe('identity', () => {
  test('addressOf is the checksummed address of the key', () => {
    assert.equal(addressOf(KEY_ONE), ADDRESS_ONE)
    assert.equal(addressOf(KEY_ONE.slice(2)), ADDRESS_ONE)
  })

  test('a personal signature recovers to its signer', () => {
    const signature = personalSign(KEY_ONE, 'hello')
    assert.match(signature, /^0x[0-9a-f]{130}$/)
    assert.ok(signature.endsWith('1b') || signature.endsWith('1c'))
    assert.equal(recoverPersonalSigner('hello', signature), ADDRESS_ONE)
    assert.notEqual(recoverPersonalSigner('hello!', signature), ADDRESS_ONE)
  })

  test('buildAuthIdentity delegates a fresh ephemeral key, signed by the account', () => {
    const now = Date.UTC(2026, 9, 29)
    const identity = buildAuthIdentity(KEY_ONE, { now, expirationMs: 24 * 3600 * 1000 })
    assert.equal(identity.expiration, '2026-10-30T00:00:00.000Z')
    assert.match(identity.ephemeralIdentity.privateKey, /^0x[0-9a-f]{64}$/)
    assert.match(identity.ephemeralIdentity.publicKey, /^0x04[0-9a-f]{128}$/)
    assert.equal(identity.ephemeralIdentity.address, addressOf(identity.ephemeralIdentity.privateKey))
    assert.equal(identity.authChain.length, 2)
    const [signer, delegate] = identity.authChain
    assert.deepEqual(signer, { type: 'SIGNER', payload: ADDRESS_ONE, signature: '' })
    assert.equal(delegate!.type, 'ECDSA_EPHEMERAL')
    assert.equal(delegate!.payload, `Decentraland Login\nEphemeral address: ${identity.ephemeralIdentity.address}\nExpiration: 2026-10-30T00:00:00.000Z`)
    assert.equal(recoverPersonalSigner(delegate!.payload, delegate!.signature), ADDRESS_ONE)
  })

  test('two identities never share an ephemeral key', () => {
    const a = buildAuthIdentity(KEY_ONE)
    const b = buildAuthIdentity(KEY_ONE)
    assert.notEqual(a.ephemeralIdentity.privateKey, b.ephemeralIdentity.privateKey)
  })

  test('encodeIdentity is standard base64 of the JSON', () => {
    const identity = buildAuthIdentity(KEY_ONE)
    const encoded = encodeIdentity(identity)
    assert.match(encoded, /^[A-Za-z0-9+/]+=*$/)
    assert.deepEqual(JSON.parse(Buffer.from(encoded, 'base64').toString('utf8')), identity)
  })

  test('newPrivateKey is a 32-byte key with an address', () => {
    const key = newPrivateKey()
    assert.match(key, /^0x[0-9a-f]{64}$/)
    assert.match(addressOf(key), /^0x[0-9a-fA-F]{40}$/)
  })

  test('a malformed key is refused', () => {
    assert.throws(() => addressOf('0x1234'), /32 bytes of hex/)
  })
})
