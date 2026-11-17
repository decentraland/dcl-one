import { describe, expect, it } from "vitest";

import { CatalystError } from "@/data/lib/catalyst/client";

import { watchRefusal } from "./LiveCastWatch";

const err = (status: number, message: string | null) =>
  new CatalystError(message ?? `Catalyst returned ${status}`, "u", status, message !== null);

describe("watchRefusal", () => {
  it("shows no stream as waiting, an expired link as ended, a ban as a refusal", () => {
    expect(watchRefusal(err(404, "No active stream found for this location"))).toMatchObject({ kind: "waiting" });
    expect(watchRefusal(err(401, "Stream access has expired"))).toMatchObject({
      kind: "waiting",
      title: "This Cast has ended",
    });
    expect(watchRefusal(err(403, "You are banned from this scene"))).toEqual({
      kind: "error",
      title: "You can't watch this Cast",
      message: "You are banned from this scene",
    });
  });

  it("keeps any other server message and falls back on a network failure", () => {
    expect(watchRefusal(err(401, "Access denied, deny-listed wallet"))).toMatchObject({
      kind: "error",
      message: "Access denied, deny-listed wallet",
    });
    expect(watchRefusal(new Error("offline"))).toMatchObject({ kind: "error", message: "Try again in a moment." });
  });
});
