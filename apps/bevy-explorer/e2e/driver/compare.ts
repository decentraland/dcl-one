// The visual comparison explorer-automation's Visual suite applies: a pixel differs when any RGB
// channel differs by more than `channel` (8), and the frame passes when the differing pixels are
// at most `maxDifferentRatio` of the frame (0.3%; 1.5% for the avatar scene).
import { decodePng, encodePng, type Image } from './png.ts'

export interface CompareOptions {
  /** The largest per-channel difference that still counts as the same pixel. */
  channel?: number
  /** The largest share of differing pixels that still passes. */
  maxDifferentRatio?: number
}

export interface Comparison {
  width: number
  height: number
  total: number
  differing: number
  ratio: number
  pass: boolean
  /** The reference dimmed, with the differing pixels in red: a PNG. */
  diff: Buffer
}

export const DEFAULT_CHANNEL_TOLERANCE = 8
export const DEFAULT_MAX_DIFFERENT_RATIO = 0.003

export function compareImages(reference: Image, actual: Image, options: CompareOptions = {}): Comparison {
  const channel = options.channel ?? DEFAULT_CHANNEL_TOLERANCE
  const maxDifferentRatio = options.maxDifferentRatio ?? DEFAULT_MAX_DIFFERENT_RATIO
  if (reference.width !== actual.width || reference.height !== actual.height) {
    throw new Error(`size mismatch: the reference is ${reference.width}x${reference.height}, the frame ${actual.width}x${actual.height}`)
  }
  const total = reference.width * reference.height
  const diff = new Uint8Array(total * 4)
  let differing = 0
  for (let p = 0; p < total; p++) {
    const i = p * 4
    const differs =
      Math.abs(reference.rgba[i]! - actual.rgba[i]!) > channel ||
      Math.abs(reference.rgba[i + 1]! - actual.rgba[i + 1]!) > channel ||
      Math.abs(reference.rgba[i + 2]! - actual.rgba[i + 2]!) > channel
    if (differs) {
      differing++
      diff.set([255, 0, 0, 255], i)
    } else {
      diff.set([reference.rgba[i]! * 0.3, reference.rgba[i + 1]! * 0.3, reference.rgba[i + 2]! * 0.3, 255], i)
    }
  }
  const ratio = total === 0 ? 0 : differing / total
  return {
    width: reference.width,
    height: reference.height,
    total,
    differing,
    ratio,
    pass: ratio <= maxDifferentRatio,
    diff: encodePng({ width: reference.width, height: reference.height, rgba: diff })
  }
}

export function comparePng(reference: Uint8Array, actual: Uint8Array, options: CompareOptions = {}): Comparison {
  return compareImages(decodePng(reference), decodePng(actual), options)
}
