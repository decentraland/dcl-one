import { describe, expect, it, vi } from "vitest";

import type { AuthIdentity } from "@/data/lib/auth/types";
import { CatalystError } from "@/data/lib/catalyst/client";

import { NO_ANSWER } from "./answer";
import { answeredIn, joinAsWatcher, type WatchDeps } from "./LiveCastWatch";
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

  it("throws a token request that failed, which is no refusal", async () => {
    const deps: WatchDeps = { getToken: vi.fn(async () => Promise.reject(new Error("offline"))), join: vi.fn() };
    await expect(joinAsWatcher(identity, { location: "0,0" }, new AbortController().signal, deps)).rejects.toThrow("offline");
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

  it("stops a join nobody answers at its bound with the reason, and drops the room if it connects later", async () => {
    const session = fakeSession();
    let connect: (s: CastSession) => void = () => {};
    const deps: WatchDeps = {
      getToken: vi.fn(async () => CREDS),
      join: vi.fn(() => new Promise<CastSession>((resolve) => (connect = resolve))),
    };
    const joined = answeredIn(5, new AbortController().signal, (signal) =>
      joinAsWatcher(identity, { location: "10,10" }, signal, deps),
    );
    await expect(joined).rejects.toThrow(NO_ANSWER);
    connect(session);
    await vi.waitFor(() => expect(session.disconnect).toHaveBeenCalledTimes(1));
  });

  it("says nothing of a join the page left before its bound", async () => {
    const controller = new AbortController();
    const settled = vi.fn();
    answeredIn(5, controller.signal, () => new Promise(() => {})).then(settled, settled);
    controller.abort();
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(settled).not.toHaveBeenCalled();
  });
});
