// Hex / engine Color3 (0–1 floats) conversions for the avatar colours.

import type { Color3 } from '../engine/generated/index'

const hex2 = (n: number): string => Math.round(Math.min(255, Math.max(0, n))).toString(16).padStart(2, '0')

export function hexToColor3(hex: string): Color3 {
  const n = parseInt(hex.replace('#', ''), 16)
  return { r: ((n >> 16) & 255) / 255, g: ((n >> 8) & 255) / 255, b: (n & 255) / 255 }
}

export function color3ToHex(c: Color3): string {
  return `#${hex2(c.r * 255)}${hex2(c.g * 255)}${hex2(c.b * 255)}`
}
