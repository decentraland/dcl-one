import { describe, expect, it } from "vitest";

import { loader, meta } from "./communities.$";

describe("/communities/*", () => {
  describe("when a visitor opens any path under /communities", () => {
    it("should answer 404, as decentraland.org does", () => {
      let thrown: unknown;
      try {
        loader();
      } catch (error) {
        thrown = error;
      }
      expect((thrown as { init?: { status?: number } }).init?.status).toBe(404);
    });

    it("should title the page as not found", () => {
      expect(meta()).toEqual([{ title: "Page not found | Decentraland" }]);
    });
  });
});
