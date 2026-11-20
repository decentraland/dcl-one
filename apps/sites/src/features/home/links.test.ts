import { describe, expect, it } from "vitest";
import { homeEventHref } from "./links";

describe("homeEventHref", () => {
  describe("when the event identifier is a plain id", () => {
    it("should select the existing event detail route instead of the legacy list redirect", () => {
      expect(homeEventHref("0795dec0-4862-4f19-a4d5-d0f3c3c3b559")).toBe(
        "/events?id=0795dec0-4862-4f19-a4d5-d0f3c3c3b559",
      );
    });
  });

  describe("when the event identifier contains query or path separators", () => {
    it("should encode them in the id parameter", () => {
      expect(homeEventHref("event/id?foo=bar")).toBe(
        "/events?id=event%2Fid%3Ffoo%3Dbar",
      );
    });
  });
});
