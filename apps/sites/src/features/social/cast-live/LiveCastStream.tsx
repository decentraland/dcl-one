import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { getStreamInfo, getStreamerToken } from "@/data/lib/catalyst/cast";
import type { TrackContext } from "@/core/lib/telemetry/track";

import CastConsole, { type LiveBindings } from "../cast-stream/CastConsole";
import type {
  EndCastFn,
  GoLiveFn,
  RequestPermissionsFn,
  ResolveTokenFn,
  ShareScreenFn,
} from "../cast-stream/machine";
import { StreamerStage, useCastTracks } from "./CastStage";
import { joinCast, probeDevices, type CastSession, type Publish } from "./session";

/** The name a streamer who typed none is shown with. */
const DEFAULT_STREAMER_NAME = "Speaker";

const resolveToken: ResolveTokenFn = async ({ token, signal }) => ({
  info: await getStreamInfo(token, { signal }),
});

/** A session's change feed as a binding: `onChange(session)` on every room event. */
function bindSession(session: CastSession | null, onChange: (s: CastSession) => void): () => void {
  return session ? session.subscribe(() => onChange(session)) : () => {};
}

/** `/cast/s/{streamingKey}`: publishes the camera, microphone and screen into the scene room. */
export default function LiveCastStream({
  trackCtx,
  streamingKey,
}: {
  trackCtx: TrackContext;
  streamingKey: string;
}) {
  const [session, setSession] = useState<CastSession | null>(null);
  const sessionRef = useRef<CastSession | null>(null);
  const devices = useRef<Publish>({ microphone: true, camera: true });
  const busy = useRef(false);
  const [toggling, setToggling] = useState(false);
  const tracks = useCastTracks(session);

  useEffect(
    () => () => {
      void sessionRef.current?.disconnect();
      sessionRef.current = null;
    },
    [],
  );

  /** Each device on its own: either one, or neither with a screen share, is enough to cast. */
  const requestPermissions = useCallback<RequestPermissionsFn>(async () => {
    devices.current = await probeDevices();
    const md = typeof navigator !== "undefined" ? navigator.mediaDevices : undefined;
    const canShare = typeof md?.getDisplayMedia === "function";
    return { granted: devices.current.microphone || devices.current.camera || canShare };
  }, []);

  const goLive = useCallback<GoLiveFn>(async ({ token, identity, signal }) => {
    const credentials = await getStreamerToken(token, identity.trim() || DEFAULT_STREAMER_NAME, {
      signal,
    });
    const next = await joinCast(credentials, { publish: devices.current, signal });
    if (signal?.aborted) {
      await next.disconnect();
      throw new DOMException("Join cancelled", "AbortError");
    }
    sessionRef.current = next;
    setSession(next);
    return credentials;
  }, []);

  const shareScreen = useCallback<ShareScreenFn>(async () => {
    const s = sessionRef.current;
    return { published: s ? await s.setScreenShare(true) : false };
  }, []);

  const endCast = useCallback<EndCastFn>(async () => {
    const s = sessionRef.current;
    sessionRef.current = null;
    await s?.disconnect();
  }, []);

  const stopShare = useCallback(() => {
    void sessionRef.current?.setScreenShare(false);
  }, []);

  const bindShareEnded = useCallback(
    (onEnded: () => void) => {
      let sharing = session?.isPublishing("screen_share") ?? false;
      return bindSession(session, (s) => {
        const now = s.isPublishing("screen_share");
        if (sharing && !now) onEnded();
        sharing = now;
      });
    },
    [session],
  );

  const bindConnectionLost = useCallback(
    (onLost: () => void) =>
      bindSession(session, (s) => {
        // Our own leave clears the ref first; anything else is a lost room.
        if (s.disconnected() && sessionRef.current === s) {
          sessionRef.current = null;
          onLost();
        }
      }),
    [session],
  );

  const toggle = useCallback(async (source: "microphone" | "camera") => {
    const s = sessionRef.current;
    if (!s || busy.current) return;
    busy.current = true;
    setToggling(true);
    try {
      const on = !s.isPublishing(source);
      if (source === "microphone") await s.setMicrophone(on);
      else await s.setCamera(on);
    } finally {
      busy.current = false;
      setToggling(false);
    }
  }, []);

  const micOn = tracks.some((t) => t.local && t.source === "microphone" && !t.muted);
  const camOn = tracks.some((t) => t.local && t.source === "camera" && !t.muted);
  const audioBlocked = session ? !session.canPlaybackAudio() : false;

  const live = useMemo<LiveBindings>(
    () => ({
      goLive,
      stopShare,
      bindShareEnded,
      bindConnectionLost,
      stage: session ? (muted) => <StreamerStage tracks={tracks} muted={muted} /> : undefined,
      participants: session ? session.participantCount() : undefined,
      micOn,
      cameraOn: camOn,
      toggleMicrophone: () => void toggle("microphone"),
      toggleCamera: () => void toggle("camera"),
      devicesBusy: toggling,
      audioBlocked,
      enableAudio: () => void session?.startAudio(),
    }),
    [
      goLive,
      stopShare,
      bindShareEnded,
      bindConnectionLost,
      session,
      tracks,
      micOn,
      camOn,
      toggling,
      toggle,
      audioBlocked,
    ],
  );

  return (
    <CastConsole
      trackCtx={trackCtx}
      token={streamingKey}
      resolveToken={resolveToken}
      requestPermissions={requestPermissions}
      shareScreen={shareScreen}
      endCast={endCast}
      live={live}
    />
  );
}
