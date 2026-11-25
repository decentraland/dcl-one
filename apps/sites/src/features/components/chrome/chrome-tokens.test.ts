// @vitest-environment node
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const read = (file: string) => readFileSync(new URL(file, import.meta.url), "utf8");
// Hex, rgb()/rgba() and hsl()/hsla() colour literals; the shared foundations' tokens carry the palette.
const COLOUR = /#[0-9a-f]{3,8}\b|\b(?:rgb|hsl)a?\(/gi;

describe.each(["SitesLayout.module.css", "routepending.css"])("when the Sites chrome stylesheet %s is read", (file) => {
  it("should take its colours from the shared foundations' tokens", () => {
    expect(read(file).match(COLOUR) ?? []).toEqual([]);
  });
});
