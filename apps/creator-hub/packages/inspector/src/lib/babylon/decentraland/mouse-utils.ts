import type { MeshPredicate, Scene, Vector3 } from '@babylonjs/core';

export const LEFT_BUTTON = 0;
export const MIDDLE_BUTTON = 1;
export const RIGHT_BUTTON = 2;

/**
 * Picks the scene at a viewport point and returns the hit, or null when the ray misses.
 *
 * `ndc` is the point in normalized device coords (x,y ∈ [-1,1], y up) relative to the canvas.
 * Without it, the scene's last pointer position is used. It answers right away: a drop lands
 * after an HTML5 drag, which sends Babylon no pointer events, so waiting for the next pointer
 * event would hold the dropped item back until the mouse moved again.
 */
export function getPointerCoords(
  scene: Scene,
  ndc?: { x: number; y: number },
  predicate: MeshPredicate = () => true,
): Vector3 | null {
  let x = scene.pointerX;
  let y = scene.pointerY;
  if (ndc) {
    // scene.pick takes the same CSS-pixel coords as scene.pointerX/Y.
    const engine = scene.getEngine();
    const scale = engine.getHardwareScalingLevel();
    const width = engine.getRenderWidth() * scale;
    const height = engine.getRenderHeight() * scale;
    x = ((ndc.x + 1) / 2) * width;
    y = ((1 - ndc.y) / 2) * height;
  }
  return scene.pick(x, y, predicate)?.pickedPoint ?? null;
}
