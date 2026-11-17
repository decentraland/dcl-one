// The HUD's `--ui-scale`, and a signal for when it changes.
//
// Scales with a 1920x1080 reference canvas by the smaller of the width and height ratios, so the canvas
// expands along the longer axis. Viewport sizes are logical px, so
// this is DPI-correct as-is: a 1512x945 MacBook viewport → ~0.79.
//
// The scale follows the viewport with no clamp over every size a window takes: nothing is hidden
// or wrapped at a small size, everything shrinks. MIN and
// MAX only keep the value finite: the HUD's layout boxes divide by it (`100% / var(--ui-scale)`),
// so it must not reach 0 in a collapsed or hidden frame, and MAX bounds those boxes on a display
// wall. MIN is under the scale of a 640x480 window (0.333); MAX is an 8K screen.
//
// It is a store rather than a hook because the scale has a SUBSCRIBER that is not a React render:
// the engine cutouts (minimap, avatar preview) report their on-screen rect to the scene, and the
// HUD is scaled with a CSS transform — which changes no border box, so a ResizeObserver never
// fires for it. Those rects therefore have to be re-measured whenever the scale moves, and they
// have to be measured AFTER it has been applied. Both used to hang off `resize` listeners, and
// EngineViewport's was registered first (child effects run before parent effects), so every
// resize measured the rect at the old scale and nothing re-measured it — which is what left the
// engine drawing the minimap at its pre-resize size.
//
// So `apply` writes the custom property and THEN wakes subscribers, all in the same task: the
// subscriber's getBoundingClientRect() forces the style recalc, so it sees the new layout without
// deferring further.
//
// A window being dragged fires `resize` many times between two paints. They are coalesced: the
// first asks for a frame, and that frame reads the viewport once, writes the property and wakes the
// subscribers, in that order, so each painted size costs one style recalc and one round of rects.

const REFERENCE_WIDTH = 1920
const REFERENCE_HEIGHT = 1080
const MIN = 0.25
const MAX = 4

let scale = 0
const listeners = new Set<() => void>()

function apply(): void {
  const expand = Math.min(window.innerWidth / REFERENCE_WIDTH, window.innerHeight / REFERENCE_HEIGHT)
  const next = Math.min(MAX, Math.max(MIN, expand))
  if (next === scale) return
  scale = next
  document.documentElement.style.setProperty('--ui-scale', next.toFixed(4))
  listeners.forEach((l) => l())
}

/** Install the scale at boot (see main.tsx) and keep it in sync with the viewport. */
export function installHudScale(): void {
  apply()
  let frame: number | null = null
  window.addEventListener('resize', () => {
    if (frame != null) return
    frame = requestAnimationFrame(() => {
      frame = null
      apply()
    })
  })
}

export function getHudScale(): number {
  return scale
}

/** Wake `onChange` after `--ui-scale` has been written, so it can measure the scaled layout. */
export function subscribeHudScale(onChange: () => void): () => void {
  listeners.add(onChange)
  return () => { listeners.delete(onChange) }
}
