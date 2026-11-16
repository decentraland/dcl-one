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
import { joinCast, type CastSession } from "./session";

/** The name a streamer who typed none is shown with. */
const DEFAULT_STREAMER_NAME = "Speaker";

const resolveToken: ResolveTokenFn = async ({ token, signal }) => ({
  info: await getStreamInfo(token, { signal }),
});

/** Asks for the camera and microphone once, then releases them: LiveKit opens its own tracks. */
const requestPermissions: RequestPermissionsFn = async () => {
  if (typeof navigator === "undefined" || !navigator.mediaDevices?.getUserMedia) {
    return { granted: false };
  }
  try {
    const probe = await navigator.mediaDevices.getUserMedia({ audio: true, video: true });
    probe.getTracks().forEach((t) => t.stop());
    return { granted: true };
  } catch {
    return { granted: false };
  }
};

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
  const tracks = useCastTracks(session);
  const [busy, setBusy] = useState(false);

  useEffect(
    () => () => {
      void sessionRef.current?.disconnect();
      sessionRef.current = null;
    },
    [],
  );

  const goLive = useCallback<GoLiveFn>(async ({ token, identity, signal }) => {
    const credentials = await getStreamerToken(token, identity.trim() || DEFAULT_STREAMER_NAME, {
      signal,
    });
    const next = await joinCast(credentials, { publish: true });
    if (signal?.aborted) {
      await next.disconnect();
      throw new Error("Join cancelled");
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
      if (!session) return () => {};
      let sharing = session.isPublishing("screen_share");
      return session.subscribe(() => {
        const now = session.isPublishing("screen_share");
        if (sharing && !now) onEnded();
        sharing = now;
      });
    },
    [session],
  );

  const toggle = useCallback(
    async (source: "microphone" | "camera") => {
      const s = sessionRef.current;
      if (!s || busy) return;
      setBusy(true);
      try {
        const on = !s.isPublishing(source);
        if (source === "microphone") await s.setMicrophone(on);
        else await s.setCamera(on);
      } finally {
        setBusy(false);
      }
    },
    [busy],
  );

  const micOn = tracks.some((t) => t.local && t.source === "microphone" && !t.muted);
  const camOn = tracks.some((t) => t.local && t.source === "camera" && !t.muted);

  const live = useMemo<LiveBindings>(
    () => ({
      goLive,
      stopShare,
      bindShareEnded,
      stage: session ? <StreamerStage tracks={tracks} muted={false} /> : undefined,
      participants: session ? session.participantCount() : undefined,
      liveControls: session ? (
        <>
          <button
            type="button"
            className={"cast-console__btn" + (micOn ? "" : " cast-console__btn--active")}
            disabled={busy}
            aria-pressed={!micOn}
            onClick={() => void toggle("microphone")}
          >
            {micOn ? "Mute microphone" : "Unmute microphone"}
          </button>
          <button
            type="button"
            className={"cast-console__btn" + (camOn ? "" : " cast-console__btn--active")}
            disabled={busy}
            aria-pressed={!camOn}
            onClick={() => void toggle("camera")}
          >
            {camOn ? "Turn camera off" : "Turn camera on"}
          </button>
        </>
      ) : undefined,
    }),
    [goLive, stopShare, bindShareEnded, session, tracks, micOn, camOn, busy, toggle],
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
