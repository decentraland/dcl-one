import { beforeEach, describe, expect, it } from "vitest";
import { z } from "zod";

import { CatalystError } from "@/data/lib/catalyst/client";

import { isRouteFatal, profileErrorReason } from "./errors";

describe("isRouteFatal", () => {
  describe("when the error is a node error", () => {
    let error: CatalystError;

    beforeEach(() => {
      error = new CatalystError("Catalyst returned 502", "/lambdas/profile", 502);
    });

    it("should stop the route", () => {
      expect(isRouteFatal(error)).toBe(true);
    });
  });

  describe("when the error is a network failure", () => {
    let error: CatalystError;

    beforeEach(() => {
      error = new CatalystError("Catalyst unreachable", "/lambdas/profile", 0);
    });

    it("should stop the route", () => {
      expect(isRouteFatal(error)).toBe(true);
    });
  });

  describe("when the error is a reply that does not parse", () => {
    let error: z.ZodError | undefined;

    beforeEach(() => {
      error = z.object({ a: z.string() }).safeParse({}).error;
    });

    it("should stop the route", () => {
      expect(isRouteFatal(error)).toBe(true);
    });
  });

  describe("when the error is a programming error", () => {
    describe("and it is a TypeError", () => {
      let error: TypeError;

      beforeEach(() => {
        error = new TypeError("Cannot read properties of undefined (reading 'name')");
      });

      it("should not count it as a profile failure", () => {
        expect(isRouteFatal(error)).toBe(false);
      });
    });

    describe("and it is a plain Error", () => {
      let error: Error;

      beforeEach(() => {
        error = new Error("boom");
      });

      it("should not count it as a profile failure", () => {
        expect(isRouteFatal(error)).toBe(false);
      });
    });
  });
});

describe("profileErrorReason", () => {
  describe("when the node answers not found", () => {
    let error: CatalystError;

    beforeEach(() => {
      error = new CatalystError("not found", "/x", 404);
    });

    it("should read it as unavailable, since not found means only an address that cannot be a wallet", () => {
      expect(profileErrorReason(error)).toBe("unavailable");
    });
  });

  describe("when the node answers a bad gateway", () => {
    let error: CatalystError;

    beforeEach(() => {
      error = new CatalystError("bad gateway", "/x", 502);
    });

    it("should read it as unavailable", () => {
      expect(profileErrorReason(error)).toBe("unavailable");
    });
  });
});
