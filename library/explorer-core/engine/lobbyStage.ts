// The lobby stage's backdrop, camera and podium, shared by the bridge (which draws the stage) and the page
// (which draws a stand-in until the stage is ready, and the whole stage without an engine), so the
// two can't drift apart. Lengths are fractions of the stage's height, measured on a 1920×1080 stage.
// The backdrop picture (the outdoor scene, with the design's top and bottom shade painted into it)
// covers the stage: scaled to fill it, centred, the overflow cut off.

/** The colour (0–255) under the backdrop picture: the tone of its bottom edge. */
export const BASE = { r: 52, g: 13, b: 9 }

/** The backdrop picture's width over its height (assets/lobby/background.jpg, 1920×1077; the bridge's
 *  images/lobby-background.jpg is the same file). */
export const BACKDROP_ASPECT = 1920 / 1077

export function backdropRect(width: number, height: number): { left: number; top: number; width: number; height: number } {
  const w = Math.max(width, height * BACKDROP_ASPECT)
  const h = Math.max(height, width / BACKDROP_ASPECT)
  return { left: (width - w) / 2, top: (height - h) / 2, width: w, height: h }
}

/** The stage camera: looking `CAMERA_PITCH` degrees down at a point `CAMERA_TARGET_Y` metres over
 *  the avatar's feet from `CAMERA_DISTANCE` metres back, with a `CAMERA_FOV` degree vertical field of
 *  view. The engine's avatar measures 1.72 m to the eyes and about 1.95 m to the top of its hair, so
 *  its hair top stands near 15% of the screen height, its eyes near 23.5% and its soles near 84.6%,
 *  on the podium's top face, its feet seen from 17° above. */
export const CAMERA_PITCH = 8.06
export const CAMERA_TARGET_Y = 0.99
export const CAMERA_DISTANCE = 6
export const CAMERA_FOV = 26.19

/** The podium under the avatar: one picture (assets/lobby/podium.png, 935×348; the bridge's
 *  images/lobby-podium.png is the same file), painted by the bridge under the avatar and by the page
 *  in its stand-in, in the same place. Width, height and top as fractions of the screen height;
 *  centred horizontally. */
export const PODIUM_IMAGE = { width: 564 / 1080, height: 208 / 1080, top: 821.3 / 1080 }

export function podiumRect(width: number, height: number): { left: number; top: number; width: number; height: number } {
  const w = height * PODIUM_IMAGE.width
  return { left: (width - w) / 2, top: height * PODIUM_IMAGE.top, width: w, height: height * PODIUM_IMAGE.height }
}
