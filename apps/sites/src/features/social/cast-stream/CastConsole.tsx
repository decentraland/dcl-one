import { useEffect, useRef, useState } from "react";
import type { ComponentProps, ReactNode } from "react";
import { useMachine } from "@xstate/react";
import { useSearchParams } from "react-router";

import { StCastStreamer as CastStreamerView } from "@dcl/one-ui";

import type { TrackContext } from "@/core/lib/telemetry/track";
import SitesLayout from "../../components/chrome/SitesLayout";
import { failureWords } from "../cast-live/answer";
import { CastNotFoundScreen } from "../cast-not-found/CastNotFound";
import { castMachine, resolveCastSnapshot, slugToState, stateToSlug, DEFAULT_DEVICES, DEVICES_BLOCKED, type EndCastFn, type GoLiveFn, type RequestPermissionsFn, type ResolveTokenFn, type ShareScreenFn, type StopShareFn, type TrackFn } from "./machine";

/** What a real session adds to the console; the story pages leave it out. */
export type LiveBindings = {
  goLive: GoLiveFn;
  stopShare: StopShareFn;
  /** Calls `onEnded` when the browser ends the screen capture; answers the unsubscribe. */
  bindShareEnded: (onEnded: () => void) => () => void;
  /** Calls `onLost` when the room goes away while live, with `removed` when the server ended it
   * on purpose; answers the unsubscribe. */
  bindConnectionLost: (onLost: (removed: boolean) => void) => () => void;
  /** The published media and the room's audio, shown in the live view; `muted` mutes the audio. */
  stage?: (muted: boolean) => ReactNode;
  /** The microphone and camera as published, and their toggles. */
  micOn: boolean;
  cameraOn: boolean;
  toggleMicrophone: () => void;
  toggleCamera: () => void;
  /** A toggle is under way. */
  devicesBusy: boolean;
  /** The browser blocked playback until a click; `enableAudio` is that click. */
  audioBlocked: boolean;
  enableAudio: () => void;
  participants?: number;
  /** The room reconnects: the viewers do not receive this Cast. */
  reconnecting: boolean;
};

type CastConsoleProps = {
  trackCtx: TrackContext;
  token: string;
  identity?: string;
  initialStep?: string;
  resolveToken?: ResolveTokenFn;
  requestPermissions?: RequestPermissionsFn;
  endCast?: EndCastFn;
  shareScreen?: ShareScreenFn;
  track?: TrackFn;
  live?: LiveBindings;
};

export default function CastConsole({
  trackCtx,
  token,
  identity,
  initialStep,
  resolveToken,
  requestPermissions,
  endCast,
  shareScreen,
  track,
  live,
}: CastConsoleProps) {
  const [searchParams] = useSearchParams();

  const urlStep = (searchParams.get("step")?.trim() || initialStep) ?? undefined;
  // A live session runs the whole flow once; only the story pages jump between steps.
  const stateId = live ? slugToState(undefined) : slugToState(urlStep);

  return (
    <CastConsoleInner
      key={stateId}
      stateId={stateId}
      trackCtx={trackCtx}
      token={token}
      identity={identity}
      resolveToken={resolveToken}
      requestPermissions={requestPermissions}
      endCast={endCast}
      shareScreen={shareScreen}
      track={track}
      live={live}
    />
  );
}

type InnerProps = {
  stateId: ReturnType<typeof slugToState>;
  trackCtx: TrackContext;
  token: string;
  identity?: string;
  resolveToken?: ResolveTokenFn;
  requestPermissions?: RequestPermissionsFn;
  endCast?: EndCastFn;
  shareScreen?: ShareScreenFn;
  track?: TrackFn;
  live?: LiveBindings;
};

function CastConsoleInner({
  stateId,
  trackCtx,
  token,
  identity,
  resolveToken,
  requestPermissions,
  endCast,
  shareScreen,
  track,
  live,
}: InnerProps) {
  const [, setSearchParams] = useSearchParams();
  const [name, setName] = useState(identity ?? "");

  const snapshot = useRef(
    resolveCastSnapshot({
      step: stateId,
      trackCtx,
      token,
      identity,
      resolveToken,
      requestPermissions,
      endCast,
      shareScreen,
      goLive: live?.goLive,
      stopShare: live?.stopShare,
      track,
    }),
  ).current;

  const [state, send] = useMachine(castMachine, {
    input: {
      trackCtx,
      token,
      identity,
      resolveToken,
      requestPermissions,
      endCast,
      shareScreen,
      goLive: live?.goLive,
      stopShare: live?.stopShare,
      track,
    },
    snapshot,
  });

  const bindShareEnded = live?.bindShareEnded;
  useEffect(() => bindShareEnded?.(() => send({ type: "SHARE_ENDED" })), [bindShareEnded, send]);
  const bindConnectionLost = live?.bindConnectionLost;
  useEffect(
    () => bindConnectionLost?.((removed) => send({ type: "CONNECTION_LOST", removed })),
    [bindConnectionLost, send],
  );

  const value = state.matches("live") ? "live" : (state.value as string);
  const step = stateToSlug(value);

  const lastStep = useRef<string | null>(null);
  useEffect(() => {
    if (live) return;
    if (lastStep.current === step) return;
    lastStep.current = step;
    setSearchParams(
      (prev) => {
        const params = new URLSearchParams(prev);
        if (params.get("step") === step) return params;
        params.set("step", step);
        return params;
      },
      { replace: true, preventScrollReset: true },
    );
  }, [step, setSearchParams, live]);

  const placeName = state.context.info?.placeName ?? "Genesis Plaza";
  const displayName = state.context.identity || identity || "";

  // The story pages have no session: their microphone and camera switch only on screen.
  const [simMic, setSimMic] = useState(true);
  const [simCamera, setSimCamera] = useState(true);
  const [tabMuted, setTabMuted] = useState(false);

  return (
    <div className="cast-console" data-step={step}>
      {value === "tokenCheck" && (
        <StCastStreamer state="joining" streamName={placeName} toasts={[]} />
      )}

      {value === "deviceSelect" && (
        <StCastStreamer
          state="onboarding"
          streamName={placeName}
          displayName={name}
          onNameChange={setName}
          devices={state.context.devices}
          // One click runs the whole join: devices, then access, then the room.
          onJoin={() => send({ type: "SELECT_DEVICES", devices: DEFAULT_DEVICES, name, join: true })}
          toasts={[]}
        />
      )}

      {value === "permissions" && (
        <StCastStreamer
          state="joining"
          streamName={placeName}
          toasts={
            state.context.permissionsDenied
              ? [
                  {
                    id: "perm",
                    title: "Camera & microphone blocked",
                    message: DEVICES_BLOCKED,
                    action: { label: "Retry", onClick: () => send({ type: "RETRY_PERMISSIONS" }) },
                    onDismiss: () => send({ type: "BACK" }),
                  },
                ]
              : []
          }
        />
      )}

      {value === "preview" && (
        <StCastStreamer
          state="onboarding"
          streamName={placeName}
          displayName={name}
          onNameChange={setName}
          devices={state.context.devices}
          onJoin={() => send({ type: "JOIN", name })}
          toasts={
            state.context.joinError
              ? [
                  {
                    id: "join",
                    title: "Could not join the cast",
                    message: state.context.joinWords ?? failureWords(state.context.joinError),
                    action: { label: "Retry", onClick: () => send({ type: "JOIN", name }) },
                  },
                ]
              : []
          }
        />
      )}

      {value === "joining" && (
        <StCastStreamer state="joining" streamName={placeName} toasts={[]} />
      )}

      {value === "live" && (
        <StCastStreamer
          state="live"
          streamName={placeName}
          displayName={displayName || "Streamer"}
          participants={live?.participants ?? 1}
          stage={live?.stage?.(tabMuted)}
          micOn={live ? live.micOn : simMic}
          cameraOn={live ? live.cameraOn : simCamera}
          onToggleMic={live ? live.toggleMicrophone : () => setSimMic((on) => !on)}
          onToggleCamera={live ? live.toggleCamera : () => setSimCamera((on) => !on)}
          devicesBusy={live?.devicesBusy}
          sharing={state.matches({ live: "sharing" })}
          shareBusy={state.matches({ live: "publishingShare" })}
          onToggleShare={() => send({ type: "TOGGLE_SCREENSHARE" })}
          onLeaveStream={() => send({ type: "LEAVE" })}
          tabMuted={tabMuted}
          onToggleTabMute={() => setTabMuted((muted) => !muted)}
          reconnecting={live?.reconnecting}
          toasts={[
            ...(live?.reconnecting
              ? [
                  {
                    id: "net",
                    title: "Reconnecting\u{2026}",
                    message: "Your viewers do not receive your Cast until the connection is back.",
                  },
                ]
              : []),
            ...(state.context.screenShareFailed
              ? [
                  {
                    id: "ss",
                    title: "Screen sharing failed",
                    message: "Your screen share stopped. Click retry to share again.",
                    action: { label: "Retry", onClick: () => send({ type: "TOGGLE_SCREENSHARE" }) },
                  },
                ]
              : []),
            ...(live?.audioBlocked && live.enableAudio
              ? [
                  {
                    id: "audio",
                    title: "Audio is paused",
                    message: "Your browser needs a click before the Cast can play sound.",
                    action: { label: "Enable audio", onClick: live.enableAudio },
                  },
                ]
              : []),
          ]}
        />
      )}

      {value === "ending" && (
        <StCastStreamer
          state="joining"
          streamName={placeName}
          toasts={[
            {
              id: "ending",
              title: "Ending your cast\u{2026}",
              message: "Closing the room and releasing your devices.",
            },
          ]}
        />
      )}

      {value === "ended" && state.context.removed && (
        // What the server ended has no Rejoin: the server would refuse it.
        <StCastStreamer
          state="error"
          streamName={placeName}
          errorTitle="You were removed from this Cast"
          errorMessage="The server closed your connection. To cast again, get a new stream link from the Admin Smart Item in your scene."
          toasts={[]}
        />
      )}

      {value === "ended" && !state.context.removed && (
        <StCastStreamer
          state="error"
          streamName={placeName}
          errorTitle={
            state.context.connectionLost ? "Your cast was disconnected" : "Your cast has ended"
          }
          errorMessage={
            state.context.connectionLost
              ? "The connection to the scene was lost. Rejoin checks your stream link and opens its lobby."
              : "Thanks for casting. Start a new cast from the Admin Smart Item in your scene."
          }
          onRetry={() => send({ type: "RETRY" })}
          retryLabel="Rejoin"
          toasts={[]}
        />
      )}

      {value === "unreachable" && (
        <StCastStreamer
          state="error"
          streamName={placeName}
          errorTitle="Could not open this stream link"
          errorMessage={failureWords(state.context.invalidReason)}
          onRetry={() => send({ type: "RETRY" })}
          retryLabel="Retry"
          toasts={[]}
        />
      )}

      {value === "invalid" && (
        <CastNotFoundScreen
          description={
            (state.context.invalidWords ?? "Invalid streaming key") +
            ". Stream links are generated on-demand by the Admin Smart Item in Decentraland scenes."
          }
        />
      )}
    </div>
  );
}

function StCastStreamer(props: ComponentProps<typeof CastStreamerView>) {
  return (
    <SitesLayout active="legal">
      <CastStreamerView {...props} />
    </SitesLayout>
  );
}
