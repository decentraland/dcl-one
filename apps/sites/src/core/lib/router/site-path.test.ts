import { describe, expect, it } from "vitest";

import { sitePath } from "./site-path";

const ORIGIN = "https://sites.test";

describe("when a return destination is checked against the site", () => {
  it("should keep a root-relative path with its query and fragment", () => {
    expect(sitePath("/events?tab=my#all", ORIGIN)).toBe("/events?tab=my#all");
  });

  it("should accept an absolute URL on the same origin as its path", () => {
    expect(sitePath("https://sites.test/shop/cart", ORIGIN)).toBe("/shop/cart");
  });

  it.each([
    ["nothing", null],
    ["an empty value", ""],
    ["another origin", "https://evil.example/shop"],
    ["a protocol-relative URL", "//evil.example/shop"],
    ["a backslash host", "/\\evil.example/shop"],
    ["a tab before the host", "/\t/evil.example/shop"],
    ["a newline before the host", "/\n/evil.example/shop"],
    ["a carriage return before the host", "/\r/evil.example/shop"],
    ["a script URL", "javascript:alert(1)"],
    ["plain HTTP on this host", "http://sites.test/shop"],
    ["a malformed URL", "http://["],
  ])("should reject %s", (_, raw) => {
    expect(sitePath(raw, ORIGIN)).toBeNull();
  });

  it("should accept plain HTTP on loopback", () => {
    expect(sitePath("http://localhost:5173/events", "http://localhost:5173")).toBe("/events");
  });
});
