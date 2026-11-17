import { describe, expect, it } from "vitest";
import { homeEventHref } from "./links";

describe("home event links", () => {
  it("selects the existing event detail route instead of the legacy list redirect", () => {
    expect(homeEventHref("0795dec0-4862-4f19-a4d5-d0f3c3c3b559")).toBe(
      "/events?id=0795dec0-4862-4f19-a4d5-d0f3c3c3b559",
    );
  });
  it("encodes event identifiers rather than introducing query or path separators", () => {
    expect(homeEventHref("event/id?foo=bar")).toBe(
      "/events?id=event%2Fid%3Ffoo%3Dbar",
    );
  });
});
