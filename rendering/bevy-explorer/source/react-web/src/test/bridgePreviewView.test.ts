// The avatar preview's view arithmetic (bridge-scene/src/domains/previewView): where a point by the avatar falls in the
// preview's window, for the Shop's nametag, and how far a zoomed view moves.
import { describe, expect, it } from 'vitest'
import { NAMETAG_SWAY, SHOP_FRAMING, SHOP_NAMES_FRAMING, shiftWithin, steadyAnchor, windowPoint } from '../../bridge-scene/src/domains/previewView'

const SHOP = SHOP_FRAMING
const PITCH = 12
const ASPECT = 573 / 942
const RAD = (PITCH * Math.PI) / 180
const at = (x: number, y: number, z: number, zoom = 1) => windowPoint({ x, y, z }, SHOP, zoom, 0, PITCH, ASPECT)

describe("the avatar preview's view", () => {
  it('shows the framing height over the feet at the middle of the window, and the view edges at its edges', () => {
    expect(at(0, SHOP.centerY, 0)).toEqual({ x: expect.closeTo(0.5, 9), y: expect.closeTo(0.5, 9) })
    expect(at(0, SHOP.centerY + SHOP.range / 2 / Math.cos(RAD), 0).y).toBeCloseTo(0, 9)
    expect(at((SHOP.range * ASPECT) / 2, SHOP.centerY, 0).x).toBeCloseTo(1, 9)
  })

  it("puts the Shop's feet near the floor of its 942px window, as the design stands the avatar", () => {
    // 0.5 + 1.84 × cos 12° / 4.9: about 817px down the window
    expect(at(0, 0, 0).y * 942).toBeCloseTo(817, 0)
  })

  it("draws Names' avatar as the design's Names pages do: the catalogue's at 91.9%, its box 109px down, not 1.4px up", () => {
    // The design's Player: 917.4px tall from 1.4px above the window in the catalogue, 842.9px from 109px down in Names.
    const k = 842.9998 / 917.4248
    const names = (x: number, y: number) => windowPoint({ x, y, z: 0 }, SHOP_NAMES_FRAMING, 1, 0, PITCH, ASPECT)
    for (const y of [0, 1.84, 3.6]) {
      const catalogue = at(0.3, y, 0)
      const drawn = names(0.3, y)
      // within a pixel down the window, for the feet, the middle of the view and a 2×-scale avatar's head
      expect(Math.abs(drawn.y * 942 - (109 + k * (catalogue.y * 942 + 1.4248)))).toBeLessThan(1)
      // and across, centred as before: 0.3m east, at 91.9% of the distance from the middle
      expect((drawn.x - 0.5) * 573).toBeCloseTo(k * (catalogue.x - 0.5) * 573, 0)
    }
  })

  it('raises a point further from the camera: two at one height, half a metre apart in depth, ~20px apart', () => {
    const near = at(0, 3.6, 0)
    const far = at(0, 3.6, 0.5)
    expect(far.x).toBeCloseTo(near.x, 9)
    expect((near.y - far.y) * 942).toBeCloseTo((0.5 * Math.sin(RAD) * 942) / SHOP.range, 6)
    expect((near.y - far.y) * 942).toBeGreaterThan(19)
  })

  it('moves a point east to the right, by its distance over the view width', () => {
    const centre = at(0, 3.6, 0)
    const east = at(0.3, 3.6, 0)
    expect(east.y).toBeCloseTo(centre.y, 9)
    expect((east.x - centre.x) * 573).toBeCloseTo((0.3 * 573) / (SHOP.range * ASPECT), 6)
  })

  it("stands the Shop's nametag on the avatar's axis, at the anchor's height, wherever the head turns", () => {
    expect(steadyAnchor(null, { x: 0.2, y: 3.6, z: -0.3 })).toEqual({ x: 0, y: 3.6, z: 0 })
  })

  it('keeps the place while the head sways, and moves it for a change of look, up or down', () => {
    const held = steadyAnchor(null, { x: 0, y: 3.6, z: 0 })
    // the idle's breath and a turn: a few centimetres up or down, the head off the axis
    expect(steadyAnchor(held, { x: -0.15, y: 3.63, z: 0.2 })).toBe(held)
    expect(steadyAnchor(held, { x: 0.1, y: 3.6 - NAMETAG_SWAY / 2, z: 0 })).toBe(held)
    // a hat put on, then off
    const hat = steadyAnchor(held, { x: 0.1, y: 3.95, z: 0 })
    expect(hat).toEqual({ x: 0, y: 3.95, z: 0 })
    expect(steadyAnchor(hat, { x: 0, y: 3.6, z: 0 })).toEqual({ x: 0, y: 3.6, z: 0 })
  })

  it("lets the tag sway by no more than about 14px of Names' window", () => {
    const names = (y: number) => windowPoint({ x: 0, y, z: 0 }, SHOP_NAMES_FRAMING, 1, 0, PITCH, ASPECT)
    const px = (names(3.6).y - names(3.6 + NAMETAG_SWAY).y) * 942
    expect(px).toBeGreaterThan(13)
    expect(px).toBeLessThan(15)
  })

  it('zooms about the moved centre, which stays inside the framing', () => {
    expect(shiftWithin(SHOP, 2, 10)).toBeCloseTo((SHOP.range - SHOP.range / 2) / 2, 9)
    expect(shiftWithin(SHOP, 1, 10)).toBe(0)
    // twice the zoom, half the view: a point 1m over the centre moves twice as far from the middle
    const once = at(0, SHOP.centerY + 1, 0)
    const twice = at(0, SHOP.centerY + 1, 0, 2)
    expect(0.5 - twice.y).toBeCloseTo(2 * (0.5 - once.y), 9)
  })
})
