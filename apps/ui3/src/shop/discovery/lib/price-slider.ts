// from: shop@94584a5c:app/src/lib/price-slider.ts
// The price filter's piecewise scale: half the track covers 0 to the knee, the other half the rest.
/** Upper bound of the slider, in credits. The Min/Max text inputs stay free-form above this. */
export const PRICE_SLIDER_MAX = 100_000

/** Where the scale changes gear, and what the halfway mark of the track is worth. */
export const PRICE_SLIDER_KNEE = 900

/** Notches per half. The knee therefore sits exactly at the middle of the track. */
const KNEE_POS = 90
export const PRICE_SLIDER_STEPS = KNEE_POS * 2

/** 10 credits per notch below the knee, an exact division, so those prices are exact multiples of it. */
const LOW_STEP = PRICE_SLIDER_KNEE / KNEE_POS

const HIGH_SPAN = PRICE_SLIDER_MAX - PRICE_SLIDER_KNEE

/**
 * Snap the coarse half to hundreds, so a drag lands on 3,100 rather than 3,102.
 *
 * The increment MUST stay well under what a notch is worth up here (~1,100 credits), and that is
 * load-bearing rather than tidy. The input is controlled: whatever price a drag produces is turned
 * straight back into a position. Round by more than a notch and two neighbouring positions collapse onto
 * the same price, which snaps the thumb out from under the cursor, it reads as stuck for a notch or two
 * and then jumps. That is exactly what a magnitude-based rounding step did to the fine half here.
 */
const HIGH_ROUND = 100

/** Track position (0…PRICE_SLIDER_STEPS) → price in credits. */
export function sliderPosToPrice(pos: number): number {
  const clamped = Math.max(0, Math.min(Math.round(pos), PRICE_SLIDER_STEPS))
  if (clamped <= KNEE_POS) return clamped * LOW_STEP
  const raw = PRICE_SLIDER_KNEE + ((clamped - KNEE_POS) / KNEE_POS) * HIGH_SPAN
  return Math.round(raw / HIGH_ROUND) * HIGH_ROUND
}

/** Price in credits → track position (0…PRICE_SLIDER_STEPS). The inverse of the mapping above. */
export function priceToSliderPos(price: number): number {
  const clamped = Math.max(0, Math.min(price, PRICE_SLIDER_MAX))
  if (clamped <= PRICE_SLIDER_KNEE) return Math.round(clamped / LOW_STEP)
  return KNEE_POS + Math.round(((clamped - PRICE_SLIDER_KNEE) / HIGH_SPAN) * KNEE_POS)
}

/** Price in credits → percentage along the track, for painting the filled span between the thumbs. */
export function priceToSliderPct(price: number): number {
  return (priceToSliderPos(price) / PRICE_SLIDER_STEPS) * 100
}
