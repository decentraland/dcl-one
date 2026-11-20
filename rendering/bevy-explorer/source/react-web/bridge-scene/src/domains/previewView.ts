// The avatar preview's orthographic view, as plain arithmetic: how far a zoomed view may move inside its framing,
// and where a point by the avatar falls in the preview's window.

/** The visible height of the view (metres, orthographic) and the height at the avatar it is centred on. */
export type Framing = { range: number; centerY: number }
/** Metres along the scene's axes: x east, y up, z north. */
export type SceneOffset = { x: number; y: number; z: number }
/** Shares of the preview window: of its width from its left, of its height from its top. */
export type WindowPoint = { x: number; y: number }

/** The Shop's whole body in its 573×942 window, as the design's catalogue pages draw it: the avatar ~751px tall,
 *  its feet on the stage's floor logo (~970px down the screen). */
export const SHOP_FRAMING: Framing = { range: 4.9, centerY: 1.84 }
/** The Shop's Names pages draw the same avatar smaller and lower, with room over its head for the nametag: the
 *  design's Player at 91.9% of the catalogue's (842.9px tall against 917.4px), its box from 109px down the window
 *  rather than 1.4px above it. So the view is the catalogue's over 0.919, centred 0.42m higher. */
export const SHOP_NAMES_FRAMING: Framing = { range: 5.33, centerY: 2.26 }

/** How far the nametag anchor's height moves before the Shop's tag follows it: 8cm of the Shop's 2×-scale avatar
 *  (4cm of its own), about 14px up the 942px window in Names' framing, unzoomed. More than the head sways in an idle,
 *  less than a hat adds. It bounds the height only: on the axis, the tag also drops the head's offset across and in
 *  depth. */
export const NAMETAG_SWAY = 0.08

/** Where the Shop's nametag stands, from the engine's anchor `read` and the place it `held`: on the avatar's axis,
 *  at the anchor's height. The place is kept while the head only sways or turns (the idle's breath, a drag), and
 *  moves with a change of look: a hat put on or off, another body. */
export function steadyAnchor(held: SceneOffset | null, read: SceneOffset): SceneOffset {
  if (held != null && Math.abs(read.y - held.y) < NAMETAG_SWAY) return held
  return { x: 0, y: read.y, z: 0 }
}

/** The view's centre moved `shift` metres up at zoom `z`, kept inside the framing. */
export function shiftWithin(f: { range: number }, z: number, shift: number): number {
  const room = (f.range - f.range / z) / 2
  return Math.max(-room, Math.min(room, shift))
}

/** Where a point by the avatar falls in the preview's window. `offset` is from the avatar's feet, which stand at
 *  `feetY`, centred across the view. The camera looks north, orthographic and pitched `pitchDeg` down, placed so its
 *  view centre stands at the framing's height at the avatar. So a point shows across at its distance east over the
 *  view's width (its height times the window's `aspect`, width over height), and up at its height above that
 *  centre foreshortened by the pitch, raised by the pitch for its depth beyond the avatar, over the view's height.
 *  The Shop's tag only passes points on the avatar's axis (steadyAnchor), so its east and depth terms are 0 there; the
 *  projection stays whole so that where the tag stands is decided by the hold alone. */
export function windowPoint(offset: SceneOffset, f: Framing, z: number, shift: number, pitchDeg: number, aspect: number, feetY = 0): WindowPoint {
  const pitch = (pitchDeg * Math.PI) / 180
  const height = f.range / z
  const centre = f.centerY + shiftWithin(f, z, shift)
  const up = (feetY + offset.y - centre) * Math.cos(pitch) + offset.z * Math.sin(pitch)
  return { x: 0.5 + offset.x / (height * aspect), y: 0.5 - up / height }
}
