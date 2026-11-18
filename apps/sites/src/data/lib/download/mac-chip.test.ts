import { describe, expect, it } from "vitest";

import { macChipFromRenderer } from "./mac-chip";

describe("macChipFromRenderer", () => {
  it.each([
    ["ANGLE (Apple, ANGLE Metal Renderer: Apple M2 Pro, Unspecified Version)", "apple_silicon"],
    ["Apple M1", "apple_silicon"],
    ["ANGLE (Intel Inc., Intel(R) Iris(TM) Plus Graphics 655, OpenGL 4.1)", "intel"],
    ["ANGLE (Apple, ANGLE Metal Renderer: AMD Radeon Pro 5500M, Unspecified Version)", "intel"],
    ["NVIDIA GeForce GT 750M OpenGL Engine", "intel"],
  ])("reads %s as %s", (renderer, chip) => {
    expect(macChipFromRenderer(renderer)).toBe(chip);
  });

  it("does not guess from a renderer that names no chip", () => {
    expect(macChipFromRenderer("Apple GPU")).toBe("unknown");
    expect(macChipFromRenderer("")).toBe("unknown");
  });
});
