// from: shop@94584a5c:app/src/lib/price-slider.spec.ts
import { describe, it, expect } from "vitest"
import {
  PRICE_SLIDER_KNEE,
  PRICE_SLIDER_MAX,
  PRICE_SLIDER_STEPS,
  priceToSliderPct,
  priceToSliderPos,
  sliderPosToPrice
} from "./price-slider"

describe("the price filter scale", () => {
  it("should anchor both ends of the track to both ends of the range", () => {
    expect(sliderPosToPrice(0)).toBe(0)
    expect(sliderPosToPrice(PRICE_SLIDER_STEPS)).toBe(PRICE_SLIDER_MAX)
    expect(priceToSliderPos(0)).toBe(0)
    expect(priceToSliderPos(PRICE_SLIDER_MAX)).toBe(PRICE_SLIDER_STEPS)
  })

  it("should put the knee at the halfway mark of the track", () => {
    expect(priceToSliderPct(PRICE_SLIDER_KNEE)).toBe(50)
    expect(sliderPosToPrice(PRICE_SLIDER_STEPS / 2)).toBe(PRICE_SLIDER_KNEE)
  })

  /**
   * The thumb is controlled: every drag turns a position into a price and straight back into a position.
   * If any position does not survive that trip the thumb snaps back under the cursor and reads as stuck,
   * which is exactly what a magnitude-based rounding step did here.
   */
  it("should return every single position unchanged through a price", () => {
    for (let pos = 0; pos <= PRICE_SLIDER_STEPS; pos++) {
      expect(priceToSliderPos(sliderPosToPrice(pos))).toBe(pos)
    }
  })

  it("should give every notch its own price, so no two positions collide", () => {
    const prices = new Set<number>()
    for (let pos = 0; pos <= PRICE_SLIDER_STEPS; pos++) prices.add(sliderPosToPrice(pos))
    expect(prices.size).toBe(PRICE_SLIDER_STEPS + 1)
  })

  // The point of the whole change: the mid-point of the track used to be 50,000 credits, which is above
  // almost everything on sale.
  it("should give the cheap end half the track to itself", () => {
    expect(sliderPosToPrice(PRICE_SLIDER_STEPS / 10)).toBe(180)
    expect(sliderPosToPrice(PRICE_SLIDER_STEPS / 4)).toBe(450)
  })

  it("should land on values a shopper would have typed", () => {
    for (let pos = 0; pos <= PRICE_SLIDER_STEPS; pos++) {
      const price = sliderPosToPrice(pos)
      expect(price % (price <= PRICE_SLIDER_KNEE ? 10 : 100)).toBe(0)
    }
  })

  it("should clamp anything outside the range rather than running off the track", () => {
    expect(sliderPosToPrice(-40)).toBe(0)
    expect(sliderPosToPrice(PRICE_SLIDER_STEPS + 500)).toBe(PRICE_SLIDER_MAX)
    expect(priceToSliderPos(-1)).toBe(0)
    expect(priceToSliderPos(PRICE_SLIDER_MAX * 3)).toBe(PRICE_SLIDER_STEPS)
  })

  it("should stay monotonic, so dragging right never lowers the price", () => {
    let previous = -1
    for (let pos = 0; pos <= PRICE_SLIDER_STEPS; pos += 5) {
      const price = sliderPosToPrice(pos)
      expect(price).toBeGreaterThanOrEqual(previous)
      previous = price
    }
  })
})
