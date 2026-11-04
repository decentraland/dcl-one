import { describe, expect, it } from "vitest";

import { loginPath, safeRedirectTarget } from "./redirect";

const HERE = new URL("https://decentraland.org/auth/login?redirectTo=%2Fshop");
const LOCAL = new URL("http://127.0.0.1:5212/auth/login");

describe("where sign-in returns the reader", () => {
  it("keeps a path, or an absolute URL on this origin, as a path with its query and fragment", () => {
    expect(safeRedirectTarget("/shop/items?sortBy=newest#top", HERE)).toBe("/shop/items?sortBy=newest#top");
    expect(safeRedirectTarget("https://decentraland.org/marketplace", HERE)).toBe("/marketplace");
    // A nested, already-encoded value survives: the parameter is decoded once, by the URL parser.
    expect(safeRedirectTarget("/play?realm=foo%2Fbar", HERE)).toBe("/play?realm=foo%2Fbar");
  });

  it("sends anything off this site, or over a weaker scheme, home", () => {
    for (const bad of [
      "https://evil.example/steal",
      "//evil.example/steal",
      "/\\evil.example/steal",
      "https://decentraland.org.evil.example/",
      "https://decentraland.org@evil.example/",
      "http://decentraland.org/shop",
      "https://decentraland.org:8443/shop",
      "javascript:alert(1)",
      "data:text/html,hi",
    ]) {
      expect(safeRedirectTarget(bad, HERE), bad).toBe("/");
    }
    expect(safeRedirectTarget(null, HERE)).toBe("/");
    expect(safeRedirectTarget("", HERE)).toBe("/");
  });

  it("never answers a path another parser would read as a host", () => {
    for (const bad of ["https://decentraland.org//evil.example/x", "https://decentraland.org/\\evil.example", "///evil.example"]) {
      expect(safeRedirectTarget(bad, HERE), bad).toBe("/");
    }
  });

  it("allows plain http only on a local host, on the page's exact port", () => {
    expect(safeRedirectTarget("http://127.0.0.1:5212/shop", LOCAL)).toBe("/shop");
    expect(safeRedirectTarget("http://127.0.0.1:9999/shop", LOCAL)).toBe("/");
    expect(safeRedirectTarget("http://127.0.0.1/shop", LOCAL)).toBe("/");
    expect(safeRedirectTarget("/shop", LOCAL)).toBe("/shop");
  });

  it("builds the sign-in link with the return path encoded once", () => {
    expect(loginPath("/shop/items?a=1&b=2")).toBe("/auth/login?redirectTo=%2Fshop%2Fitems%3Fa%3D1%26b%3D2");
  });
});
