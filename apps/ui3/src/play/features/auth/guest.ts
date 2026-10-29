// Persistent guest accounts: "Explore as guest" creates a guest wallet and signs in with it like
// any other account, so the guest keeps its avatar and name across visits. On web this runs in the
// shell page (src/shell/main.ts), which keeps the identity and its key with the other sign-ins; on
// native, in the page with the bridge scene making the thirdweb calls.
import { secp256k1 } from '@noble/curves/secp256k1.js'
import { keccak_256 } from '@noble/hashes/sha3.js'
import { bytesToHex, hexToBytes } from '@noble/hashes/utils.js'
import { personalSign } from '../../shell/sign'
import type { AuthIdentity } from './sso'
import { GUEST_SESSION_KEY, guestLogin, guestSign, type GuestWallet } from './thirdweb'

const IDENTITY_LIFETIME_MS = 30 * 24 * 3600 * 1000

/** Where the thirdweb calls run and the session id is kept: the page on web, the bridge scene on native. */
export interface GuestBackend {
  /** Keep `sessionId` as this device's guest (replacing any earlier one) and sign it in. */
  login(sessionId: string): Promise<GuestWallet>
  sign(token: string, message: string): Promise<string>
}

export const webGuestBackend: GuestBackend = {
  login(sessionId) {
    localStorage.setItem(GUEST_SESSION_KEY, sessionId)
    return guestLogin(sessionId)
  },
  sign: guestSign
}

// thirdweb only serves the guest wallet to the origins it allowlists, Decentraland's own.
const THIRDWEB_DOMAINS = ['decentraland.org', 'decentraland.zone', 'decentraland.today']

/**
 * Elsewhere (a dcl-one deployment, a local stack) the guest wallet is a key made in this browser:
 * the session id is the wallet's private key. It signs the identity and is then dropped, as the
 * thirdweb session id is; the stored identity is what brings the guest back.
 */
export const localGuestBackend: GuestBackend = {
  login(sessionId) {
    const walletAddress = publicKeyAddress(secp256k1.getPublicKey(hexToBytes(sessionId), false))
    return Promise.resolve({ token: sessionId, walletAddress })
  },
  sign: (token, message) => Promise.resolve(personalSign(token, message))
}

/** The guest wallet a web page on `hostname` can reach. */
export function webGuestBackendFor(hostname: string): GuestBackend {
  const thirdweb = THIRDWEB_DOMAINS.some((d) => hostname === d || hostname.endsWith(`.${d}`))
  return thirdweb ? webGuestBackend : localGuestBackend
}

function checksumAddress(address: Uint8Array): string {
  const hex = bytesToHex(address)
  const hash = bytesToHex(keccak_256(new TextEncoder().encode(hex)))
  let out = '0x'
  for (let i = 0; i < hex.length; i++) out += parseInt(hash[i], 16) >= 8 ? hex[i].toUpperCase() : hex[i]
  return out
}

/** The checksummed address of an uncompressed secp256k1 public key. */
export function publicKeyAddress(publicKey: Uint8Array): string {
  return checksumAddress(keccak_256(publicKey.slice(1)).slice(-20))
}

/** Create a new guest wallet and an identity it has signed. Any earlier guest is replaced. */
export async function createGuestIdentity(backend: GuestBackend): Promise<AuthIdentity> {
  const sessionId = bytesToHex(crypto.getRandomValues(new Uint8Array(32)))
  const { token, walletAddress } = await backend.login(sessionId)

  const privateKey = secp256k1.utils.randomSecretKey()
  const publicKey = secp256k1.getPublicKey(privateKey, false)
  const address = publicKeyAddress(publicKey)
  const expiration = new Date(Date.now() + IDENTITY_LIFETIME_MS).toISOString()
  const payload = `Decentraland Login\nEphemeral address: ${address}\nExpiration: ${expiration}`
  const signature = await backend.sign(token, payload)

  return {
    ephemeralIdentity: { address, publicKey: `0x${bytesToHex(publicKey)}`, privateKey: `0x${bytesToHex(privateKey)}` },
    expiration,
    authChain: [
      { type: 'SIGNER', payload: walletAddress.toLowerCase(), signature: '' },
      { type: 'ECDSA_EPHEMERAL', payload, signature }
    ]
  }
}
