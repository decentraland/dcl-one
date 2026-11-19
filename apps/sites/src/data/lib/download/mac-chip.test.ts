import { describe, expect, it } from "vitest";

import { macChipFromRenderer, preferChip } from "./mac-chip";

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

describe("preferChip", () => {
  const appleSilicon = { osKey: "macos", arch: "arm64" };
  const intel = { osKey: "macos", arch: "amd64" };
  const windows = { osKey: "windows", arch: "amd64" };

  it("puts the Intel build first on an Intel Mac and the Apple Silicon one among the others", () => {
    expect(preferChip("intel", appleSilicon, [intel, windows])).toEqual({ primary: intel, others: [appleSilicon, windows] });
  });

  it("keeps the order when the chip is unknown or already first", () => {
    expect(preferChip("unknown", appleSilicon, [intel, windows])).toEqual({ primary: appleSilicon, others: [intel, windows] });
    expect(preferChip("apple_silicon", appleSilicon, [intel, windows])).toEqual({ primary: appleSilicon, others: [intel, windows] });
  });

  it("leaves another system's builds alone", () => {
    expect(preferChip("intel", windows, [appleSilicon, intel])).toEqual({ primary: windows, others: [appleSilicon, intel] });
  });
});
