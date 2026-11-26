import { describe, expect, it, vi } from "vitest";

import { EDGE_BOTTOM, EDGE_LEFT, EDGE_RIGHT, EDGE_TOP, GRID_MAX, GRID_MID, GRID_MIN, clampView, coordToPoint, cssColor, drawLayers, keepInView, kindLayer, markLayer, markLine, parcelPx, placePopup, pointToCoord, stagePan, viewportOf, zoomAt, type LayerCanvas, type MapLayer } from "./mapView";
import type { LandTile, Tiles } from "./tiles";

// A shell of 340 x 340 shows the square of 340 parcels at 1 px per parcel at zoom 1, 10 px at zoom 10.
const SHELL = { w: 340, h: 340 };
const WIDE = { w: 680, h: 340 };

const tile = (x: number, y: number, over: Partial<LandTile> = {}): LandTile => ({ id: `${x},${y}`, x, y, kind: "owned", top: false, left: false, topLeft: false, owner: null, name: null, estateId: null, price: null, rentalPricePerDay: null, ...over });

function fakeCanvas() {
  const fills: string[] = [];
  const ctx = {
    setTransform: vi.fn(),
    clearRect: vi.fn(),
    fillRect: vi.fn(),
    set fillStyle(value: string) {
      fills.push(value);
    },
  };
  return { ctx: ctx as unknown as LayerCanvas, fillRect: ctx.fillRect, setTransform: ctx.setTransform, clearRect: ctx.clearRect, fills };
}

const only =
  (x: number, y: number, t: ReturnType<MapLayer>): MapLayer =>
  (px, py) =>
    px === x && py === y ? t : null;

describe("the view", () => {
  it("gives the pixels of one parcel from the shell's smaller side and the zoom", () => {
    expect(parcelPx(SHELL, 1)).toBe(1);
    expect(parcelPx(WIDE, 10)).toBe(10);
  });

  it("keeps the zoom inside its range", () => {
    expect(clampView({ x: GRID_MID, y: GRID_MID, zoom: 50 }, SHELL, 1, 12).zoom).toBe(12);
    expect(clampView({ x: GRID_MID, y: GRID_MID, zoom: 0.1 }, SHELL, 1, 12).zoom).toBe(1);
  });

  it("keeps the centre where the square still fills the shell", () => {
    expect(clampView({ x: 30, y: 30, zoom: 1 }, SHELL, 1, 12)).toEqual({ x: GRID_MID, y: GRID_MID, zoom: 1 });
    // At zoom 2 in a shell twice as wide as high, the square is as wide as the shell and twice as high.
    expect(clampView({ x: 30, y: 300, zoom: 2 }, WIDE, 1, 12)).toEqual({ x: GRID_MID, y: GRID_MID + 85, zoom: 2 });
  });

  it("returns the same view when nothing is over a limit", () => {
    const view = { x: 4, y: -7, zoom: 10 };
    expect(clampView(view, SHELL, 1, 12)).toBe(view);
  });

  it("gives the pan of the square for the stage transform", () => {
    expect(stagePan({ x: GRID_MID, y: GRID_MID, zoom: 10 }, SHELL)).toEqual({ x: 0, y: 0 });
    expect(stagePan({ x: 9.5, y: GRID_MID, zoom: 10 }, SHELL)).toEqual({ x: -100, y: 0 });
    expect(stagePan({ x: GRID_MID, y: 9.5, zoom: 10 }, SHELL)).toEqual({ x: 0, y: 100 });
  });

  it("finds the parcel under a point and the point of a parcel's middle", () => {
    const view = { x: GRID_MID, y: GRID_MID, zoom: 10 };
    expect(pointToCoord(view, SHELL, { x: 275, y: 65 })).toEqual({ x: 10, y: 10 });
    expect(pointToCoord(view, SHELL, { x: 279, y: 61 })).toEqual({ x: 10, y: 10 });
    expect(pointToCoord(view, SHELL, { x: 255, y: 205 })).toEqual({ x: 8, y: -4 });
    expect(coordToPoint(view, SHELL, { x: 10, y: 10 })).toEqual({ x: 275, y: 65 });
  });

  it("finds no parcel outside the square", () => {
    expect(pointToCoord({ x: GRID_MID, y: GRID_MID, zoom: 1 }, WIDE, { x: 10, y: 100 })).toBeNull();
  });

  it("reports the corner parcels of what the shell shows", () => {
    expect(viewportOf({ x: 0, y: 0, zoom: 10 }, SHELL)).toEqual({ center: { x: 0, y: 0 }, zoom: 10, nw: { x: -17, y: 17 }, se: { x: 17, y: -17 } });
    const whole = viewportOf({ x: GRID_MID, y: GRID_MID, zoom: 1 }, WIDE);
    expect(whole.nw).toEqual({ x: GRID_MIN, y: GRID_MAX });
    expect(whole.se).toEqual({ x: GRID_MAX, y: GRID_MIN });
  });

  it("zooms about a point: the parcel under it stays under it", () => {
    const from = { x: GRID_MID, y: GRID_MID, zoom: 10 };
    const anchor = { x: 275, y: 65 };
    const to = zoomAt(from, SHELL, 12, anchor);
    expect(to.zoom).toBe(12);
    const p = coordToPoint(to, SHELL, { x: 10, y: 10 });
    expect(p.x).toBeCloseTo(anchor.x, 9);
    expect(p.y).toBeCloseTo(anchor.y, 9);
  });

  it("zooms about the middle of the shell when no point is given", () => {
    expect(zoomAt({ x: 3, y: 4, zoom: 10 }, SHELL, 12)).toEqual({ x: 3, y: 4, zoom: 12 });
  });

  it("moves the view by the least that shows a parcel", () => {
    const view = { x: 0, y: 0, zoom: 10 };
    expect(keepInView(view, SHELL, { x: 30, y: 0 })).toEqual({ x: 14.5, y: 0, zoom: 10 });
    expect(keepInView(view, SHELL, { x: 5, y: 5 })).toBe(view);
  });
});

describe("the popup's place", () => {
  it("is below and right of the pointer when there is room", () => {
    expect(placePopup({ x: 20, y: 20 }, SHELL)).toEqual({ left: 34, top: 34 });
  });

  it("goes to the left near the right edge", () => {
    expect(placePopup({ x: 330, y: 20 }, SHELL)).toEqual({ right: 20, top: 34 });
  });

  it("goes above near the bottom edge", () => {
    expect(placePopup({ x: 20, y: 330 }, SHELL)).toEqual({ left: 34, bottom: 20 });
  });

  it("stays against the edge of a shell narrower than the popup", () => {
    expect(placePopup({ x: 150, y: 20 }, { w: 200, h: 340 })).toEqual({ right: 8, top: 34 });
  });
});

describe("a layer colour", () => {
  const props: Record<string, string> = { "--_taken": " var(--fill-5)", "--fill-5": "rgba(255,255,255,.12)" };
  const lookup = (name: string) => props[name] ?? "";

  it("is used as written when it is not a custom property", () => {
    expect(cssColor("#ff0044", lookup)).toBe("#ff0044");
  });

  it("is looked up through custom properties, one after the other", () => {
    expect(cssColor("var(--_taken)", lookup)).toBe("rgba(255,255,255,.12)");
  });

  it("falls back to the value in the reference, and is empty when there is none", () => {
    expect(cssColor("var(--unknown, #123456)", lookup)).toBe("#123456");
    expect(cssColor("var(--unknown)", lookup)).toBe("");
  });
});

describe("the base layer from tile data", () => {
  const tiles: Tiles = { "1,2": tile(1, 2, { kind: "plaza", left: true }), "3,2": tile(3, 2, { kind: "road" }), "900,0": tile(900, 0) };
  const layer = kindLayer(tiles);

  it("gives each known parcel the colour of its kind and its joins", () => {
    expect(layer(1, 2)).toEqual({ color: "var(--_plaza)", top: false, left: true, topLeft: false });
    expect(layer(3, 2)?.color).toBe("var(--_road)");
  });

  it("gives nothing for an unknown parcel or a parcel outside the square", () => {
    expect(layer(2, 2)).toBeFalsy();
    expect(layer(900, 0)).toBeFalsy();
  });

  it("marks the parcels that are chosen, with a line on every side, and no other parcel", () => {
    const onSale = kindLayer({ "5,5": tile(5, 5, { price: 100 }), "6,5": tile(6, 5) }, (t) => t.price !== null);
    expect(onSale(5, 5)).toEqual({ color: "var(--_taken)", top: false, left: false, topLeft: false, mark: "var(--_sale)", edges: EDGE_TOP | EDGE_RIGHT | EDGE_BOTTOM | EDGE_LEFT });
    expect(onSale(6, 5)?.mark).toBeUndefined();
    expect(kindLayer({ "5,5": tile(5, 5, { price: 100 }) })(5, 5)?.mark).toBeUndefined();
  });

  it("gives joined marked parcels one outline: no line on the side where they join", () => {
    // An estate of 2 x 2 parcels on sale, from 5,5 to 6,6, and a parcel on sale next to it that is not joined.
    const estate = { price: 100, estateId: "1" };
    const onSale = kindLayer(
      {
        "5,6": tile(5, 6, estate),
        "6,6": tile(6, 6, { ...estate, left: true }),
        "5,5": tile(5, 5, { ...estate, top: true }),
        "6,5": tile(6, 5, { ...estate, top: true, left: true, topLeft: true }),
        "7,5": tile(7, 5, { price: 50 }),
      },
      (t) => t.price !== null,
    );
    expect(onSale(5, 6)?.edges).toBe(EDGE_TOP | EDGE_LEFT);
    expect(onSale(6, 6)?.edges).toBe(EDGE_TOP | EDGE_RIGHT);
    expect(onSale(5, 5)?.edges).toBe(EDGE_BOTTOM | EDGE_LEFT);
    expect(onSale(6, 5)?.edges).toBe(EDGE_BOTTOM | EDGE_RIGHT);
    expect(onSale(7, 5)?.edges).toBe(EDGE_TOP | EDGE_RIGHT | EDGE_BOTTOM | EDGE_LEFT);
  });
});

describe("the layer of marks over the atlas image", () => {
  const tiles: Tiles = { "5,5": tile(5, 5, { price: 100 }), "6,5": tile(6, 5), "900,0": tile(900, 0, { price: 100 }) };
  const layer = markLayer(tiles, (t) => t.price !== null);

  it("gives the chosen parcels the colour of the mark, joined on every side, with no outline", () => {
    expect(layer(5, 5)).toEqual({ color: "var(--_sale)", top: true, left: true, topLeft: true });
  });

  it("gives nothing for a parcel that is not chosen, an unknown parcel or a parcel outside the square", () => {
    expect(layer(6, 5)).toBeFalsy();
    expect(layer(7, 5)).toBeFalsy();
    expect(layer(900, 0)).toBeFalsy();
  });

  it("is painted as the whole parcel, with one draw, at a high and a low zoom", () => {
    const color = (value: string) => value;
    const high = fakeCanvas();
    drawLayers(high.ctx, { view: { x: 0, y: 0, zoom: 10 }, size: SHELL, dpr: 1, layers: [layer], color });
    expect(high.fillRect.mock.calls).toEqual([[215, 115, 10, 10]]);
    expect(high.fills).toEqual(["var(--_sale)"]);
    const low = fakeCanvas();
    drawLayers(low.ctx, { view: { x: GRID_MID, y: GRID_MID, zoom: 1 }, size: SHELL, dpr: 1, layers: [layer], color });
    expect(low.fillRect.mock.calls).toEqual([[175, 164, 1, 1]]);
  });
});

describe("the width of a mark's outline", () => {
  it("is an eighth of the parcel on whole device pixels, 2 px at the most", () => {
    expect(markLine(8, 1)).toBe(1);
    expect(markLine(15, 1)).toBe(1);
    expect(markLine(16, 1)).toBe(2);
    expect(markLine(40, 1)).toBe(2);
    expect(markLine(12, 2)).toBe(1.5);
  });

  it("is 0, for a fill, when the line is under 1 px or under 1 device pixel", () => {
    expect(markLine(7.9, 1)).toBe(0);
    expect(markLine(7.9, 2)).toBe(0);
    expect(markLine(12, 0.5)).toBe(0);
  });
});

describe("the painter", () => {
  const view = { x: 0, y: 0, zoom: 10 };
  const color = (value: string) => value;
  // With this view, parcel 0,0 is the square from 165 to 175 on both axes; the gap at 10 px is 1 px.

  it("clears the canvas in CSS pixels at the pixel ratio", () => {
    const c = fakeCanvas();
    drawLayers(c.ctx, { view, size: SHELL, dpr: 2, layers: [only(0, 0, { color: "red" })], color });
    expect(c.setTransform).toHaveBeenCalledWith(2, 0, 0, 2, 0, 0);
    expect(c.clearRect).toHaveBeenCalledWith(0, 0, 340, 340);
  });

  it("asks a layer for the parcels in view only", () => {
    const c = fakeCanvas();
    const layer = vi.fn(() => null);
    drawLayers(c.ctx, { view, size: SHELL, dpr: 1, layers: [layer], color });
    expect(layer).toHaveBeenCalledTimes(35 * 35);
    expect(c.fillRect).not.toHaveBeenCalled();
  });

  it("asks a layer for `overscan` parcels more on each side of the view, inside the grid", () => {
    const c = fakeCanvas();
    const layer = vi.fn(() => null);
    drawLayers(c.ctx, { view, size: SHELL, dpr: 1, layers: [layer], color, overscan: 1 });
    expect(layer).toHaveBeenCalledTimes(37 * 37);
    expect(layer).toHaveBeenCalledWith(-18, 18);
    expect(layer).toHaveBeenCalledWith(18, -18);
    layer.mockClear();
    // At zoom 1 the view is the whole grid: there is no parcel beyond it.
    drawLayers(c.ctx, { view: { x: GRID_MID, y: GRID_MID, zoom: 1 }, size: SHELL, dpr: 1, layers: [layer], color, overscan: 1 });
    expect(layer).toHaveBeenCalledTimes(340 * 340);
  });

  it("paints a parcel of the overscan at its own place, so a larger tile reaches into the view", () => {
    const c = fakeCanvas();
    // Parcel 0,18 is the row above the view: the square from 165 to 175 on x and from -15 to -5 on y.
    drawLayers(c.ctx, { view, size: SHELL, dpr: 1, layers: [only(0, 18, { color: "red", scale: 4 })], color, overscan: 1 });
    expect(c.fillRect.mock.calls).toEqual([[151, -29, 39, 39]]);
    c.fillRect.mockClear();
    drawLayers(c.ctx, { view, size: SHELL, dpr: 1, layers: [only(0, 18, { color: "red", scale: 4 })], color });
    expect(c.fillRect).not.toHaveBeenCalled();
  });

  it("paints a parcel with a gap on its left and top", () => {
    const c = fakeCanvas();
    drawLayers(c.ctx, { view, size: SHELL, dpr: 1, layers: [only(0, 0, { color: "red" })], color });
    expect(c.fills).toEqual(["red"]);
    expect(c.fillRect.mock.calls).toEqual([[166, 166, 9, 9]]);
  });

  it("closes the gap on a side where the parcel is joined", () => {
    const rects = (t: { left?: boolean; top?: boolean; topLeft?: boolean }) => {
      const c = fakeCanvas();
      drawLayers(c.ctx, { view, size: SHELL, dpr: 1, layers: [only(0, 0, { color: "red", ...t })], color });
      return c.fillRect.mock.calls;
    };
    expect(rects({ left: true })).toEqual([[165, 166, 10, 9]]);
    expect(rects({ top: true })).toEqual([[166, 165, 9, 10]]);
    expect(rects({ left: true, top: true, topLeft: true })).toEqual([[165, 165, 10, 10]]);
    // Joined on both sides but not across the corner: the corner stays open, and no pixel is painted twice.
    expect(rects({ left: true, top: true })).toEqual([
      [165, 166, 10, 9],
      [166, 165, 9, 1],
    ]);
  });

  it("paints no gap when a parcel is smaller than 4 px", () => {
    const c = fakeCanvas();
    drawLayers(c.ctx, { view: { x: GRID_MID, y: GRID_MID, zoom: 1 }, size: SHELL, dpr: 1, layers: [only(0, 0, { color: "red" })], color });
    expect(c.fillRect.mock.calls).toEqual([[170, 169, 1, 1]]);
  });

  // 240 px for 340 parcels (120 px at a pixel ratio of 2): two edges of some parcels are on the same pixel line.
  const SMALL = [
    { size: { w: 240, h: 240 }, dpr: 1 },
    { size: { w: 120, h: 120 }, dpr: 2 },
  ];
  const whole = { x: GRID_MID, y: GRID_MID, zoom: 1 };
  const rects = (c: ReturnType<typeof fakeCanvas>) => c.fillRect.mock.calls as [number, number, number, number][];

  it("gives a lone parcel under one device pixel one device pixel", () => {
    // One parcel of each 5 on both axes: no two of them are on the same device pixel.
    const lone: MapLayer = (x, y) => ((x - GRID_MIN) % 5 === 0 && (y - GRID_MIN) % 5 === 0 ? { color: "red" } : null);
    for (const { size, dpr } of SMALL) {
      const c = fakeCanvas();
      drawLayers(c.ctx, { view: whole, size, dpr, layers: [lone], color });
      const calls = rects(c);
      expect(calls).toHaveLength(68 * 68);
      expect(calls.every(([, , w, h]) => w === 1 / dpr && h === 1 / dpr)).toBe(true);
      expect(new Set(calls.map(([x, y]) => `${x},${y}`)).size).toBe(68 * 68);
    }
  });

  it("paints no device pixel twice when the parcels are under one device pixel", () => {
    const every: MapLayer = () => ({ color: "red" });
    for (const { size, dpr } of SMALL) {
      const c = fakeCanvas();
      drawLayers(c.ctx, { view: whole, size, dpr, layers: [every], color });
      const calls = rects(c);
      // Each device pixel of the grid one time: a parcel with no area adds nothing to a pixel that has paint.
      expect(calls).toHaveLength(240 * 240);
      expect(calls.every(([, , w, h]) => w === 1 / dpr && h === 1 / dpr)).toBe(true);
      expect(new Set(calls.map(([x, y]) => `${x},${y}`)).size).toBe(240 * 240);
    }
  });

  it("puts a parcel under one device pixel on its own pixel, in its own colour", () => {
    // A shell that is not square and a view off the centre. One parcel of each 7: no two on the same pixel.
    const size = { w: 300, h: 200 };
    const view = { x: GRID_MID + 10, y: GRID_MID + 20, zoom: 1 };
    const at = (x: number, y: number) => (x - GRID_MIN) % 7 === 3 && (y - GRID_MIN) % 7 === 3 && x < GRID_MAX && y > GRID_MIN;
    const tint = (x: number) => ((x - GRID_MIN) % 2 === 0 ? "red" : "blue");
    const lattice: MapLayer = (x, y) => (at(x, y) ? { color: tint(x) } : null);
    const s = parcelPx(size, 1);
    const want = new Map<string, string>();
    for (let x = GRID_MIN; x <= GRID_MAX; x++)
      for (let y = GRID_MIN; y <= GRID_MAX; y++)
        if (at(x, y)) want.set(`${Math.round(size.w / 2 + (x - 0.5 - view.x) * s)},${Math.round(size.h / 2 - (y + 0.5 - view.y) * s)}`, tint(x));
    const got = new Map<string, string>();
    let style = "";
    const ctx = {
      setTransform() {},
      clearRect() {},
      fillRect(x: number, y: number, w: number, h: number) {
        expect([w, h]).toEqual([1, 1]);
        expect(got.has(`${x},${y}`)).toBe(false);
        got.set(`${x},${y}`, style);
      },
      set fillStyle(value: string) {
        style = value;
      },
    };
    drawLayers(ctx as unknown as LayerCanvas, { view, size, dpr: 1, layers: [lattice], color });
    // The parcels in view only, each on the pixel of its top left corner.
    expect(got.size).toBeGreaterThan(1000);
    for (const [cell, colour] of got) expect(want.get(cell)).toBe(colour);
    expect(new Set(got.values())).toEqual(new Set(["red", "blue"]));
  });

  it("gives two parcels under one device pixel that have the same free pixel that pixel one time", () => {
    // Blocks of 2 by 2 parcels, 6 apart: some blocks have two parcels with no area on one device pixel.
    const blocks: MapLayer = (x, y) => ((x - GRID_MIN) % 6 < 2 && (y - GRID_MIN) % 6 < 2 ? { color: "red" } : null);
    for (const { size, dpr } of SMALL) {
      const c = fakeCanvas();
      drawLayers(c.ctx, { view: whole, size, dpr, layers: [blocks], color });
      const calls = rects(c);
      const cells = new Set(calls.map(([x, y]) => `${x},${y}`));
      expect(calls.every(([, , w, h]) => w === 1 / dpr && h === 1 / dpr)).toBe(true);
      expect(cells.size).toBe(calls.length);
      // Each block has paint, and fewer pixels than parcels: some parcels share a pixel.
      expect(calls.length).toBeGreaterThanOrEqual(57 * 57);
      expect(calls.length).toBeLessThan(4 * 57 * 57);
    }
  });

  it("keeps a parcel under one device pixel inside the grid", () => {
    // The last column and the last row only. At this size and pixel ratio, the two have no area.
    const border: MapLayer = (x, y) => (x === GRID_MAX || y === GRID_MIN ? { color: "red" } : null);
    const size = { w: 241, h: 185 };
    const dpr = 1.25;
    const c = fakeCanvas();
    drawLayers(c.ctx, { view: whole, size, dpr, layers: [border], color });
    const s = parcelPx(size, 1);
    const right = Math.round((size.w / 2 + 170 * s) * dpr);
    const bottom = Math.round((size.h / 2 + 170 * s) * dpr);
    const cells = rects(c).map(([x, y, w, h]) => [Math.round(x * dpr), Math.round(y * dpr), Math.round((x + w) * dpr), Math.round((y + h) * dpr)] as const);
    expect(cells.every(([x0, y0, x1, y1]) => x1 > x0 && y1 > y0 && x1 <= right && y1 <= bottom)).toBe(true);
    // The last pixel column and the last pixel row of the grid have the paint of the border.
    expect(cells.filter(([, , x1]) => x1 === right).length).toBeGreaterThan(100);
    expect(cells.filter(([, , , y1]) => y1 === bottom).length).toBeGreaterThan(100);
  });

  it("paints a parcel with a scale under one device pixel as at a larger size", () => {
    const row: MapLayer = (x, y) => (y === 0 && x >= 0 && x < 12 ? { color: "red", scale: 3 } : null);
    const c = fakeCanvas();
    drawLayers(c.ctx, { view: whole, size: { w: 240, h: 240 }, dpr: 1, layers: [row], color });
    const calls = rects(c);
    expect(calls).toHaveLength(12);
    expect(calls.every(([, , w, h]) => w > 1 && h > 1)).toBe(true);
  });

  it("paints the layers in order", () => {
    const c = fakeCanvas();
    drawLayers(c.ctx, { view, size: SHELL, dpr: 1, layers: [only(0, 0, { color: "red" }), only(0, 0, { color: "blue" })], color });
    expect(c.fills).toEqual(["red", "blue"]);
  });

  it("skips a parcel whose colour is not known", () => {
    const c = fakeCanvas();
    drawLayers(c.ctx, { view, size: SHELL, dpr: 1, layers: [only(0, 0, { color: "var(--unknown)" })], color: () => "" });
    expect(c.fillRect).not.toHaveBeenCalled();
  });
});

describe("the painter's mark", () => {
  const high = { x: 0, y: 0, zoom: 10 };
  const low = { x: GRID_MID, y: GRID_MID, zoom: 1 };
  const color = (value: string) => value;
  // At zoom 10, parcel 0,0 is painted from 166 to 175 on both axes and the line of the mark is 1 px.

  it("outlines a marked parcel at a high zoom, inside the parcel, and paints no line twice", () => {
    const c = fakeCanvas();
    drawLayers(c.ctx, { view: high, size: SHELL, dpr: 1, layers: [only(0, 0, { color: "red", mark: "white" })], color });
    expect(c.fills).toEqual(["red", "white"]);
    expect(c.fillRect.mock.calls).toEqual([
      [166, 166, 9, 9],
      [166, 166, 9, 1],
      [166, 174, 9, 1],
      [166, 167, 1, 7],
      [174, 167, 1, 7],
    ]);
  });

  it("paints a parcel that is not marked with no outline at a high zoom", () => {
    const c = fakeCanvas();
    drawLayers(c.ctx, { view: high, size: SHELL, dpr: 1, layers: [only(0, 0, { color: "red" })], color });
    expect(c.fills).toEqual(["red"]);
    expect(c.fillRect.mock.calls).toEqual([[166, 166, 9, 9]]);
  });

  it("paints only the sides in `edges`, up to the side where the parcel is joined", () => {
    const c = fakeCanvas();
    drawLayers(c.ctx, { view: high, size: SHELL, dpr: 1, layers: [only(0, 0, { color: "red", left: true, mark: "white", edges: EDGE_TOP | EDGE_BOTTOM })], color });
    expect(c.fillRect.mock.calls).toEqual([
      [165, 166, 10, 9],
      [165, 166, 10, 1],
      [165, 174, 10, 1],
    ]);
  });

  it("paints the line on whole device pixels", () => {
    const c = fakeCanvas();
    drawLayers(c.ctx, { view: { x: 0, y: 0, zoom: 12 }, size: SHELL, dpr: 2, layers: [only(0, 0, { color: "red", mark: "white", edges: EDGE_TOP })], color });
    expect(c.fillRect.mock.calls[1]).toEqual([165.5, 165.5, 10.5, 1.5]);
  });

  it("paints a marked parcel whole in the colour of the mark at a low zoom", () => {
    const c = fakeCanvas();
    drawLayers(c.ctx, { view: low, size: SHELL, dpr: 1, layers: [only(0, 0, { color: "red", mark: "white" })], color });
    expect(c.fills).toEqual(["white"]);
    expect(c.fillRect.mock.calls).toEqual([[170, 169, 1, 1]]);
  });

  it("paints a parcel that is not marked in its own colour at a low zoom", () => {
    const c = fakeCanvas();
    drawLayers(c.ctx, { view: low, size: SHELL, dpr: 1, layers: [only(0, 0, { color: "red" })], color });
    expect(c.fills).toEqual(["red"]);
    expect(c.fillRect.mock.calls).toEqual([[170, 169, 1, 1]]);
  });

  it("paints the parcel in its own colour when the colour of the mark is not known", () => {
    const c = fakeCanvas();
    drawLayers(c.ctx, { view: low, size: SHELL, dpr: 1, layers: [only(0, 0, { color: "red", mark: "var(--unknown)" })], color: (value) => (value === "red" ? value : "") });
    expect(c.fills).toEqual(["red"]);
  });

  it("costs one read for each parcel in view, no draw at a low zoom and four draws for each mark at a high zoom", () => {
    // LAND on every parcel in view, one parcel in 25 on sale.
    const tiles: Tiles = {};
    for (let x = -30; x <= 30; x++) for (let y = -30; y <= 30; y++) tiles[`${x},${y}`] = tile(x, y, { price: x % 5 === 0 && y % 5 === 0 ? 100 : null });
    const draws = (v: typeof high, marked: boolean) => {
      const c = fakeCanvas();
      const layer = vi.fn(kindLayer(tiles, marked ? (t) => t.price !== null : undefined));
      drawLayers(c.ctx, { view: v, size: SHELL, dpr: 1, layers: [layer], color });
      return { reads: layer.mock.calls.length, rects: c.fillRect.mock.calls.length };
    };
    // At zoom 10 the shell shows 35 x 35 parcels, 49 of them on sale.
    expect(draws(high, false)).toEqual({ reads: 35 * 35, rects: 35 * 35 });
    expect(draws(high, true)).toEqual({ reads: 35 * 35, rects: 35 * 35 + 4 * 49 });
    // At zoom 1 it shows the square of 340 x 340 parcels, with LAND on 61 x 61 of them.
    expect(draws(low, false)).toEqual({ reads: 340 * 340, rects: 61 * 61 });
    expect(draws(low, true)).toEqual({ reads: 340 * 340, rects: 61 * 61 });
  });
});
