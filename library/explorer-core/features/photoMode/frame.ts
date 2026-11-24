// The photo is the centred 16:9 share of the screen the engine cuts the saved image from: this
// is the same rule, so the overlay's frame shows exactly what the photo keeps.
export const PHOTO_ASPECT = 1920 / 1080
export const FRAME_SCALE = 0.87

/** The photo frame's share of the screen per axis, for a screen of `aspect` (width / height). */
export function normalizedFrameSize(aspect: number): { x: number; y: number } {
  return aspect > PHOTO_ASPECT
    ? { x: (FRAME_SCALE * PHOTO_ASPECT) / aspect, y: FRAME_SCALE }
    : { x: FRAME_SCALE, y: (FRAME_SCALE * aspect) / PHOTO_ASPECT }
}
