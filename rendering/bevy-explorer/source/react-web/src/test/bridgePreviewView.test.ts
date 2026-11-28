// The avatar preview's view arithmetic (bridge-scene/src/domains/previewView): the Shop's framings, checked against
// the design by where they draw the avatar in the preview's window, and how far a zoomed view moves.
import { describe, expect, it } from 'vitest'
import { SHOP_FRAMING, SHOP_NAMES_FRAMING, SHOP_NAMETAG_SCALE, shiftWithin, type Framing } from '../../bridge-scene/src/domains/previewView'

const PITCH = 12
const ASPECT = 573 / 942
const RAD = (PITCH * Math.PI) / 180

/** Where a point by the avatar falls in the preview's window (shares of its width from its left, of its height from
 *  its top), as the bridge's camera draws it: looking north, orthographic, pitched 12° down, its view centre at the
 *  framing's height at the avatar, whose feet stand at 0. A point shows across at its distance east over the view's
 *  width, and up at its height above that centre foreshortened by the pitch, over the view's height. */
function windowPoint(x: number, y: number, f: Framing, zoom = 1, shift = 0): { x: number; y: number } {
  const height = f.range / zoom
  const centre = f.centerY + shiftWithin(f, zoom, shift)
  const up = (y - centre) * Math.cos(RAD)
  return { x: 0.5 + x / (height * ASPECT), y: 0.5 - up / height }
}

describe("the Shop's avatar preview view", () => {
  it('shows the framing height at the middle of the window, and the view edges at its edges', () => {
    expect(windowPoint(0, SHOP_FRAMING.centerY, SHOP_FRAMING)).toEqual({ x: expect.closeTo(0.5, 9), y: expect.closeTo(0.5, 9) })
    expect(windowPoint(0, SHOP_FRAMING.centerY + SHOP_FRAMING.range / 2 / Math.cos(RAD), SHOP_FRAMING).y).toBeCloseTo(0, 9)
    expect(windowPoint((SHOP_FRAMING.range * ASPECT) / 2, SHOP_FRAMING.centerY, SHOP_FRAMING).x).toBeCloseTo(1, 9)
  })

  it("puts the catalogue's feet near the floor of its 942px window, as the design stands the avatar", () => {
    // 0.5 + 1.84 × cos 12° / 4.9: about 817px down the window
    expect(windowPoint(0, 0, SHOP_FRAMING).y * 942).toBeCloseTo(817, 0)
  })

  it("draws Names' avatar as the design's Names pages do: the catalogue's at 91.9%, its box 109px down, not 1.4px up", () => {
    // The design's Player: 917.4px tall from 1.4px above the window in the catalogue, 842.9px from 109px down in Names.
    const k = 842.9998 / 917.4248
    for (const y of [0, 1.84, 3.6]) {
      const catalogue = windowPoint(0.3, y, SHOP_FRAMING)
      const names = windowPoint(0.3, y, SHOP_NAMES_FRAMING)
      // within a pixel down the window, for the feet, the middle of the view and a 2×-scale avatar's head
      expect(Math.abs(names.y * 942 - (109 + k * (catalogue.y * 942 + 1.4248)))).toBeLessThan(1)
      // and across, centred as before: 0.3m east, at 91.9% of the distance from the middle
      expect((names.x - 0.5) * 573).toBeCloseTo(k * (catalogue.x - 0.5) * 573, 0)
    }
  })

  it('zooms about the moved centre, which stays inside the framing', () => {
    expect(shiftWithin(SHOP_FRAMING, 2, 10)).toBeCloseTo((SHOP_FRAMING.range - SHOP_FRAMING.range / 2) / 2, 9)
    expect(shiftWithin(SHOP_FRAMING, 2, -10)).toBeCloseTo(-(SHOP_FRAMING.range - SHOP_FRAMING.range / 2) / 2, 9)
    expect(shiftWithin(SHOP_FRAMING, 1, 10)).toBe(0)
    // twice the zoom, half the view: a point 1m over the centre moves twice as far from the middle
    const once = windowPoint(0, SHOP_FRAMING.centerY + 1, SHOP_FRAMING)
    const twice = windowPoint(0, SHOP_FRAMING.centerY + 1, SHOP_FRAMING, 2)
    expect(0.5 - twice.y).toBeCloseTo(2 * (0.5 - once.y), 9)
  })

  it("sizes the engine's nametag on Names as the design draws it: 54.066px of the 942px window", () => {
    // The engine's idle tag: its 18-unit name at 1.21 line height, 5 units above and below; each unit 0.075% of the
    // view's height (nametag.wgsl's SCREEN_SCALE × UNIT, halved), under the preview's orthographic camera too.
    const tagUnits = 18 * 1.21 + 5 + 5
    const shareOfView = (0.15 * 0.01) / 2
    expect(tagUnits * shareOfView * SHOP_NAMETAG_SCALE * 942).toBeCloseTo(54.066, 0)
  })
})
