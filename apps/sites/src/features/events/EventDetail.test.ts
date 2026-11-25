import type { MouseEvent } from "react";
import { describe, expect, it } from "vitest";

import { isPlainClick } from "./EventDetail";

type Click = Pick<
  MouseEvent<HTMLAnchorElement>,
  "defaultPrevented" | "button" | "metaKey" | "ctrlKey" | "shiftKey" | "altKey"
>;

const click = (over: Partial<Click> = {}) =>
  ({
    defaultPrevented: false,
    button: 0,
    metaKey: false,
    ctrlKey: false,
    shiftKey: false,
    altKey: false,
    ...over,
  }) as MouseEvent<HTMLAnchorElement>;

describe("isPlainClick", () => {
  describe("when the click is a plain left click", () => {
    it("should be plain", () => {
      expect(isPlainClick(click())).toBe(true);
    });
  });

  describe.each([
    ["the middle button", { button: 1 }],
    ["the right button", { button: 2 }],
    ["Cmd", { metaKey: true }],
    ["Ctrl", { ctrlKey: true }],
    ["Shift", { shiftKey: true }],
    ["Alt", { altKey: true }],
  ])("when the click uses %s", (_name, over) => {
    it("should leave it to the browser", () => {
      expect(isPlainClick(click(over))).toBe(false);
    });
  });

  describe("when an earlier handler already prevented the default", () => {
    it("should leave it alone", () => {
      expect(isPlainClick(click({ defaultPrevented: true }))).toBe(false);
    });
  });
});
