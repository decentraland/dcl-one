import { describe, expect, it } from "vitest";

import { CatalystError } from "@/data/lib/catalyst/client";

import { watchRefusal } from "./LiveCastWatch";

const err = (status: number, message: string | null) =>
  new CatalystError(message ?? `Catalyst returned ${status}`, "u", status, message !== null);

describe("watchRefusal", () => {
  it("shows no stream as waiting and an expired link as ended", () => {
    expect(watchRefusal(err(404, "No active stream found for 0,0"))).toMatchObject({ kind: "waiting" });
    expect(watchRefusal(err(404, null))).toMatchObject({ kind: "waiting" });
    expect(watchRefusal(err(401, "Stream access has expired. Please generate a new stream link."))).toMatchObject({
      kind: "waiting",
      title: "This Cast has ended",
    });
  });

  it("shows a ban as a refusal with no new join", () => {
    for (const [status, message] of [
      [403, "You are banned from this scene"],
      [403, "Access denied, platform-banned user"],
      [401, "Access denied, deny-listed wallet."],
    ] as const) {
      expect(watchRefusal(err(status, message))).toEqual({ kind: "error", title: "You cannot join this Cast", message, again: false });
    }
  });

  it("shows a place that is gone and a world that refuses as a refusal a new join can pass", () => {
    for (const [status, message] of [
      [404, "No place found with parcel 0,0"],
      [401, "Access denied, you are not authorized to access this world"],
    ] as const) {
      expect(watchRefusal(err(status, message))).toEqual({ kind: "error", title: "You cannot join this Cast", message, again: true });
    }
  });

  it("calls a request that failed no refusal", () => {
    expect(watchRefusal(err(503, "World authorization unavailable"))).toBeNull();
    expect(watchRefusal(err(0, null))).toBeNull();
    expect(watchRefusal(new Error("offline"))).toBeNull();
  });
});
