import { describe, expect, it } from "vitest";

import { PAGE_SLOTS, pageWindow } from "./pageWindow";

describe("the pages a pager draws", () => {
  it("draws every page when they fit", () => {
    expect(pageWindow(2, 5)).toEqual([0, 1, 2, 3, 4]);
  });

  it("keeps the same number of slots on every page, so the arrows never move", () => {
    const widths = Array.from({ length: 40 }, (_, page) => pageWindow(page, 40).length);
    expect(new Set(widths)).toEqual(new Set([PAGE_SLOTS]));
  });

  it("slides the window at either end rather than shrinking it", () => {
    expect(pageWindow(0, 40)).toEqual([0, 1, 2, 3, 4, "gap", 39]);
    expect(pageWindow(39, 40)).toEqual([0, "gap", 35, 36, 37, 38, 39]);
  });

  it("centres the current page with its neighbours in the middle", () => {
    expect(pageWindow(20, 40)).toEqual([0, "gap", 19, 20, 21, "gap", 39]);
  });
});
