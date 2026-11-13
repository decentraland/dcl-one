import { sha256 } from '@noble/hashes/sha2.js'
import { sha256 as nativeSha256 } from '@noble/hashes/webcrypto.js'

export const RAW_CODEC = 0x55
export const DAG_PB_CODEC = 0x70

const BASE32 = 'abcdefghijklmnopqrstuvwxyz234567'
const BASE58 = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz'

// sha2-256 multihash: code 0x12, digest length 0x20
function multihash(digest: Uint8Array, prefix: number[]): Uint8Array {
  const out = new Uint8Array(prefix.length + 2 + digest.length)
  out.set(prefix)
  out.set([0x12, 0x20], prefix.length)
  out.set(digest, prefix.length + 2)
  return out
}

function base32(bytes: Uint8Array): string {
  let out = ''
  let buffer = 0
  let bits = 0
  for (const byte of bytes) {
    buffer = (buffer << 8) | byte
    bits += 8
    while (bits >= 5) {
      bits -= 5
      out += BASE32[(buffer >>> bits) & 31]
    }
    buffer &= (1 << bits) - 1
  }
  return bits > 0 ? out + BASE32[(buffer << (5 - bits)) & 31] : out
}

function base58(bytes: Uint8Array): string {
  const digits: number[] = []
  for (const byte of bytes) {
    let carry = byte
    for (let i = 0; i < digits.length; i++) {
      carry += digits[i] << 8
      digits[i] = carry % 58
      carry = Math.floor(carry / 58)
    }
    while (carry > 0) {
      digits.push(carry % 58)
      carry = Math.floor(carry / 58)
    }
  }
  let out = ''
  for (let i = 0; i < bytes.length && bytes[i] === 0; i++) out += BASE58[0]
  for (let i = digits.length - 1; i >= 0; i--) out += BASE58[digits[i]]
  return out
}

export function varint(value: number): number[] {
  const out: number[] = []
  while (value >= 0x80) {
    out.push((value % 0x80) | 0x80)
    value = Math.floor(value / 0x80)
  }
  out.push(value)
  return out
}

const hasNativeDigest = typeof globalThis.crypto?.subtle?.digest === 'function'

// From this size the platform's digest is faster than the JavaScript one, although each call
// costs a thread hop. Measured on Node 24: equal near 4 KiB, six times faster at 256 KiB.
const NATIVE_DIGEST_MIN_BYTES = 4096

/**
 * The sha2-256 digest of the bytes. Large input goes to the platform's digest, which runs off
 * the main thread, so several calls run at the same time. Each path reads the bytes before this
 * function returns, so the caller can change them afterwards. A view of shared memory gets the
 * JavaScript digest, because the platform refuses it. The result never rejects.
 */
export function sha256Digest(bytes: Uint8Array): Uint8Array | Promise<Uint8Array> {
  if (
    !hasNativeDigest || bytes.length < NATIVE_DIGEST_MIN_BYTES ||
    (typeof SharedArrayBuffer !== 'undefined' && bytes.buffer instanceof SharedArrayBuffer)
  ) return sha256(bytes)
  return nativeSha256(bytes).catch(() => sha256(bytes))
}

/** The binary CIDv1 of a block with this sha2-256 digest: version, codec, multihash. */
export function cidV1(codec: number, digest: Uint8Array): Uint8Array {
  return multihash(digest, [0x01, codec])
}

/** A binary CIDv1 in its base32 text form ("bafkrei…" raw, "bafybei…" dag-pb). */
export function cidV1Text(cid: Uint8Array): string {
  return 'b' + base32(cid)
}

/** The base58 text form of a sha2-256 digest ("Qm…"). */
export function qmText(digest: Uint8Array): string {
  return base58(multihash(digest, []))
}
