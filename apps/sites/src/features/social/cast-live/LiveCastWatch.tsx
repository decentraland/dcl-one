import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { useAuth } from "@/data/lib/auth/context";
import type { AuthIdentity } from "@/data/lib/auth/types";
import { getWatcherToken, defaultDisplayName, type CastCredentials } from "@/data/lib/catalyst/cast";
import { guestWatcherIdentity } from "@/data/lib/catalyst/cast-guest";
import { CatalystError } from "@/data/lib/catalyst/client";
import {
  fallbackPlaceName,
  isWorldLocation,
  type WatchResult,
} from "@/data/lib/catalyst/landings/cast-watcher";
import type { TrackContext } from "@/core/lib/telemetry/track";
import { usePageView } from "@/core/lib/telemetry/page-view";

import CastWatch, { type WatchJoin, type WatchLive, type WatchRefusal } from "../cast-watch/CastWatch";
import { WatcherStage, useCastTracks } from "./CastStage";
import { CAST_WATCHER_TITLE, castPageView, type CastPageInfo } from "./page-view";
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

export const LOST_REFUSAL: WatchRefusal = {
  kind: "waiting",
  title: "This Cast has ended",
  message: "The connection to the scene closed. Join again when the Cast is back.",
};

export type WatchDeps = {
  getToken: typeof getWatcherToken;
  join: (credentials: CastCredentials, opts: { signal?: AbortSignal }) => Promise<CastSession>;
};

const LIVE_DEPS: WatchDeps = { getToken: getWatcherToken, join: (c, o) => joinCast(c, o) };

/** One watcher join: the signed token, then the room. A join the page abandoned (`signal`)
 * disconnects whatever it connected and answers null. */
export async function joinAsWatcher(
  identity: AuthIdentity,
  where: { location: string; parcel?: string },
  signal: AbortSignal,
  deps: WatchDeps = LIVE_DEPS,
): Promise<{ refused: WatchRefusal } | { credentials: CastCredentials; session: CastSession } | null> {
  let credentials: CastCredentials;
  try {
    credentials = await deps.getToken(
      identity,
      { ...where, displayName: defaultDisplayName(identity.signer) },
      { signal },
    );
  } catch (err) {
    if (signal.aborted) return null;
    return { refused: watchRefusal(err) };
  }
  if (signal.aborted) return null;
  let session: CastSession;
  try {
    session = await deps.join(credentials, { signal });
  } catch (err) {
    if (signal.aborted) return null;
    throw err;
  }
  if (signal.aborted) {
    await session.disconnect();
    return null;
  }
  return { credentials, session };
}

/** `/cast/w/{location}`: watches a parcel or world with a session or temporary browser identity. */
export default function LiveCastWatch({
  trackCtx,
  location,
  parcel,
  scenes,
}: {
  trackCtx: TrackContext;
  location: string;
  parcel?: string;
  scenes?: Array<{ base: string; title: string }> | null;
}) {
  const { identity } = useAuth();
  const [chosenParcel, setChosenParcel] = useState(
    () => parcel ?? (scenes?.length === 1 ? scenes[0].base : undefined),
  );
  const [session, setSession] = useState<CastSession | null>(null);
  const sessionRef = useRef<CastSession | null>(null);
  const aborter = useRef<AbortController | null>(null);
  const tracks = useCastTracks(session);
  const [placeName, setPlaceName] = useState(() => fallbackPlaceName(location));
  // Known once the watcher token is: the legacy watcher page view's place.
  const [joinedPlace, setJoinedPlace] = useState<CastPageInfo | null>(null);
  usePageView(castPageView(CAST_WATCHER_TITLE, joinedPlace));

  useEffect(
    () => () => {
      aborter.current?.abort();
      void sessionRef.current?.disconnect();
      sessionRef.current = null;
    },
    [],
  );

  const join = useCallback(async (): Promise<WatchJoin> => {
    aborter.current?.abort();
    const controller = new AbortController();
    aborter.current = controller;
    const signer = identity ?? await guestWatcherIdentity();
    if (controller.signal.aborted) return new Promise<WatchJoin>(() => {});
    const joined = await joinAsWatcher(signer, { location, parcel: chosenParcel }, controller.signal);
    // Abandoned: the page left or a newer join started. Never settles, so nothing renders.
    if (joined === null) return new Promise<WatchJoin>(() => {});
    if ("refused" in joined) return { refused: joined.refused };
    if (joined.credentials.placeName) setPlaceName(joined.credentials.placeName);
    setJoinedPlace({ placeName: joined.credentials.placeName || location, location, isWorld: isWorldLocation(location) });
    sessionRef.current = joined.session;
    setSession(joined.session);
    return { roomId: joined.credentials.roomId };
  }, [identity, location, chosenParcel]);

  const leave = useCallback(() => {
    aborter.current?.abort();
    const s = sessionRef.current;
    sessionRef.current = null;
    setSession(null);
    void s?.disconnect();
  }, []);

  const bindLost = useCallback(
    (onLost: (why: WatchRefusal) => void) => {
      if (!session) return () => {};
      return session.subscribe(() => {
        // Our own leave clears the ref first; anything else is a lost room.
        if (session.disconnected() && sessionRef.current === session) {
          sessionRef.current = null;
          setSession(null);
          onLost(LOST_REFUSAL);
        }
      });
    },
    [session],
  );

  const audioBlocked = session ? !session.canPlaybackAudio() : false;
  const live = useMemo<WatchLive>(
    () => ({
      join,
      chooseScene: scenes && scenes.length > 1 && !chosenParcel
        ? { scenes, onChoose: setChosenParcel }
        : undefined,
      leave,
      bindLost,
      stage: (muted) => <WatcherStage tracks={tracks} muted={muted} />,
      participantCount: session ? session.participantCount() : 0,
      audioBlocked,
      enableAudio: () => void session?.startAudio(),
    }),
    [join, leave, bindLost, tracks, session, audioBlocked, scenes, chosenParcel],
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
