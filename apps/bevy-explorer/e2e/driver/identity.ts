// An AuthIdentity for `/login_identity`, built as the HUD builds one for a guest
// (react-web/src/features/auth/guest.ts, shell/sign.ts): a fresh ephemeral key, delegated by the
// account's key with an EIP-191 personal signature over the catalyst's delegation message.
import { secp256k1 } from '@noble/curves/secp256k1.js'
import { keccak_256 } from '@noble/hashes/sha3.js'
import { bytesToHex, hexToBytes, utf8ToBytes } from '@noble/hashes/utils.js'

export interface AuthChainLink {
  type: 'SIGNER' | 'ECDSA_EPHEMERAL'
  payload: string
  signature: string
}

export interface AuthIdentity {
  ephemeralIdentity: { address: string; publicKey: string; privateKey: string }
  expiration: string
  authChain: AuthChainLink[]
}

export const IDENTITY_LIFETIME_MS = 30 * 24 * 3600 * 1000

function checksumAddress(address: Uint8Array): string {
  const hex = bytesToHex(address)
  const hash = bytesToHex(keccak_256(utf8ToBytes(hex)))
  let out = '0x'
  for (let i = 0; i < hex.length; i++) out += parseInt(hash[i]!, 16) >= 8 ? hex[i]!.toUpperCase() : hex[i]!
  return out
}

/** The EIP-55 address of an uncompressed secp256k1 public key. */
export function publicKeyAddress(publicKey: Uint8Array): string {
  return checksumAddress(keccak_256(publicKey.slice(1)).slice(-20))
}

export function privateKeyBytes(privateKey: string): Uint8Array {
  const hex = privateKey.trim().replace(/^0x/i, '')
  if (!/^[0-9a-f]{64}$/i.test(hex)) throw new Error('a private key is 32 bytes of hex')
  return hexToBytes(hex)
}

/** The address of a private key. */
export function addressOf(privateKey: string): string {
  return publicKeyAddress(secp256k1.getPublicKey(privateKeyBytes(privateKey), false))
}

/** EIP-191 `personal_sign`: deterministic (RFC 6979), low-s, `r || s || v` with v = 27 or 28. */
export function personalSign(privateKey: string, message: string): string {
  const body = utf8ToBytes(message)
  const prefix = utf8ToBytes(`\x19Ethereum Signed Message:\n${body.length}`)
  const prefixed = new Uint8Array(prefix.length + body.length)
  prefixed.set(prefix)
  prefixed.set(body, prefix.length)
  const bytes = secp256k1.sign(keccak_256(prefixed), privateKeyBytes(privateKey), { prehash: false, format: 'recovered' })
  const sig = secp256k1.Signature.fromBytes(bytes, 'recovered')
  const out = new Uint8Array(65)
  out.set(sig.toBytes('compact'))
  out[64] = 27 + (sig.recovery ?? 0)
  return `0x${bytesToHex(out)}`
}

/** The signer of an EIP-191 signature over `message`, as a checksummed address. */
export function recoverPersonalSigner(message: string, signature: string): string {
  const bytes = hexToBytes(signature.replace(/^0x/i, ''))
  if (bytes.length !== 65) throw new Error('a personal signature is 65 bytes')
  const body = utf8ToBytes(message)
  const prefix = utf8ToBytes(`\x19Ethereum Signed Message:\n${body.length}`)
  const prefixed = new Uint8Array(prefix.length + body.length)
  prefixed.set(prefix)
  prefixed.set(body, prefix.length)
  const recovered = new Uint8Array(65)
  recovered[0] = bytes[64]! - 27
  recovered.set(bytes.slice(0, 64), 1)
  const point = secp256k1.Signature.fromBytes(recovered, 'recovered').recoverPublicKey(keccak_256(prefixed))
  return publicKeyAddress(point.toBytes(false))
}

export function delegationMessage(ephemeralAddress: string, expiration: string): string {
  return `Decentraland Login\nEphemeral address: ${ephemeralAddress}\nExpiration: ${expiration}`
}

/** An identity for the account behind `privateKey`, valid for `expirationMs` (30 days by default). */
export function buildAuthIdentity(privateKey: string, options: { expirationMs?: number; now?: number } = {}): AuthIdentity {
  const { expirationMs = IDENTITY_LIFETIME_MS, now = Date.now() } = options
  const ephemeralKey = secp256k1.utils.randomSecretKey()
  const ephemeralPublic = secp256k1.getPublicKey(ephemeralKey, false)
  const ephemeralAddress = publicKeyAddress(ephemeralPublic)
  const expiration = new Date(now + expirationMs).toISOString()
  const payload = delegationMessage(ephemeralAddress, expiration)
  return {
    ephemeralIdentity: {
      address: ephemeralAddress,
      publicKey: `0x${bytesToHex(ephemeralPublic)}`,
      privateKey: `0x${bytesToHex(ephemeralKey)}`
    },
    expiration,
    authChain: [
      { type: 'SIGNER', payload: addressOf(privateKey), signature: '' },
      { type: 'ECDSA_EPHEMERAL', payload, signature: personalSign(privateKey, payload) }
    ]
  }
}

/** The identity as `/login_identity` takes it: standard base64 of its JSON. */
export function encodeIdentity(identity: AuthIdentity): string {
  return Buffer.from(JSON.stringify(identity), 'utf8').toString('base64')
}

/** A fresh account key, for a new-user scenario. */
export function newPrivateKey(): string {
  return `0x${bytesToHex(secp256k1.utils.randomSecretKey())}`
}
