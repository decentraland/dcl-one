import { describe, expect, it, vi } from "vitest";

import type { AuthIdentity } from "@/data/lib/auth/types";
import { CatalystError } from "@/data/lib/catalyst/client";

import { joinAsWatcher, type WatchDeps } from "./LiveCastWatch";
import type { CastSession } from "./session";

const identity = { signer: "0x00000000000000000000000000000000000abcd1" } as unknown as AuthIdentity;
const CREDS = { url: "wss://lk", token: "jwt", roomId: "scene-room", placeName: "Plaza" };

function fakeSession() {
  return { disconnect: vi.fn(async () => {}) } as unknown as CastSession & {
    disconnect: ReturnType<typeof vi.fn>;
  };
}

describe("joinAsWatcher", () => {
  it("signs the token with the viewer's name and joins its room", async () => {
    const session = fakeSession();
    const deps: WatchDeps = { getToken: vi.fn(async () => CREDS), join: vi.fn(async () => session) };
    const out = await joinAsWatcher(identity, { location: "10,10" }, new AbortController().signal, deps);
    expect(out).toEqual({ credentials: CREDS, session });
    expect(deps.getToken).toHaveBeenCalledWith(
      identity,
      { location: "10,10", displayName: "Guest#bcd1" },
      expect.objectContaining({ signal: expect.any(AbortSignal) }),
    );
  });

  it("turns a gatekeeper refusal into the page's refusal without connecting", async () => {
    const join = vi.fn();
    const deps: WatchDeps = {
      getToken: vi.fn(async () => {
        throw new CatalystError("No active stream found for location: 0,0", "u", 404, true);
      }),
      join,
    };
    const out = await joinAsWatcher(identity, { location: "0,0" }, new AbortController().signal, deps);
    expect(out).toMatchObject({ refused: { kind: "waiting" } });
    expect(join).not.toHaveBeenCalled();
  });

  it("disconnects a room that connected after the page left, and reports nothing", async () => {
    const session = fakeSession();
    const controller = new AbortController();
    const deps: WatchDeps = {
      getToken: vi.fn(async () => CREDS),
      join: vi.fn(async () => {
        controller.abort();
        return session;
      }),
    };
    expect(await joinAsWatcher(identity, { location: "10,10" }, controller.signal, deps)).toBeNull();
    expect(session.disconnect).toHaveBeenCalledTimes(1);
  });

  it("never joins when the page left during the token request", async () => {
    const controller = new AbortController();
    const join = vi.fn();
    const deps: WatchDeps = {
      getToken: vi.fn(async () => {
        controller.abort();
        return CREDS;
      }),
      join,
    };
    expect(await joinAsWatcher(identity, { location: "10,10" }, controller.signal, deps)).toBeNull();
    expect(join).not.toHaveBeenCalled();
  });
});
