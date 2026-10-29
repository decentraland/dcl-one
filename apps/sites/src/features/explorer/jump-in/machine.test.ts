import { describe, expect, it, vi } from "vitest";
import { createActor, waitFor } from "xstate";

import {
  jumpInMachine,
  JUMP_IN_EVENTS,
  type JumpInPlace,
  type LaunchFn,
  type LaunchTarget,
  type TrackFn,
} from "./machine";

const PLACE: JumpInPlace = {
  id: "place-1",
  title: "Genesis Plaza",
  base_position: "-3,-2",
  world: false,
  world_name: null,
};

const TARGET: LaunchTarget = {
  launchUrl: "https://example.org/play/?position=-3%2C-2&realm=dcl-one",
  realm: "dcl-one",
};

const okLaunch: LaunchFn = async () => TARGET;

function inputFor(launch: LaunchFn, track: TrackFn) {
  return {
    place: PLACE,
    trackCtx: {
      sid: "sid-xyz",
      story: "places/detail",
      variant: "base",
    },
    launch,
    track,
  };
}

function names(track: ReturnType<typeof vi.fn>) {
  return track.mock.calls.map((c) => c[0]);
}

describe("jumpInMachine", () => {
  it("starts immediately, tracks completion, and retries launch failures", async () => {
    const track = vi.fn();
    const actor = createActor(jumpInMachine, { input: inputFor(okLaunch, track) }).start();
    actor.send({ type: "START" });
    await waitFor(actor, (s) => s.matches("launched"));

    const events = names(track);
    expect(events).toContain(JUMP_IN_EVENTS.started);
    expect(events).toContain(JUMP_IN_EVENTS.completed);
    const startedCall = track.mock.calls.find((c) => c[0] === JUMP_IN_EVENTS.started);
    expect(startedCall?.[2]).toMatchObject({
      sid: "sid-xyz",
      story: "places/detail",
      variant: "base",
    });
    expect(actor.getSnapshot().context.target).toEqual(TARGET);

    const failTrack = vi.fn();
    let calls = 0;
    const launch: LaunchFn = async (args) => {
      calls += 1;
      if (calls === 1) throw new Error("realm unreachable");
      return okLaunch(args);
    };
    const failing = createActor(jumpInMachine, { input: inputFor(launch, failTrack) }).start();
    failing.send({ type: "START" });
    await waitFor(failing, (s) => s.matches("error"));
    expect(names(failTrack)).toContain(JUMP_IN_EVENTS.started);
    expect(names(failTrack)).toContain(JUMP_IN_EVENTS.failed);
    expect(failing.getSnapshot().context.error).toBe("realm unreachable");

    failing.send({ type: "RETRY" });
    await waitFor(failing, (s) => s.matches("launched"));
    expect(names(failTrack)).toContain(JUMP_IN_EVENTS.completed);
    expect(failing.getSnapshot().context.target).toEqual(TARGET);
  });
});
