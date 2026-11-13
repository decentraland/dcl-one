import { describe, expect, it } from "vitest";
import { z } from "zod";

import { CatalystError } from "@/data/lib/catalyst/client";

import { isRouteFatal, profileErrorReason } from "./errors";

describe("isRouteFatal", () => {
  it("stops the route for a node error, a network failure or a reply that does not parse", () => {
    expect(isRouteFatal(new CatalystError("Catalyst returned 502", "/lambdas/profile", 502))).toBe(true);
    expect(isRouteFatal(new CatalystError("Catalyst unreachable", "/lambdas/profile", 0))).toBe(true);
    expect(isRouteFatal(z.object({ a: z.string() }).safeParse({}).error)).toBe(true);
  });

  it("does not count a programming error as a profile failure", () => {
    expect(isRouteFatal(new TypeError("Cannot read properties of undefined (reading 'name')"))).toBe(false);
    expect(isRouteFatal(new Error("boom"))).toBe(false);
  });
});

describe("profileErrorReason", () => {
  it("reads every node error as unavailable: not found means only an address that cannot be a wallet", () => {
    expect(profileErrorReason(new CatalystError("not found", "/x", 404))).toBe("unavailable");
    expect(profileErrorReason(new CatalystError("bad gateway", "/x", 502))).toBe("unavailable");
  });
});
