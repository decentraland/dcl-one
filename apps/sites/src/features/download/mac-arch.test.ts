import { describe, expect, it } from "vitest";

import { macArchFromRenderer } from "./mac-arch";

describe("macArchFromRenderer", () => {
  it.each([
    [
      "ANGLE (Apple, ANGLE Metal Renderer: Apple M4 Pro, Unspecified Version)",
      "apple_silicon",
    ],
    [
      "ANGLE (Intel, Intel(R) Iris(TM) Plus Graphics OpenGL Engine, OpenGL 4.1)",
      "intel",
    ],
    ["AMD Radeon Pro 5500M OpenGL Engine", "intel"],
    ["NVIDIA GeForce GT 750M OpenGL Engine", "intel"],
    ["SwiftShader", "unknown"],
    [null, "unknown"],
  ] as const)("reads %s as %s", (renderer, arch) => {
    expect(macArchFromRenderer(renderer)).toBe(arch);
  });
});
