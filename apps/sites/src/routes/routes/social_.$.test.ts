import { beforeEach, describe, expect, it } from "vitest";

import { loader } from "./social_.$";

describe("/social/*", () => {
  describe("when the path is not a community page", () => {
    let thrown: unknown;

    beforeEach(() => {
      thrown = undefined;
      try {
        loader();
      } catch (error) {
        thrown = error;
      }
    });

    it("should answer 404", () => {
      expect((thrown as { init?: { status?: number } }).init?.status).toBe(404);
    });
  });
});
