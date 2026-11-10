import { describe, expect, it } from "vitest";

import { safeReturn, safeReturnPath } from "./safe-return";

describe("safeReturn", () => {
  describe.each([
    "/blog",
    "/blog/announcements/a-post",
    "/shop/cart?tab=1&x=%20y",
    "/blog/search?q=wearables#results",
    "/",
  ])("when the target is the local path %j", (target) => {
    it("should keep it as it is", () => {
      expect(safeReturn(target)).toBe(target);
    });
  });

  describe.each([
    ["missing", null],
    ["empty", ""],
    ["an absolute URL", "https://evil.example/x"],
    ["protocol-relative", "//evil.example/x"],
    ["a backslash host", "/\\evil.example"],
    ["a backslash later in the path", "/x\\..\\evil"],
    ["encoded slashes", "/%2F%2Fevil.example"],
    ["an encoded slash after one", "/%2Fevil.example"],
    ["an encoded backslash", "/%5Cevil.example"],
    ["leading whitespace", " /blog"],
    ["a leading tab", "\t/blog"],
    ["an embedded newline", "/blog\n//evil.example"],
    ["an encoded control character", "/blog%0a"],
    ["a javascript: URL", "javascript:alert(1)"],
    ["a data: URL", "data:text/html,x"],
    ["a malformed escape", "/blog%E0%A4%A"],
    ["a dot segment that normalizes to a host", "/.//evil.example"],
    ["a dot-dot segment that normalizes to a host", "/x/..//evil.example"],
  ])("when the target is %s", (_name, target) => {
    it("should refuse it", () => {
      expect(safeReturn(target)).toBeNull();
    });
  });
});

describe("safeReturn's normalized answer", () => {
  it.each([
    ["/a/../b", "/b"],
    ["/./b", "/b"],
    ["/%2e%2e/x", "/x"],
  ])("should answer %j as %j", (target, expected) => {
    expect(safeReturn(target)).toBe(expected);
  });
});

describe("safeReturnPath", () => {
  const HERE = new URL("https://decentraland.org/blog/x");

  // The 32 inputs both former validators were compared on (rebase 2), under the merged stricter rule.
  it.each([
    ["/blog", "/blog"],
    ["/blog?x=1#h", "/blog?x=1#h"],
    ["/", "/"],
    ["/a/../b", "/b"],
    ["/./b", "/b"],
    ["/.//evil.example", "FALLBACK"],
    ["/a//b", "/a//b"],
    ["/%2e%2e/x", "/x"],
    ["//evil.example", "FALLBACK"],
    ["/\\evil.example", "FALLBACK"],
    ["/%2F%2Fevil.example", "FALLBACK"],
    ["/%5Cevil.example", "FALLBACK"],
    ["/%2f%2fevil.example", "FALLBACK"],
    ["/a b", "FALLBACK"],
    ["/a\tb", "FALLBACK"],
    ["/\t/evil.example", "FALLBACK"],
    ["/a\nb", "FALLBACK"],
    ["/a\\b", "FALLBACK"],
    ["/%E0%A4%A", "FALLBACK"],
    ["/%00x", "FALLBACK"],
    ["foo", "FALLBACK"],
    ["?x=1", "FALLBACK"],
    ["#h", "FALLBACK"],
    ["", "FALLBACK"],
    [" /blog", "FALLBACK"],
    ["https://decentraland.org/blog", "/blog"],
    ["http://decentraland.org/blog", "FALLBACK"],
    ["https://evil.example/", "FALLBACK"],
    ["http://localhost/x", "FALLBACK"],
    ["javascript:alert(1)", "FALLBACK"],
    ["data:text/html,x", "FALLBACK"],
    ["https://decentraland.org.evil.example/", "FALLBACK"],
  ])("should answer %j with %j on decentraland.org", (target, expected) => {
    expect(safeReturnPath(target, { fallback: "FALLBACK", here: HERE })).toBe(
      expected,
    );
  });

  it("should keep an absolute URL on this origin only when it knows the page's origin", () => {
    expect(
      safeReturnPath("https://decentraland.org/blog", {
        fallback: "/",
        here: HERE,
      }),
    ).toBe("/blog");
    expect(
      safeReturnPath("https://decentraland.org/blog", { fallback: "/" }),
    ).toBe("/");
  });

  it("should pass the path an absolute URL reduces to through the path rules", () => {
    expect(
      safeReturnPath("https://decentraland.org/.//evil.example", {
        fallback: "/",
        here: HERE,
      }),
    ).toBe("/");
    expect(
      safeReturnPath("https://decentraland.org/%2F%2Fevil.example", {
        fallback: "/",
        here: HERE,
      }),
    ).toBe("/");
  });

  it("should accept plain http only on a local host, on the page's exact origin", () => {
    const local = new URL("http://127.0.0.1:5212/auth/login");
    expect(
      safeReturnPath("http://127.0.0.1:5212/shop", {
        fallback: "/",
        here: local,
      }),
    ).toBe("/shop");
    expect(
      safeReturnPath("http://127.0.0.1:9999/shop", {
        fallback: "/",
        here: local,
      }),
    ).toBe("/");
  });

  it("should answer the page's own fallback", () => {
    expect(safeReturnPath("//evil.example", { fallback: "/blog" })).toBe(
      "/blog",
    );
    expect(safeReturnPath(null, { fallback: "/" })).toBe("/");
  });
});
