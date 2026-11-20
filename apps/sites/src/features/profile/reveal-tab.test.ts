// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";

import { revealTab } from "@dcl/one-ui/apps/sites/profile/reveal-tab";

type Box = { left: number; right: number };

/** A tab bar 300px wide at the page's left edge, scrolled by `scrollLeft`, with one tab at `tab` in the bar's content. */
function barWith(tab: Box, scrollLeft: number) {
  const bar = document.createElement("div");
  bar.setAttribute("role", "tablist");
  const link = document.createElement("a");
  bar.append(link);
  document.body.append(bar);
  bar.scrollLeft = scrollLeft;
  bar.getBoundingClientRect = () => ({ left: 0, right: 300 }) as DOMRect;
  link.getBoundingClientRect = () => ({ left: tab.left - bar.scrollLeft, right: tab.right - bar.scrollLeft }) as DOMRect;
  return { bar, link };
}

afterEach(() => document.body.replaceChildren());

describe("revealTab", () => {
  it("scrolls the bar until a tab past its end edge is inside it, with the gutter beside it", () => {
    const { bar, link } = barWith({ left: 420, right: 560 }, 0);

    revealTab(link);

    expect(bar.scrollLeft).toBe(560 - 300 + 16);
  });

  it("scrolls the bar back for a tab before its start edge", () => {
    const { bar, link } = barWith({ left: 100, right: 180 }, 200);

    revealTab(link);

    expect(bar.scrollLeft).toBe(100 - 16);
  });

  it("leaves the bar alone for a tab already inside it, and does nothing without a tab or a bar", () => {
    const { bar, link } = barWith({ left: 40, right: 120 }, 0);

    revealTab(link);
    revealTab(null);
    revealTab(document.createElement("a"));

    expect(bar.scrollLeft).toBe(0);
  });
});
