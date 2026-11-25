import { expect, test } from "vitest";
import { color3ToHex, hexToColor3 } from "./backpack";

test("swatches are stored as the profile's sRGB 0..1 channels", () => {
  const black = hexToColor3("#1c1c1c");
  expect(black.r).toBeCloseTo(28 / 255, 5);
  expect(black.g).toBe(black.r);
  expect(black.b).toBe(black.r);
  expect(color3ToHex(black)).toBe("#1c1c1c");
});

test.each(["#000000", "#ffffff", "#c98c63", "#5c3824", "#3a6ea5", "#01090a"])("palette %s survives save and reload", (hex) => {
  expect(color3ToHex(hexToColor3(hex))).toBe(hex);
});

test("saved profile colors display as the same sRGB hex", () => {
  expect(color3ToHex({ r: 0.5, g: 0.5, b: 0.5 })).toBe("#808080");
});
