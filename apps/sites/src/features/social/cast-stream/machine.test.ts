import { describe, expect, it, vi } from "vitest";
import { createActor, waitFor } from "xstate";
import { settle } from "../../test-utils/fake-clock";

import {
  castMachine,
  CAST_EVENTS,
  STATE_TO_SLUG,
  SLUG_TO_STATE,
  FIRST_STEP_SLUG,
  DEFAULT_DEVICES,
  resolveCastSnapshot,
  slugToState,
  stateToSlug,
  simulateResolveToken,
  simulateGrant,
  simulateShareScreen,
  type ResolveTokenFn,
  type RequestPermissionsFn,
  type EndCastFn,
  type GoLiveFn,
  type ShareScreenFn,
  type TokenResult,
  type TrackFn,
} from "./machine";

const RESULT: TokenResult = {
  info: { placeName: "Test Plaza", placeId: "p1", location: "1,2", isWorld: false },
  credentials: {
    url: "wss://livekit.test",
    token: "SIMULATED.test.stub",
    roomId: "scene-1:2-test-plaza",
    identity: "Speaker",
  },
};

const okResolve: ResolveTokenFn = async () => RESULT;
const grant: RequestPermissionsFn = async () => ({ granted: true });
const endOk: EndCastFn = async () => {};

function inputFor(
  resolveToken: ResolveTokenFn,
  requestPermissions: RequestPermissionsFn,
  track: TrackFn,
) {
  return {
    trackCtx: {
      sid: "sid-abc",
      story: "landings-cast-stream",
      variant: "console",
    },
    token: "stream-key-123",
    identity: "Speaker",
    resolveToken,
    requestPermissions,
    endCast: endOk,
    track,
  };
}

function names(track: ReturnType<typeof vi.fn>) {
  return track.mock.calls.map((c) => c[0]);
}

async function toLive(track: TrackFn, shareScreen?: ShareScreenFn) {
  const actor = createActor(castMachine, {
    input: { ...inputFor(okResolve, grant, track), shareScreen },
  }).start();
  await waitFor(actor, (s) => s.matches("deviceSelect"));
  actor.send({ type: "SELECT_DEVICES", devices: DEFAULT_DEVICES });
  await waitFor(actor, (s) => s.matches("preview"));
  actor.send({ type: "JOIN" });
  await waitFor(actor, (s) => s.matches({ live: "idle" }));
  return actor;
}

describe("castMachine \u{2014} URL ?step slug map", () => {
  it("uses the audit-spec kebab-case step names, unique and round-tripping, and falls back to token-check", () => {
    const mapped = new Set(Object.keys(STATE_TO_SLUG));
    expect(mapped).toEqual(new Set(Object.keys(castMachine.states)));
    expect(slugToState("token-check")).toBe("tokenCheck");
    expect(slugToState("device-select")).toBe("deviceSelect");
    for (const [state, slug] of Object.entries(STATE_TO_SLUG)) {
      expect(SLUG_TO_STATE[slug]).toBe(state);
      expect(stateToSlug(state)).toBe(slug);
      expect(slugToState(slug)).toBe(state);
    }
    expect(FIRST_STEP_SLUG).toBe(STATE_TO_SLUG.tokenCheck);
    for (const bad of [null, undefined, "", "nope"]) expect(slugToState(bad)).toBe("tokenCheck");
    expect(stateToSlug("bogus")).toBe(FIRST_STEP_SLUG);
  });
});

describe("castMachine \u{2014} deep-link hydration (snapshot, no event replay)", () => {
  it("boots token-check without a snapshot, hydrates later steps silently, and only real transitions track", async () => {
    const track = vi.fn();
    const resolveToken = vi.fn(okResolve);
    const requestPermissions = vi.fn(grant);
    const trackCtx = inputFor(resolveToken, requestPermissions, track).trackCtx;
    expect(resolveCastSnapshot({ step: "tokenCheck", trackCtx, token: "k" })).toBeUndefined();

    const preview = createActor(castMachine, {
      input: inputFor(resolveToken, requestPermissions, track),
      snapshot: resolveCastSnapshot({
        step: "preview",
        trackCtx,
        token: "k",
        resolveToken,
        requestPermissions,
        track,
      }),
    }).start();
    expect(preview.getSnapshot().matches("preview")).toBe(true);
    expect(preview.getSnapshot().context.info?.placeName).toBeTruthy();
    await Promise.resolve();
    expect(track).not.toHaveBeenCalled();
    expect(resolveToken).not.toHaveBeenCalled();
    expect(requestPermissions).not.toHaveBeenCalled();
    expect(preview.getSnapshot().matches("preview")).toBe(true);

    const live = createActor(castMachine, {
      input: inputFor(okResolve, grant, track),
      snapshot: resolveCastSnapshot({ step: "live", trackCtx, token: "k", track }),
    }).start();
    expect(live.getSnapshot().matches("live")).toBe(true);
    expect(track).not.toHaveBeenCalled();
    live.send({ type: "LEAVE" });
    expect(names(track)).toContain(CAST_EVENTS.ending);
  });
});

describe("castMachine \u{2014} happy path (token -> devices -> permissions -> preview -> live -> ended)", () => {
  it("fires the full funnel through live, then LEAVE fires teardown telemetry through ended", async () => {
    const track = vi.fn();
    const actor = createActor(castMachine, {
      input: inputFor(okResolve, grant, track),
    }).start();

    await waitFor(actor, (s) => s.matches("deviceSelect"));
    actor.send({ type: "SELECT_DEVICES", devices: DEFAULT_DEVICES });
    await waitFor(actor, (s) => s.matches("preview"));
    actor.send({ type: "JOIN" });
    expect(actor.getSnapshot().matches("joining")).toBe(true);
    await waitFor(actor, (s) => s.matches("live"));

    let events = names(track);
    for (const e of [
      CAST_EVENTS.tokenChecked,
      CAST_EVENTS.tokenValid,
      CAST_EVENTS.devicesSelected,
      CAST_EVENTS.permissionsGranted,
      CAST_EVENTS.previewReady,
      CAST_EVENTS.joinRequested,
      CAST_EVENTS.wentLive,
    ]) {
      expect(events).toContain(e);
    }
    expect(events.indexOf(CAST_EVENTS.tokenChecked)).toBeLessThan(
      events.indexOf(CAST_EVENTS.wentLive),
    );
    const liveCall = track.mock.calls.find((c) => c[0] === CAST_EVENTS.wentLive);
    expect(liveCall?.[1]).toMatchObject({ stub: true });
    expect(liveCall?.[2]).toMatchObject({
      sid: "sid-abc",
      variant: "console",
    });

    actor.send({ type: "LEAVE" });
    await waitFor(actor, (s) => s.matches("ended"));
    events = names(track);
    expect(events).toContain(CAST_EVENTS.ending);
    expect(events).toContain(CAST_EVENTS.ended);
    const endedCall = track.mock.calls.find((c) => c[0] === CAST_EVENTS.ended);
    expect(endedCall?.[1]).toMatchObject({ stub: true });
  });
});

describe("castMachine \u{2014} invalid token path", () => {
  it("a bad/expired token routes to invalid with the reason, and RETRY re-checks it", async () => {
    const track = vi.fn();
    let calls = 0;
    const resolveToken: ResolveTokenFn = async (args) => {
      calls += 1;
      if (calls === 1) throw new Error("Streaming token has expired");
      return okResolve(args);
    };
    const actor = createActor(castMachine, {
      input: inputFor(resolveToken, grant, track),
    }).start();

    await waitFor(actor, (s) => s.matches("invalid"));
    expect(actor.getSnapshot().context.invalidReason).toBe("Streaming token has expired");
    const events = names(track);
    expect(events).toContain(CAST_EVENTS.tokenChecked);
    expect(events).toContain(CAST_EVENTS.invalidToken);
    expect(events).not.toContain(CAST_EVENTS.tokenValid);
    const invalidCall = track.mock.calls.find((c) => c[0] === CAST_EVENTS.invalidToken);
    expect(invalidCall?.[1]).toMatchObject({ reason: "Streaming token has expired" });

    actor.send({ type: "RETRY" });
    await waitFor(actor, (s) => s.matches("deviceSelect"));
    expect(actor.getSnapshot().matches("deviceSelect")).toBe(true);
  });
});

describe("castMachine \u{2014} permission denial path (guardrail)", () => {
  it("a denial stays on permissions, flags denied, and is recoverable", async () => {
    const track = vi.fn();
    let calls = 0;
    const requestPermissions: RequestPermissionsFn = async (args) => {
      calls += 1;
      if (calls === 1) return { granted: false };
      return grant(args);
    };
    const actor = createActor(castMachine, {
      input: inputFor(okResolve, requestPermissions, track),
    }).start();

    await waitFor(actor, (s) => s.matches("deviceSelect"));
    actor.send({ type: "SELECT_DEVICES", devices: DEFAULT_DEVICES });
    await waitFor(actor, (s) => s.matches("permissions") && s.context.permissionsDenied);
    expect(names(track)).toContain(CAST_EVENTS.permissionsDenied);
    expect(names(track)).not.toContain(CAST_EVENTS.permissionsGranted);

    actor.send({ type: "RETRY_PERMISSIONS" });
    await waitFor(actor, (s) => s.matches("preview"));
    expect(actor.getSnapshot().context.permissionsDenied).toBe(false);
    expect(names(track)).toContain(CAST_EVENTS.permissionsGranted);
  });
});

describe("castMachine \u{2014} screen-share in live (simulated LiveKit publish)", () => {
  it("TOGGLE_SCREENSHARE publishes then unpublishes; a failed or thrown publish stays live idle and is recoverable", async () => {
    const okTrack = vi.fn();
    const sharing = await toLive(okTrack, async () => ({ published: true }));
    sharing.send({ type: "TOGGLE_SCREENSHARE" });
    await waitFor(sharing, (s) => s.matches({ live: "sharing" }));
    expect(sharing.getSnapshot().context.screenSharing).toBe(true);
    expect(names(okTrack)).toContain(CAST_EVENTS.screenshareStarted);
    const startedCall = okTrack.mock.calls.find((c) => c[0] === CAST_EVENTS.screenshareStarted);
    expect(startedCall?.[1]).toMatchObject({ stub: true });
    sharing.send({ type: "TOGGLE_SCREENSHARE" });
    expect(sharing.getSnapshot().matches({ live: "idle" })).toBe(true);
    expect(sharing.getSnapshot().context.screenSharing).toBe(false);
    expect(sharing.getSnapshot().context.screenShareFailed).toBe(false);

    const failTrack = vi.fn();
    const failed = await toLive(failTrack, async () => ({ published: false }));
    failed.send({ type: "TOGGLE_SCREENSHARE" });
    await waitFor(failed, (s) => s.context.screenShareFailed === true);
    expect(failed.getSnapshot().matches({ live: "idle" })).toBe(true);
    expect(failed.getSnapshot().context.screenSharing).toBe(false);
    expect(names(failTrack)).toContain(CAST_EVENTS.screenshareFailed);
    expect(names(failTrack)).not.toContain(CAST_EVENTS.screenshareStarted);
    const failedCall = failTrack.mock.calls.find((c) => c[0] === CAST_EVENTS.screenshareFailed);
    expect(failedCall?.[1]).toMatchObject({ stub: true });

    const thrown = await toLive(vi.fn(), async () => {
      throw new Error("getDisplayMedia denied");
    });
    thrown.send({ type: "TOGGLE_SCREENSHARE" });
    await waitFor(thrown, (s) => s.context.screenShareFailed === true);
    expect(thrown.getSnapshot().matches({ live: "idle" })).toBe(true);
    thrown.send({ type: "LEAVE" });
    expect(
      thrown.getSnapshot().matches("ending") || thrown.getSnapshot().matches("ended"),
    ).toBe(true);
  });
});

describe("simulateResolveToken / simulateGrant / simulateShareScreen", () => {
  it("resolve faithful upstream shapes, reject blank or expired tokens, and grant/publish by default", async () => {
    vi.useFakeTimers();
    const r = await settle(simulateResolveToken({ token: "abc", identity: "Eve" }));
    expect(r.info.placeName).toBeTruthy();
    expect(r.credentials?.url).toMatch(/^wss:/);
    expect(r.credentials?.token).toContain("SIMULATED.");
    expect(r.credentials?.identity).toBe("Eve");
    await settle(
      expect(simulateResolveToken({ token: "   ", identity: "" })).rejects.toThrow(
        /Invalid streaming key/,
      ),
    );
    await settle(
      expect(simulateResolveToken({ token: "expired", identity: "" })).rejects.toThrow(/expired/),
    );
    expect(await settle(simulateGrant({ devices: DEFAULT_DEVICES }))).toEqual({ granted: true });
    expect(await settle(simulateShareScreen({}))).toEqual({ published: true });
  });
});

describe("castMachine \u{2014} a real session (goLive, stopShare, SHARE_ENDED)", () => {
  const CREDS = { url: "wss://lk.test", token: "jwt", roomId: "scene-room" };

  async function toPreview(goLive: GoLiveFn, track: TrackFn, extra: object = {}) {
    const actor = createActor(castMachine, {
      input: { ...inputFor(async () => ({ info: RESULT.info }), grant, track), identity: "", goLive, ...extra },
    }).start();
    await waitFor(actor, (s) => s.matches("deviceSelect"));
    actor.send({ type: "SELECT_DEVICES", devices: DEFAULT_DEVICES, name: "  Ada  " });
    await waitFor(actor, (s) => s.matches("preview"));
    return actor;
  }

  it("mints at JOIN with the typed name, keeps the credentials and reports a real (not stub) session", async () => {
    const track = vi.fn();
    const goLive = vi.fn<GoLiveFn>(async () => CREDS);
    const actor = await toPreview(goLive, track);
    expect(actor.getSnapshot().context.credentials).toBeUndefined();
    actor.send({ type: "JOIN" });
    await waitFor(actor, (s) => s.matches({ live: "idle" }));
    expect(goLive).toHaveBeenCalledTimes(1);
    expect(goLive.mock.calls[0][0]).toMatchObject({ token: "stream-key-123", identity: "Ada" });
    expect(actor.getSnapshot().context.credentials).toEqual(CREDS);
    const liveCall = track.mock.calls.find((c) => c[0] === CAST_EVENTS.wentLive);
    expect(liveCall?.[1]).toEqual({ room_id: "scene-room", stub: false });
  });

  it("a refused join returns to preview with the reason, and a retry joins", async () => {
    const track = vi.fn();
    const goLive = vi
      .fn<GoLiveFn>()
      .mockRejectedValueOnce(new Error("Streaming token has expired"))
      .mockResolvedValueOnce(CREDS);
    const actor = await toPreview(goLive, track);
    actor.send({ type: "JOIN" });
    await waitFor(actor, (s) => s.matches("preview") && !!s.context.joinError);
    expect(actor.getSnapshot().context.joinError).toBe("Streaming token has expired");
    expect(names(track)).toContain(CAST_EVENTS.joinFailed);
    actor.send({ type: "JOIN" });
    await waitFor(actor, (s) => s.matches("live"));
    expect(actor.getSnapshot().context.joinError).toBeUndefined();
  });

  it("a lost room ends a live cast as disconnected, and an old join error does not come back", async () => {
    const track = vi.fn();
    const goLive = vi
      .fn<GoLiveFn>()
      .mockRejectedValueOnce(new Error("Streaming token has expired"))
      .mockResolvedValue(CREDS);
    const actor = await toPreview(goLive, track);
    actor.send({ type: "JOIN" });
    await waitFor(actor, (s) => s.matches("preview") && !!s.context.joinError);
    actor.send({ type: "BACK" });
    await waitFor(actor, (s) => s.matches("deviceSelect"));
    expect(actor.getSnapshot().context.joinError).toBeUndefined();
    actor.send({ type: "SELECT_DEVICES", devices: DEFAULT_DEVICES });
    await waitFor(actor, (s) => s.matches("preview"));
    expect(actor.getSnapshot().context.joinError).toBeUndefined();

    actor.send({ type: "JOIN" });
    await waitFor(actor, (s) => s.matches({ live: "idle" }));
    actor.send({ type: "CONNECTION_LOST" });
    expect(actor.getSnapshot().matches("ended")).toBe(true);
    expect(actor.getSnapshot().context.connectionLost).toBe(true);
    const lost = track.mock.calls.find((c) => c[0] === CAST_EVENTS.connectionLost);
    expect(lost?.[1]).toEqual({ room_id: "scene-room" });
  });

  it("stopping a share calls stopShare; the browser ending it does not, and both leave sharing", async () => {
    const stopShare = vi.fn();
    const shareScreen: ShareScreenFn = async () => ({ published: true });
    const actor = await toPreview(async () => CREDS, vi.fn(), { stopShare, shareScreen });
    actor.send({ type: "JOIN" });
    await waitFor(actor, (s) => s.matches({ live: "idle" }));

    actor.send({ type: "TOGGLE_SCREENSHARE" });
    await waitFor(actor, (s) => s.matches({ live: "sharing" }));
    actor.send({ type: "TOGGLE_SCREENSHARE" });
    expect(actor.getSnapshot().matches({ live: "idle" })).toBe(true);
    expect(stopShare).toHaveBeenCalledTimes(1);

    actor.send({ type: "TOGGLE_SCREENSHARE" });
    await waitFor(actor, (s) => s.matches({ live: "sharing" }));
    actor.send({ type: "SHARE_ENDED" });
    expect(actor.getSnapshot().matches({ live: "idle" })).toBe(true);
    expect(actor.getSnapshot().context.screenSharing).toBe(false);
    expect(stopShare).toHaveBeenCalledTimes(1);
  });
});

describe("castMachine \u{2014} a stopped invocation", () => {
  it("never reports its abort as an invalid key", async () => {
    const resolveToken: ResolveTokenFn = ({ signal }) =>
      new Promise((_, reject) => signal?.addEventListener("abort", () => reject(new Error("signal is aborted without reason"))));
    const track = vi.fn();
    const actor = createActor(castMachine, { input: inputFor(resolveToken, grant, track) }).start();
    actor.stop();
    await new Promise((r) => setTimeout(r, 0));
    expect(actor.getSnapshot().matches("invalid")).toBe(false);
    expect(names(track)).not.toContain(CAST_EVENTS.invalidToken);
  });
});

describe("castMachine \u{2014} one join from device select", () => {
  it("SELECT_DEVICES with join goes live once access is granted, without stopping at preview", async () => {
    const track = vi.fn();
    const goLive = vi.fn<GoLiveFn>(async ({ identity }) => ({ ...RESULT.credentials!, identity }));
    const actor = createActor(castMachine, {
      input: { ...inputFor(okResolve, grant, track), goLive },
    }).start();
    await waitFor(actor, (s) => s.matches("deviceSelect"));
    actor.send({ type: "SELECT_DEVICES", devices: DEFAULT_DEVICES, name: " Ada ", join: true });
    await waitFor(actor, (s) => s.matches({ live: "idle" }));

    expect(goLive).toHaveBeenCalledTimes(1);
    expect(goLive.mock.calls[0]![0]).toMatchObject({ identity: "Ada", devices: DEFAULT_DEVICES });
    expect(names(track)).toContain(CAST_EVENTS.permissionsGranted);
    expect(names(track)).toContain(CAST_EVENTS.joinRequested);
    expect(names(track)).not.toContain(CAST_EVENTS.previewReady);
    actor.stop();
  });

  it("a denied join waits for RETRY_PERMISSIONS and then joins by itself", async () => {
    let granted = false;
    const goLive = vi.fn<GoLiveFn>(async () => RESULT.credentials!);
    const actor = createActor(castMachine, {
      input: { ...inputFor(okResolve, async () => ({ granted }), vi.fn()), goLive },
    }).start();
    await waitFor(actor, (s) => s.matches("deviceSelect"));
    actor.send({ type: "SELECT_DEVICES", devices: DEFAULT_DEVICES, join: true });
    await waitFor(actor, (s) => s.matches("permissions") && s.context.permissionsDenied);
    expect(goLive).not.toHaveBeenCalled();

    granted = true;
    actor.send({ type: "RETRY_PERMISSIONS" });
    await waitFor(actor, (s) => s.matches({ live: "idle" }));
    expect(goLive).toHaveBeenCalledTimes(1);
    actor.stop();
  });

  it("a failed join lands on preview, whose JOIN takes the name typed there", async () => {
    const goLive = vi
      .fn<GoLiveFn>()
      .mockRejectedValueOnce(new Error("room is full"))
      .mockImplementation(async ({ identity }) => ({ ...RESULT.credentials!, identity }));
    const actor = createActor(castMachine, {
      input: { ...inputFor(okResolve, grant, vi.fn()), goLive },
    }).start();
    await waitFor(actor, (s) => s.matches("deviceSelect"));
    actor.send({ type: "SELECT_DEVICES", devices: DEFAULT_DEVICES, name: "Ada", join: true });
    await waitFor(actor, (s) => s.matches("preview"));
    expect(actor.getSnapshot().context.joinError).toBe("room is full");

    actor.send({ type: "JOIN", name: "Grace" });
    await waitFor(actor, (s) => s.matches({ live: "idle" }));
    expect(goLive.mock.calls[1]![0]).toMatchObject({ identity: "Grace" });
    actor.stop();
  });
});
