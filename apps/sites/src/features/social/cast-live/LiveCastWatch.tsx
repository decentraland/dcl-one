import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { useAuth } from "@/data/lib/auth/context";
import { getWatcherToken, defaultDisplayName } from "@/data/lib/catalyst/cast";
import { CatalystError } from "@/data/lib/catalyst/client";
import {
  fallbackPlaceName,
  isWorldLocation,
  type WatchResult,
} from "@/data/lib/catalyst/landings/cast-watcher";
import type { TrackContext } from "@/core/lib/telemetry/track";
import { openSignIn } from "../../components/auth/signin-store";

import CastWatch, { type WatchLive, type WatchRefusal } from "../cast-watch/CastWatch";
import { WatcherStage, useCastTracks } from "./CastStage";
import { joinCast, type CastSession } from "./session";

/** How the gatekeeper's refusals of `POST /cast/watcher-token` read on the page. */
export function watchRefusal(err: unknown): WatchRefusal {
  const status = err instanceof CatalystError ? err.status : 0;
  const server = err instanceof CatalystError && err.serverMessage ? err.message : null;
  if (status === 404) {
    return {
      kind: "waiting",
      title: "No one is currently casting to this link...",
      message: "This Decentraland Cast may be starting soon or has already ended.",
    };
  }
  if (status === 401 && server === "Stream access has expired") {
    return {
      kind: "waiting",
      title: "This Cast has ended",
      message: "The stream link for this place has expired.",
    };
  }
  if (status === 403) {
    return { kind: "error", title: "You can't watch this Cast", message: server ?? "Access denied." };
  }
  return {
    kind: "error",
    title: "Could not join the cast",
    message: server ?? "Try again in a moment.",
  };
}

/** `/cast/w/{location}`: watches the Cast of a parcel or a world with the viewer's own wallet. */
export default function LiveCastWatch({
  trackCtx,
  location,
  parcel,
}: {
  trackCtx: TrackContext;
  location: string;
  parcel?: string;
}) {
  const { identity } = useAuth();
  const [session, setSession] = useState<CastSession | null>(null);
  const sessionRef = useRef<CastSession | null>(null);
  const tracks = useCastTracks(session);
  const [placeName, setPlaceName] = useState(() => fallbackPlaceName(location));

  useEffect(
    () => () => {
      void sessionRef.current?.disconnect();
      sessionRef.current = null;
    },
    [],
  );

  const join = useCallback(async (): Promise<WatchRefusal | null> => {
    if (!identity) {
      openSignIn();
      return { kind: "error", title: "Sign in to watch", message: "Watching a Cast needs your Decentraland account." };
    }
    let credentials;
    try {
      credentials = await getWatcherToken(identity, {
        location,
        parcel,
        displayName: defaultDisplayName(identity.signer),
      });
    } catch (err) {
      return watchRefusal(err);
    }
    if (credentials.placeName) setPlaceName(credentials.placeName);
    const next = await joinCast(credentials, { publish: false });
    await next.startAudio().catch(() => {});
    sessionRef.current = next;
    setSession(next);
    return null;
  }, [identity, location, parcel]);

  const leave = useCallback(() => {
    const s = sessionRef.current;
    sessionRef.current = null;
    setSession(null);
    void s?.disconnect();
  }, []);

  const live = useMemo<WatchLive>(
    () => ({
      join,
      leave,
      stage: (muted) => <WatcherStage tracks={tracks} muted={muted} />,
      participantCount: session ? session.participantCount() : undefined,
    }),
    [join, leave, tracks, session],
  );

  const watch: WatchResult = {
    status: "live",
    location,
    isWorld: isWorldLocation(location),
    placeName,
    identity: identity ? defaultDisplayName(identity.signer) : "Viewer",
  };

  return <CastWatch trackCtx={trackCtx} watch={watch} live={live} />;
}
