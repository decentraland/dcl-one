import { useEffect, useRef } from "react";
import { useMachine } from "@xstate/react";

import JumpInView from "@ui/explorer/components/JumpInView";

import type { TrackContext } from "@/core/lib/telemetry/track";
import {
  jumpInMachine,
  type JumpInPlace,
  type LaunchFn,
  type LaunchTarget,
  type TrackFn,
} from "./machine";

type JumpInProps = {
  place: JumpInPlace;
  trackCtx: TrackContext;
  launch?: LaunchFn;
  track?: TrackFn;
  onLaunch?: (target: LaunchTarget) => void;
};

function defaultOnLaunch(target: LaunchTarget): void {
  if (typeof window !== "undefined" && target.launchUrl) {
    window.location.assign(target.launchUrl);
  }
}

export default function JumpIn({
  place,
  trackCtx,
  launch,
  track,
  onLaunch = defaultOnLaunch,
}: JumpInProps) {
  const [state, send] = useMachine(jumpInMachine, {
    input: { place, trackCtx, launch, track },
  });

  const launchedRef = useRef(false);
  useEffect(() => {
    if (state.matches("launched") && !launchedRef.current) {
      launchedRef.current = true;
      const target = state.context.target;
      if (target) onLaunch(target);
    }
  }, [state, onLaunch]);

  return (
    <JumpInView
      placeTitle={place.title}
      launching={state.matches("launching")}
      launched={state.matches("launched")}
      errored={state.matches("error")}
      error={state.context.error}
      onStart={() => send({ type: "START" })}
      onRetry={() => send({ type: "RETRY" })}
    />
  );
}
