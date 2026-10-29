// The address a personal_sign signature over `message` recovers to, in lower case: what a verifier
// (the catalyst, the engine) takes the signer to be.
import { secp256k1 } from '@noble/curves/secp256k1.js'
import { keccak_256 } from '@noble/hashes/sha3.js'
import { hexToBytes, utf8ToBytes } from '@noble/hashes/utils.js'
import { publicKeyAddress } from '../features/auth/guest'

export function recoverSigner(message: string, signature: string): string {
  const body = utf8ToBytes(message)
  const prefixed = new Uint8Array([...utf8ToBytes(`\x19Ethereum Signed Message:\n${body.length}`), ...body])
  const bytes = hexToBytes(signature.slice(2))
  const sig = secp256k1.Signature.fromBytes(bytes.slice(0, 64), 'compact').addRecoveryBit(bytes[64] - 27)
  return publicKeyAddress(sig.recoverPublicKey(keccak_256(prefixed)).toBytes(false)).toLowerCase()
}
