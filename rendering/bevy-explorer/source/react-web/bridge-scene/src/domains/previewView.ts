// The avatar preview's orthographic view, as plain arithmetic: the Shop's framings, and how far a zoomed view may
// move inside its framing.

/** The visible height of the view (metres, orthographic) and the height at the avatar it is centred on. */
export type Framing = { range: number; centerY: number }

/** The Shop's whole body in its 573×942 window, as the design's catalogue pages draw it: the avatar ~751px tall,
 *  its feet on the stage's floor logo (~970px down the screen). */
export const SHOP_FRAMING: Framing = { range: 4.9, centerY: 1.84 }
/** The Shop's Names pages draw the same avatar smaller and lower, with room over its head for the nametag: the
 *  design's Player at 91.9% of the catalogue's (842.9px tall against 917.4px), its box from 109px down the window
 *  rather than 1.4px above it. So the view is the catalogue's over 0.919, centred 0.42m higher. */
export const SHOP_NAMES_FRAMING: Framing = { range: 5.33, centerY: 2.26 }

/** The view's centre moved `shift` metres up at zoom `z`, kept inside the framing (none at zoom 1). */
export function shiftWithin(f: { range: number }, z: number, shift: number): number {
  const room = (f.range - f.range / z) / 2
  return Math.max(-room, Math.min(room, shift))
}
