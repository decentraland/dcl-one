import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import LandMap from "./LandMap";
import { sampleTiles } from "./fixtures";
import type { LandTile, Tiles } from "./tiles";

// jsdom has no layout and no canvas. The shell is 340 x 340 at 10,20 of the page: at zoom 10 a parcel
// is 10 px, and the middle of parcel x,y is at 175 + 10x, 165 - 10y of the shell.
const SHELL = { left: 10, top: 20, width: 340, height: 340 };
const at = (x: number, y: number) => ({ clientX: SHELL.left + x, clientY: SHELL.top + y });
const parcel = (x: number, y: number) => at(175 + 10 * x, 165 - 10 * y);

const KIND_COLORS: Record<string, string> = { "--_taken": "#111111", "--_unowned": "#222222", "--_road": "#333333", "--_plaza": "#444444", "--_district": "#555555", "--_sale": "#666666" };

/** Gives the shell a size, the canvas a 2D context that records, and runs animation frames at once. */
function stubBrowser() {
  vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(
    () => ({ ...SHELL, right: SHELL.left + SHELL.width, bottom: SHELL.top + SHELL.height, x: SHELL.left, y: SHELL.top, toJSON: () => ({}) }) as DOMRect,
  );
  const fills: string[] = [];
  // The rectangles of the last paint, each with its colour.
  const rects: { color: string; x: number; y: number; w: number; h: number }[] = [];
  let current = "";
  const ctx = {
    setTransform: vi.fn(),
    clearRect: vi.fn(() => {
      rects.length = 0;
    }),
    fillRect: vi.fn((x: number, y: number, w: number, h: number) => {
      rects.push({ color: current, x, y, w, h });
    }),
    set fillStyle(value: string) {
      current = value;
      fills.push(value);
    },
  };
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockImplementation((() => ctx) as never);
  vi.stubGlobal("requestAnimationFrame", (run: FrameRequestCallback) => {
    run(0);
    return 1;
  });
  vi.stubGlobal("cancelAnimationFrame", () => {});
  const real = window.getComputedStyle.bind(window);
  vi.spyOn(window, "getComputedStyle").mockImplementation((el, pseudo) => {
    const style = real(el, pseudo);
    if (!(el instanceof HTMLCanvasElement)) return style;
    return { getPropertyValue: (name: string) => KIND_COLORS[name] ?? style.getPropertyValue(name) } as CSSStyleDeclaration;
  });
  return { ctx, fills, rects };
}

/** LAND on every parcel from -20 to 20 on both axes. */
function landGrid(): Tiles {
  const tiles: Tiles = {};
  for (let x = -20; x <= 20; x++) {
    for (let y = -20; y <= 20; y++) {
      const id = `${x},${y}`;
      tiles[id] = { id, x, y, kind: "owned", top: false, left: false, topLeft: false, owner: "0x" + "ab".repeat(20), name: null, estateId: null, price: null, rentalPricePerDay: null };
    }
  }
  return tiles;
}

/** The grid with parcel 2,3 on sale and parcel -5,1 for rent. */
function listed(): Tiles {
  const tiles = landGrid();
  tiles["2,3"] = { ...tiles["2,3"]!, price: 4000 };
  tiles["-5,1"] = { ...tiles["-5,1"]!, rentalPricePerDay: "1" };
  return tiles;
}

const SURFACES = ["atlas", "tiles"] as const;

/** The element of the map that takes the pointer: the first in the region. Nothing else names it. */
const pointerTarget = (container: HTMLElement) => container.querySelector('[role="region"]')!.firstElementChild as HTMLElement;

/** A map painted from the tile data, unless the test asks for the atlas image. */
function renderMap(props: Partial<Parameters<typeof LandMap>[0]> = {}) {
  const onOpen = vi.fn<(href: string, tile: LandTile) => void>();
  const view = render(<LandMap tiles={landGrid()} surface="tiles" initialZoom={10} onOpen={onOpen} label="Test map" {...props} />);
  const target = pointerTarget(view.container);
  const opened = () => onOpen.mock.calls.map(([, tile]) => tile.id);
  const click = (init: { clientX: number; clientY: number }, pointerId = 1) => {
    fireEvent.pointerDown(target, { pointerId, button: 0, ...init });
    fireEvent.pointerUp(target, { pointerId, button: 0, ...init });
  };
  return { ...view, target, onOpen, opened, click };
}

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("LandMap base", () => {
  it("paints the known parcels on one canvas, each kind in its colour, on a map of tile data", () => {
    const { ctx, fills } = stubBrowser();
    const { container } = render(<LandMap tiles={sampleTiles()} surface="tiles" initialZoom={10} />);
    expect(container.querySelectorAll("canvas").length).toBe(1);
    expect(ctx.fillRect).toHaveBeenCalled();
    expect(new Set(fills)).toEqual(new Set([KIND_COLORS["--_road"], KIND_COLORS["--_plaza"], KIND_COLORS["--_taken"]]));
    expect(container.querySelector("img")).toBeNull();
  });

  it("paints again when the map moves", () => {
    const { ctx } = stubBrowser();
    const { target } = renderMap();
    const before = ctx.clearRect.mock.calls.length;
    fireEvent.pointerDown(target, { pointerId: 1, button: 0, ...parcel(8, -4) });
    fireEvent.pointerMove(target, { pointerId: 1, ...parcel(4, -4) });
    expect(ctx.clearRect.mock.calls.length).toBe(before + 1);
  });

  it("shows the atlas image, and no canvas, while there is no tile data", () => {
    stubBrowser();
    const { container } = render(<LandMap tiles={null} />);
    expect(container.querySelector("canvas")).toBeNull();
    expect(container.querySelector("img")).not.toBeNull();
  });

  it("shows no image and no canvas while a map of tile data waits for the data", () => {
    stubBrowser();
    const { container } = render(<LandMap tiles={null} surface="tiles" />);
    expect(container.querySelector("canvas")).toBeNull();
    expect(container.querySelector("img")).toBeNull();
  });

  it("does not read every tile again when the pointer moves", () => {
    stubBrowser();
    const tiles = landGrid();
    const values = vi.spyOn(Object, "values");
    const passes = () => values.mock.calls.filter(([o]) => o === tiles).length;
    const { target } = renderMap({ tiles });
    const mounted = passes();
    expect(mounted).toBeGreaterThan(0);
    for (let i = 0; i < 5; i++) fireEvent.pointerMove(target, { pointerId: 1, ...parcel(i, 3) });
    fireEvent.pointerDown(target, { pointerId: 1, button: 0, ...parcel(8, -4) });
    for (let i = 1; i <= 5; i++) fireEvent.pointerMove(target, { pointerId: 1, ...parcel(8 - i, -4) });
    fireEvent.pointerUp(target, { pointerId: 1, button: 0, ...parcel(3, -4) });
    expect(passes()).toBe(mounted);
  });
});

describe("LandMap over the atlas image", () => {
  const SALE = KIND_COLORS["--_sale"]!;

  it("keeps the atlas image as its base after the tile data arrives, and paints no parcel of the base", () => {
    const { rects } = stubBrowser();
    const { container, rerender } = render(<LandMap tiles={null} initialZoom={10} />);
    expect(container.querySelectorAll("img").length).toBe(1);
    rerender(<LandMap tiles={landGrid()} initialZoom={10} />);
    expect(container.querySelectorAll("img").length).toBe(1);
    // No layer is on: there is nothing to paint, so there is no canvas.
    expect(container.querySelector("canvas")).toBeNull();
    expect(rects).toEqual([]);
  });

  it("paints the LAND on sale as the whole parcel in the colour of the mark at a high and a low zoom, and no other parcel", () => {
    // Parcel 2,3 is from 190 to 200 across and from 130 to 140 down at zoom 10, and 1 px at 172,166 at zoom 1.
    const cases = [
      { zoom: 10, rect: { x: 190, y: 130, w: 10, h: 10 } },
      { zoom: 1, rect: { x: 172, y: 166, w: 1, h: 1 } },
    ];
    for (const { zoom, rect } of cases) {
      const { rects, fills } = stubBrowser();
      const { container, unmount } = render(<LandMap tiles={listed()} initialZoom={zoom} layers={{ onSale: true }} />);
      expect(rects).toEqual([{ color: SALE, ...rect }]);
      expect(new Set(fills)).toEqual(new Set([SALE]));
      expect(container.querySelectorAll("img").length).toBe(1);
      unmount();
      vi.restoreAllMocks();
    }
  });

  it("paints the LAND for rent in the same way when that layer is on", () => {
    const { rects } = stubBrowser();
    render(<LandMap tiles={listed()} initialZoom={10} layers={{ forRent: true }} />);
    expect(rects).toEqual([{ color: SALE, x: 120, y: 150, w: 10, h: 10 }]);
  });

  it("removes the paint when the layers go off", () => {
    stubBrowser();
    const tiles = listed();
    const { container, rerender } = render(<LandMap tiles={tiles} initialZoom={10} layers={{ onSale: true }} />);
    expect(container.querySelector("canvas")).not.toBeNull();
    rerender(<LandMap tiles={tiles} initialZoom={10} layers={{}} />);
    expect(container.querySelector("canvas")).toBeNull();
    expect(container.querySelectorAll("img").length).toBe(1);
  });

  it("has the canvas over the image and under the account's LAND, all at one place", () => {
    stubBrowser();
    const { container } = render(<LandMap tiles={listed()} initialZoom={10} layers={{ onSale: true, owned: new Set(["2,3"]) }} />);
    const img = container.querySelector("img")!;
    const canvas = container.querySelector("canvas")!;
    const own = container.querySelector(".shop-landmap__owned")!;
    expect(img.compareDocumentPosition(canvas) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(canvas.compareDocumentPosition(own) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    const stages = [...container.querySelectorAll<HTMLElement>(".shop-landmap__stage")];
    expect(stages.length).toBe(2);
    expect(stages[0]!.style.transform).toBe(stages[1]!.style.transform);
    expect(stages[0]!.style.width).toBe(stages[1]!.style.width);
  });

  it("takes the colour of the mark from the style sheet: the cyan of the Shop at 0.85, and the text colour on a map of tile data", () => {
    stubBrowser();
    const { container, rerender } = render(<LandMap tiles={null} />);
    const mark = () => window.getComputedStyle(container.querySelector('[role="region"]')!).getPropertyValue("--_sale").replace(/\s+/g, "");
    expect(mark()).toBe("rgba(0,211,255,0.85)");
    rerender(<LandMap tiles={null} surface="tiles" />);
    expect(mark()).toBe("var(--text)");
  });

  it("has the legend of the image: no district row", () => {
    stubBrowser();
    renderMap({ surface: "atlas", withLegend: true });
    fireEvent.click(screen.getByRole("button", { name: "Map colors" }));
    expect(screen.getAllByRole("listitem").map((li) => li.textContent?.trim())).toEqual(["Plaza", "Your LAND", "Your rented LAND", "On sale or on rent", "Taken"]);
  });
});

describe("LandMap LAND on sale, on a map of tile data", () => {
  const SALE = KIND_COLORS["--_sale"];

  it("outlines a parcel on sale at a high zoom, and no parcel that is not on sale", () => {
    const { rects } = stubBrowser();
    render(<LandMap tiles={listed()} surface="tiles" initialZoom={10} layers={{ onSale: true }} />);
    const marks = rects.filter((r) => r.color === SALE);
    // Four lines, all inside parcel 2,3: from 190 to 200 across and from 130 to 140 down.
    expect(marks.length).toBe(4);
    for (const r of marks) {
      expect(r.x).toBeGreaterThanOrEqual(190);
      expect(r.x + r.w).toBeLessThanOrEqual(200);
      expect(r.y).toBeGreaterThanOrEqual(130);
      expect(r.y + r.h).toBeLessThanOrEqual(140);
      expect(Math.min(r.w, r.h)).toBe(1);
    }
  });

  it("paints a parcel on sale in the colour of the mark at a low zoom, and no parcel that is not on sale", () => {
    const { rects } = stubBrowser();
    render(<LandMap tiles={listed()} surface="tiles" initialZoom={1} layers={{ onSale: true }} />);
    // At zoom 1 a parcel is 1 px: parcel 2,3 is at 172,166.
    expect(rects.filter((r) => r.color === SALE)).toEqual([{ color: SALE, x: 172, y: 166, w: 1, h: 1 }]);
    expect(rects.length).toBe(41 * 41);
  });

  it("marks the LAND for rent with the same mark when that layer is on", () => {
    const { rects } = stubBrowser();
    render(<LandMap tiles={listed()} surface="tiles" initialZoom={1} layers={{ onSale: true, forRent: true }} />);
    expect(rects.filter((r) => r.color === SALE).map((r) => [r.x, r.y])).toEqual([
      [172, 166],
      [165, 168],
    ]);
  });

  it("marks no parcel when the layers are off, at a high or a low zoom", () => {
    for (const zoom of [10, 1]) {
      const { rects } = stubBrowser();
      const { unmount } = render(<LandMap tiles={listed()} surface="tiles" initialZoom={zoom} />);
      expect(rects.length).toBeGreaterThan(0);
      expect(rects.filter((r) => r.color === SALE)).toEqual([]);
      unmount();
      vi.restoreAllMocks();
    }
  });

  it("shows the mark in the legend", () => {
    stubBrowser();
    renderMap({ withLegend: true });
    fireEvent.click(screen.getByRole("button", { name: "Map colors" }));
    expect(screen.getByText("On sale or on rent")).toBeInTheDocument();
  });
});

describe.each(SURFACES)("LandMap pointer, surface %s", (surface) => {
  it("opens the parcel that is clicked", () => {
    stubBrowser();
    const { click, onOpen, opened } = renderMap({ surface });
    click(parcel(8, -4));
    expect(opened()).toEqual(["8,-4"]);
    expect(onOpen.mock.calls[0]![0]).toMatch(/^\/shop\/token\//);
  });

  it("moves the map with a drag of the primary button, and a drag opens nothing", () => {
    stubBrowser();
    const { target, click, opened } = renderMap({ surface });
    fireEvent.pointerDown(target, { pointerId: 1, button: 0, ...parcel(8, -4) });
    fireEvent.pointerMove(target, { pointerId: 1, ...parcel(4, -4) });
    fireEvent.pointerUp(target, { pointerId: 1, button: 0, ...parcel(4, -4) });
    expect(opened()).toEqual([]);
    // The map moved four parcels to the left: the same point is now four parcels further east.
    click(parcel(8, -4));
    expect(opened()).toEqual(["12,-4"]);
  });

  it("does not move the map or open a parcel with another button", () => {
    stubBrowser();
    const { target, click, opened } = renderMap({ surface });
    fireEvent.pointerDown(target, { pointerId: 1, button: 2, ...parcel(8, -4) });
    fireEvent.pointerMove(target, { pointerId: 1, ...parcel(4, -4) });
    fireEvent.pointerUp(target, { pointerId: 1, button: 2, ...parcel(4, -4) });
    fireEvent.pointerDown(target, { pointerId: 1, button: 1, ...parcel(8, -4) });
    fireEvent.pointerUp(target, { pointerId: 1, button: 1, ...parcel(8, -4) });
    expect(opened()).toEqual([]);
    click(parcel(8, -4));
    expect(opened()).toEqual(["8,-4"]);
  });

  it("captures the pointer, so a drag goes on after the pointer leaves the map", () => {
    stubBrowser();
    const { target, click, opened } = renderMap({ surface });
    const taker = target;
    const captured = new Set<number>();
    taker.setPointerCapture = (id) => void captured.add(id);
    taker.hasPointerCapture = (id) => captured.has(id);
    fireEvent.pointerDown(target, { pointerId: 7, button: 0, ...parcel(8, -4) });
    expect([...captured]).toEqual([7]);
    fireEvent.pointerMove(target, { pointerId: 7, ...parcel(6, -4) });
    fireEvent.pointerLeave(taker, { pointerId: 7 });
    fireEvent.pointerMove(target, { pointerId: 7, ...parcel(4, -4) });
    fireEvent.pointerUp(target, { pointerId: 7, button: 0, ...parcel(4, -4) });
    click(parcel(8, -4));
    expect(opened()).toEqual(["12,-4"]);
  });

  it("ends the drag on a pointer cancel", () => {
    stubBrowser();
    const { target, click, opened } = renderMap({ surface });
    fireEvent.pointerDown(target, { pointerId: 1, button: 0, ...parcel(8, -4) });
    fireEvent.pointerMove(target, { pointerId: 1, ...parcel(6, -4) });
    fireEvent.pointerCancel(target, { pointerId: 1 });
    fireEvent.pointerMove(target, { pointerId: 1, ...parcel(2, -4) });
    fireEvent.pointerUp(target, { pointerId: 1, button: 0, ...parcel(2, -4) });
    expect(opened()).toEqual([]);
    // Only the two parcels before the cancel count.
    click(parcel(8, -4));
    expect(opened()).toEqual(["10,-4"]);
  });

  it("zooms about the pointer: the parcel under it stays under it", () => {
    stubBrowser();
    const { target, click, opened } = renderMap({ surface });
    click(parcel(10, 10));
    fireEvent.wheel(target, { ctrlKey: true, deltaY: -200, ...parcel(10, 10) });
    expect(screen.getByRole("button", { name: "Zoom in" })).toBeDisabled();
    click(parcel(10, 10));
    expect(opened()).toEqual(["10,10", "10,10"]);
  });

  it("leaves the wheel to the page without Ctrl or Meta", () => {
    stubBrowser();
    const { target } = renderMap({ surface });
    fireEvent.wheel(target, { deltaY: -200, ...parcel(10, 10) });
    expect(screen.getByRole("button", { name: "Zoom in" })).toBeEnabled();
  });

  it("does not open the parcel under the zoom buttons or the legend button", () => {
    stubBrowser();
    const { opened } = renderMap({ surface, withLegend: true });
    for (const name of ["Zoom in", "Zoom out", "Map colors"]) {
      const button = screen.getByRole("button", { name });
      fireEvent.pointerDown(button, { pointerId: 1, button: 0, ...parcel(8, -4) });
      fireEvent.pointerUp(button, { pointerId: 1, button: 0, ...parcel(8, -4) });
    }
    expect(opened()).toEqual([]);
  });
});

describe.each(SURFACES)("LandMap popup, surface %s", (surface) => {
  it("is below and right of the pointer", () => {
    stubBrowser();
    const { target } = renderMap({ surface });
    fireEvent.pointerMove(target, { pointerId: 1, ...at(20, 20) });
    const popup = screen.getByRole("tooltip");
    expect(popup).toHaveTextContent("-15,14");
    expect(popup.style.left).toBe("34px");
    expect(popup.style.top).toBe("34px");
  });

  it("goes to the other side near the right and bottom edges", () => {
    stubBrowser();
    const { target } = renderMap({ surface });
    fireEvent.pointerMove(target, { pointerId: 1, ...at(330, 330) });
    const popup = screen.getByRole("tooltip");
    expect(popup.style.right).toBe("20px");
    expect(popup.style.bottom).toBe("20px");
    expect(popup.style.left).toBe("");
    expect(popup.style.top).toBe("");
  });

  it("is not shown with withPopup off", () => {
    stubBrowser();
    const { target } = renderMap({ surface, withPopup: false });
    fireEvent.pointerMove(target, { pointerId: 1, ...at(20, 20) });
    expect(screen.queryByRole("tooltip")).toBeNull();
  });
});

describe("LandMap props", () => {
  it("names the region with the label", () => {
    stubBrowser();
    renderMap();
    expect(screen.getByRole("region", { name: "Test map" })).toBeInTheDocument();
  });

  it("puts `center` at the middle of the map, and follows it when it changes", () => {
    stubBrowser();
    const tiles = landGrid();
    const onOpen = vi.fn<(href: string, tile: LandTile) => void>();
    const { container, rerender } = render(<LandMap tiles={tiles} initialZoom={10} onOpen={onOpen} center={{ x: 12, y: -4 }} />);
    const target = pointerTarget(container);
    const clickMiddle = () => {
      fireEvent.pointerDown(target, { pointerId: 1, button: 0, ...at(172, 168) });
      fireEvent.pointerUp(target, { pointerId: 1, button: 0, ...at(172, 168) });
    };
    clickMiddle();
    rerender(<LandMap tiles={tiles} initialZoom={10} onOpen={onOpen} center={{ x: -6, y: 9 }} />);
    clickMiddle();
    expect(onOpen.mock.calls.map(([, tile]) => tile.id)).toEqual(["12,-4", "-6,9"]);
  });

  it("keeps the view on a resize, and does not go back to `center`", () => {
    stubBrowser();
    let resized = () => {};
    vi.stubGlobal(
      "ResizeObserver",
      class {
        constructor(run: () => void) {
          resized = run;
        }
        observe() {}
        disconnect() {}
      },
    );
    const onViewChange = vi.fn();
    const { target } = renderMap({ onViewChange, center: { x: 0, y: 0 } });
    fireEvent.pointerDown(target, { pointerId: 1, button: 0, ...parcel(8, -4) });
    fireEvent.pointerMove(target, { pointerId: 1, ...parcel(4, -4) });
    fireEvent.pointerUp(target, { pointerId: 1, button: 0, ...parcel(4, -4) });
    const before = onViewChange.mock.calls.at(-1)![0];
    vi.mocked(Element.prototype.getBoundingClientRect).mockImplementation(() => ({ ...SHELL, width: 400, height: 300, right: SHELL.left + 400, bottom: SHELL.top + 300, x: SHELL.left, y: SHELL.top, toJSON: () => ({}) }) as DOMRect);
    act(() => resized());
    const after = onViewChange.mock.calls.at(-1)![0];
    expect(after.nw).not.toEqual(before.nw);
    expect(after.center).toEqual({ x: 4, y: 0 });
  });

  it("zooms with the buttons up to the limit", () => {
    stubBrowser();
    renderMap();
    fireEvent.click(screen.getByRole("button", { name: "Zoom in" }));
    expect(screen.getByRole("button", { name: "Zoom in" })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "Zoom out" }));
    expect(screen.getByRole("button", { name: "Zoom in" })).toBeEnabled();
  });

  it("opens the legend from its button", () => {
    stubBrowser();
    renderMap({ withLegend: true });
    fireEvent.click(screen.getByRole("button", { name: "Map colors" }));
    expect(screen.getByText("Taken")).toBeInTheDocument();
  });

  it("names the district colour in the legend of a map of tile data", () => {
    stubBrowser();
    renderMap({ withLegend: true });
    fireEvent.click(screen.getByRole("button", { name: "Map colors" }));
    expect(screen.getByText("District")).toBeInTheDocument();
  });
});

describe("LandMap layers and parcel callbacks", () => {
  it("paints the given layers over the tile data, in their order", () => {
    const { rects } = stubBrowser();
    renderMap({ paintLayers: [(x, y) => (x === 2 && y === 3 ? { color: "#aa0000" } : null), (x, y) => (x === 2 && y === 3 ? { color: "#00aa00" } : null)] });
    const colors = rects.map((r) => r.color);
    expect(colors.filter((c) => c === "#aa0000")).toHaveLength(1);
    expect(colors.filter((c) => c === "#00aa00")).toHaveLength(1);
    expect(colors.indexOf("#111111")).toBeLessThan(colors.indexOf("#aa0000"));
    expect(colors.indexOf("#aa0000")).toBeLessThan(colors.indexOf("#00aa00"));
  });

  it("paints the layers on a canvas when there is no tile data", () => {
    const { rects } = stubBrowser();
    const { container } = render(<LandMap tiles={null} surface="tiles" initialZoom={10} paintLayers={[(x, y) => (x === 0 && y === 0 ? { color: "#aa0000" } : null)]} />);
    expect(container.querySelector("canvas")).not.toBeNull();
    expect(rects.map((r) => r.color)).toEqual(["#aa0000"]);
  });

  it("paints again when the layers change", () => {
    const { rects } = stubBrowser();
    const tiles = landGrid();
    const { rerender } = render(<LandMap tiles={tiles} surface="tiles" initialZoom={10} paintLayers={[(x, y) => (x === 1 && y === 1 ? { color: "#aa0000" } : null)]} />);
    expect(rects.some((r) => r.color === "#aa0000")).toBe(true);
    rerender(<LandMap tiles={tiles} surface="tiles" initialZoom={10} paintLayers={[(x, y) => (x === 1 && y === 1 ? { color: "#0000aa" } : null)]} />);
    expect(rects.some((r) => r.color === "#aa0000")).toBe(false);
    expect(rects.some((r) => r.color === "#0000aa")).toBe(true);
  });

  it("tells the parcel under the pointer once for each parcel", () => {
    stubBrowser();
    const onParcelHover = vi.fn();
    const { target } = renderMap({ onParcelHover });
    fireEvent.pointerMove(target, { pointerId: 1, ...parcel(3, 2) });
    fireEvent.pointerMove(target, { pointerId: 1, ...at(175 + 31, 165 - 21) });
    fireEvent.pointerMove(target, { pointerId: 1, ...parcel(4, 2) });
    expect(onParcelHover.mock.calls).toEqual([
      [3, 2],
      [4, 2],
    ]);
  });

  it("tells the press, the release and the click of a parcel, and a drag is no click", () => {
    stubBrowser();
    const onParcelDown = vi.fn();
    const onParcelUp = vi.fn();
    const onParcelClick = vi.fn();
    const { target, click } = renderMap({ onParcelDown, onParcelUp, onParcelClick, onOpen: undefined });
    click(parcel(8, -4));
    expect(onParcelDown.mock.calls).toEqual([[8, -4]]);
    expect(onParcelUp.mock.calls).toEqual([[8, -4]]);
    expect(onParcelClick.mock.calls).toEqual([[8, -4]]);
    fireEvent.pointerDown(target, { pointerId: 1, button: 0, ...parcel(8, -4) });
    fireEvent.pointerMove(target, { pointerId: 1, ...parcel(4, -4) });
    fireEvent.pointerUp(target, { pointerId: 1, button: 0, ...parcel(4, -4) });
    expect(onParcelClick).toHaveBeenCalledTimes(1);
    expect(onParcelUp).toHaveBeenCalledTimes(2);
  });

  it("does not move with a drag when `draggable` is off, and tells the parcels the drag goes over", () => {
    stubBrowser();
    const onParcelHover = vi.fn();
    const onParcelUp = vi.fn();
    const { target } = renderMap({ draggable: false, onParcelHover, onParcelUp });
    fireEvent.pointerDown(target, { pointerId: 1, button: 0, ...parcel(8, -4) });
    fireEvent.pointerMove(target, { pointerId: 1, ...parcel(4, -4) });
    fireEvent.pointerUp(target, { pointerId: 1, button: 0, ...parcel(4, -4) });
    expect(onParcelHover.mock.calls).toEqual([[4, -4]]);
    expect(onParcelUp.mock.calls).toEqual([[4, -4]]);
  });

  it("moves with a drag of another button when `dragWithAnyButton` is on, and that is no click", () => {
    stubBrowser();
    const onParcelClick = vi.fn();
    const { target, click } = renderMap({ dragWithAnyButton: true, onParcelClick });
    fireEvent.pointerDown(target, { pointerId: 1, button: 2, ...parcel(8, -4) });
    fireEvent.pointerMove(target, { pointerId: 1, ...parcel(4, -4) });
    fireEvent.pointerUp(target, { pointerId: 1, button: 2, ...parcel(4, -4) });
    fireEvent.pointerDown(target, { pointerId: 1, button: 2, ...parcel(0, 0) });
    fireEvent.pointerUp(target, { pointerId: 1, button: 2, ...parcel(0, 0) });
    expect(onParcelClick).not.toHaveBeenCalled();
    click(parcel(8, -4));
    expect(onParcelClick.mock.calls).toEqual([[12, -4]]);
  });

  it("tells the view at the start and after each move", () => {
    stubBrowser();
    const onViewChange = vi.fn();
    const { target } = renderMap({ onViewChange, center: { x: 0, y: 0 } });
    const first = onViewChange.mock.calls.at(-1)![0];
    expect(first.center).toEqual({ x: 0, y: 0 });
    expect(first.nw).toEqual({ x: -17, y: 17 });
    expect(first.se).toEqual({ x: 17, y: -17 });
    fireEvent.pointerDown(target, { pointerId: 1, button: 0, ...parcel(8, -4) });
    fireEvent.pointerMove(target, { pointerId: 1, ...parcel(4, -4) });
    fireEvent.pointerUp(target, { pointerId: 1, button: 0, ...parcel(4, -4) });
    expect(onViewChange.mock.calls.at(-1)![0].center).toEqual({ x: 4, y: 0 });
  });

  it("zooms on a plain wheel turn with `wheelZoom` always", () => {
    stubBrowser();
    const onViewChange = vi.fn();
    const { target } = renderMap({ wheelZoom: "always", initialZoom: 5, onViewChange });
    fireEvent.wheel(target, { deltaY: -200, ...parcel(0, 0) });
    expect(onViewChange.mock.calls.at(-1)![0].zoom).toBeGreaterThan(5);
  });

  it("keeps the zoom inside `minZoom` and `maxZoom`", () => {
    stubBrowser();
    const onViewChange = vi.fn();
    renderMap({ initialZoom: 10, minZoom: 8, maxZoom: 20, onViewChange });
    fireEvent.click(screen.getByRole("button", { name: "Zoom in" }));
    expect(onViewChange.mock.calls.at(-1)![0].zoom).toBe(15);
    fireEvent.click(screen.getByRole("button", { name: "Zoom in" }));
    expect(onViewChange.mock.calls.at(-1)![0].zoom).toBe(20);
    expect(screen.getByRole("button", { name: "Zoom in" })).toBeDisabled();
    for (let i = 0; i < 4; i++) fireEvent.click(screen.getByRole("button", { name: "Zoom out" }));
    expect(onViewChange.mock.calls.at(-1)![0].zoom).toBe(8);
    expect(screen.getByRole("button", { name: "Zoom out" })).toBeDisabled();
  });

  it("has no zoom buttons with `withZoomControls` off", () => {
    stubBrowser();
    renderMap({ withZoomControls: false });
    expect(screen.queryByRole("button", { name: "Zoom in" })).toBeNull();
  });

  it("keeps the default names and texts of the zoom buttons", () => {
    stubBrowser();
    renderMap();
    expect(screen.getByRole("button", { name: "Zoom in" })).toHaveTextContent("+");
    expect(screen.getByRole("button", { name: "Zoom out" })).toHaveTextContent("\u2212");
  });

  it("names and labels the zoom buttons from `zoomButtons`", () => {
    stubBrowser();
    renderMap({ zoomButtons: { inLabel: "Add zoom", outLabel: "Decrease zoom", outText: "-" } });
    expect(screen.getByRole("button", { name: "Add zoom" })).toHaveTextContent("+");
    expect(screen.getByRole("button", { name: "Decrease zoom" })).toHaveTextContent("-");
    expect(screen.queryByRole("button", { name: "Zoom in" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Zoom out" })).toBeNull();
  });

  it("takes the next zoom of a button from `zoomStep`, inside the zoom range", () => {
    stubBrowser();
    const onViewChange = vi.fn();
    const zoomStep = vi.fn((zoom: number, direction: 1 | -1) => zoom + direction * 2);
    renderMap({ initialZoom: 10, minZoom: 9, maxZoom: 20, zoomStep, onViewChange });
    fireEvent.click(screen.getByRole("button", { name: "Zoom in" }));
    expect(onViewChange.mock.calls.at(-1)![0].zoom).toBe(12);
    fireEvent.click(screen.getByRole("button", { name: "Zoom out" }));
    fireEvent.click(screen.getByRole("button", { name: "Zoom out" }));
    expect(onViewChange.mock.calls.at(-1)![0].zoom).toBe(9);
    expect(screen.getByRole("button", { name: "Zoom out" })).toBeDisabled();
  });

  it("paints in the task of the render with `paintInTask`, with no frame request", () => {
    const { rects } = stubBrowser();
    const frames = vi.fn(() => 1);
    vi.stubGlobal("requestAnimationFrame", frames);
    renderMap({ paintInTask: true });
    expect(frames).not.toHaveBeenCalled();
    expect(rects.length).toBeGreaterThan(0);
  });

  it("paints the marks over the atlas image in the task of the render, so they move with the image", () => {
    const { rects } = stubBrowser();
    const frames = vi.fn(() => 1);
    vi.stubGlobal("requestAnimationFrame", frames);
    renderMap({ surface: "atlas", tiles: listed(), layers: { onSale: true } });
    expect(frames).not.toHaveBeenCalled();
    expect(rects.length).toBeGreaterThan(0);
  });

  it("waits for a frame to paint without `paintInTask`", () => {
    const { rects } = stubBrowser();
    const frames = vi.fn(() => 1);
    vi.stubGlobal("requestAnimationFrame", frames);
    renderMap();
    expect(frames).toHaveBeenCalled();
    expect(rects.length).toBe(0);
  });

  it("paints the parcels of `overscan` beyond the view, and none of them without it", () => {
    const { rects } = stubBrowser();
    // Parcel -18,0 is the column on the left of the view.
    const beyond = [(x: number, y: number) => (x === -18 && y === 0 ? { color: "#aa0000", scale: 2 } : null)];
    const { unmount } = renderMap({ paintLayers: beyond });
    expect(rects.filter((r) => r.color === "#aa0000")).toHaveLength(0);
    unmount();
    renderMap({ paintLayers: beyond, overscan: 1 });
    expect(rects.filter((r) => r.color === "#aa0000")).toHaveLength(1);
  });
});
