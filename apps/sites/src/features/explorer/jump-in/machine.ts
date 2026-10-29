import { assign, fromPromise, setup } from "xstate";
import { toErrorMessage } from "@/core/lib/errors";

import { playUrl } from "@/data/lib/catalyst/bases";
import { catalystBase } from "@/data/lib/catalyst/client";
import { track as defaultTrack, type TrackContext, type TrackFn } from "@/core/lib/telemetry/track";

export type LaunchTarget = {
  launchUrl: string;
  realm?: string;
};

export type LaunchFn = (args: {
  place: JumpInPlace;
  signal?: AbortSignal;
}) => Promise<LaunchTarget>;

export type JumpInPlace = {
  id: string;
  title: string;
  base_position: string;
  world?: boolean;
  world_name?: string | null;
};

export type { TrackFn };

type JumpInInput = {
  place: JumpInPlace;
  trackCtx: TrackContext;
  launch?: LaunchFn;
  track?: TrackFn;
};

type JumpInContext = {
  place: JumpInPlace;
  trackCtx: TrackContext;
  launch: LaunchFn;
  track: TrackFn;
  target?: LaunchTarget;
  error?: string;
};

type JumpInEvent =
  | { type: "START" }
  | { type: "RETRY" };

export const JUMP_IN_EVENTS = {
  started: "jump_in_started",
  completed: "jump_in_completed",
  failed: "jump_in_failed",
} as const;

type AboutRealm = { configurations?: { realmName?: string } };

function buildLaunchUrl(place: JumpInPlace, realm?: string): string {
  const params = new URLSearchParams();
  if (place.world && place.world_name) {
    params.set("realm", place.world_name);
  } else {
    const pos = (place.base_position || "0,0").trim();
    params.set("position", pos);
    if (realm) params.set("realm", realm);
  }
  return playUrl(params);
}

const resolveLaunch: LaunchFn = async ({ place, signal }) => {
  let realm: string | undefined;
  try {
    const res = await fetch(`${catalystBase()}/about`, {
      headers: { accept: "application/json" },
      signal,
    });
    if (res.ok) {
      const about = (await res.json()) as AboutRealm;
      realm = about.configurations?.realmName || undefined;
    }
  } catch {
  }
  return { launchUrl: buildLaunchUrl(place, realm), realm };
};

export const jumpInMachine = setup({
  types: {
    context: {} as JumpInContext,
    events: {} as JumpInEvent,
    input: {} as JumpInInput,
  },
  actors: {
    launchPlace: fromPromise<LaunchTarget, { place: JumpInPlace; launch: LaunchFn }>(
      ({ input, signal }) => input.launch({ place: input.place, signal }),
    ),
  },
  actions: {
    trackStarted: ({ context }) =>
      context.track(
        JUMP_IN_EVENTS.started,
        { place_id: context.place.id },
        context.trackCtx,
      ),
    trackCompleted: ({ context }) =>
      context.track(
        JUMP_IN_EVENTS.completed,
        { place_id: context.place.id, launch_url: context.target?.launchUrl },
        context.trackCtx,
      ),
    trackFailed: ({ context }) =>
      context.track(
        JUMP_IN_EVENTS.failed,
        { place_id: context.place.id, error: context.error },
        context.trackCtx,
      ),
  },
}).createMachine({
  id: "jumpIn",
  context: ({ input }) => ({
    place: input.place,
    trackCtx: input.trackCtx,
    launch: input.launch ?? resolveLaunch,
    track: input.track ?? defaultTrack,
  }),
  initial: "idle",
  states: {
    idle: {
      on: {
        START: { target: "launching", actions: "trackStarted" },
      },
    },
    launching: {
      entry: assign({ error: undefined }),
      invoke: {
        id: "launchPlace",
        src: "launchPlace",
        input: ({ context }) => ({ place: context.place, launch: context.launch }),
        onDone: {
          target: "launched",
          actions: [
            assign({ target: ({ event }) => event.output }),
            "trackCompleted",
          ],
        },
        onError: {
          target: "error",
          actions: [
            assign({
              error: ({ event }) =>
                toErrorMessage(event.error, "launch failed"),
            }),
            "trackFailed",
          ],
        },
      },
    },
    launched: {
      type: "final",
    },
    error: {
      on: {
        RETRY: { target: "launching" },
      },
    },
  },
});

