import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { useAuth } from "@/data/lib/auth/context";
import type { AuthIdentity } from "@/data/lib/auth/types";
import { getWatcherToken, defaultDisplayName, type CastCredentials } from "@/data/lib/catalyst/cast";
import { CatalystError } from "@/data/lib/catalyst/client";
import {
  fallbackPlaceName,
  isWorldLocation,
  type WatchResult,
} from "@/data/lib/catalyst/landings/cast-watcher";
import type { TrackContext } from "@/core/lib/telemetry/track";
import { usePageView } from "@/core/lib/telemetry/page-view";

import { openSignIn } from "../../components/auth/signin-store";
import CastWatch, { type WatchJoin, type WatchLive, type WatchLoss, type WatchRefusal } from "../cast-watch/CastWatch";
import { JOIN_TIMEOUT_MS, NO_ANSWER } from "./answer";
import { WatcherStage, useCastTracks, useWatcherStage } from "./CastStage";
import { CAST_WATCHER_TITLE, castPageView, type CastPageInfo } from "./page-view";
import { canDecode, joinCast, onRoomLost, type CastSession } from "./session";
import { stalledCast } from "./stage";

/** How the gatekeeper's refusals of `POST /cast/watcher-token` read on the page. Null for a
 * request that failed, which is no refusal. */
export function watchRefusal(err: unknown): WatchRefusal | null {
  if (!(err instanceof CatalystError) || err.status < 400 || err.status >= 500) return null;
  const server = err.serverMessage ? err.message : "";
  if (err.status === 404 && !server.includes("place found")) {
    return {
      kind: "waiting",
      title: "No one is currently casting to this link...",
      message: "This Decentraland Cast may be starting soon or has already ended.",
    };
  }
  if (err.status === 401 && server.startsWith("Stream access has expired")) {
    return {
      kind: "waiting",
      title: "This Cast has ended",
      message: "The stream link for this place has expired.",
    };
  }
  return {
    kind: "error",
    title: "You cannot join this Cast",
    message: server || "Access denied.",
    // A ban holds for a new join; a place or a world's access can come back.
    again: err.status !== 403 && !server.includes("deny-listed"),
  };
}

export type WatchDeps = {
  getToken: typeof getWatcherToken;
  join: (credentials: CastCredentials, opts: { signal?: AbortSignal }) => Promise<CastSession>;
};

const LIVE_DEPS: WatchDeps = { getToken: getWatcherToken, join: (c, o) => joinCast(c, o) };

/** An automatic join has this long to answer, so the three tries of a lost Cast end in 47 s (their
 * waits and three bounds), not in 152 s. A join by a click keeps `JOIN_TIMEOUT_MS`. */
export const REJOIN_TIMEOUT_MS = 10_000;

/** How often the page asks the browser for the codec of a video it could not decode, and how
 * many times it joins again for it. Longer than `REJOIN_TIMEOUT_MS`: a join has its answer
 * before the next. */
const CODEC_RETRY_MS = 15_000;
const CODEC_REJOINS = 3;

/** `run`'s answer, or a rejection with `NO_ANSWER` when it has none in `ms`: its signal aborts
 * then, as it does with `signal`. A run the page abandoned (`signal`) is never rejected. */
export function answeredIn<T>(ms: number, signal: AbortSignal, run: (signal: AbortSignal) => Promise<T>): Promise<T> {
  const timeout = AbortSignal.timeout(ms);
  const late = new Promise<never>((_, reject) =>
    timeout.addEventListener("abort", () => signal.aborted || reject(new Error(NO_ANSWER))),
  );
  return Promise.race([run(AbortSignal.any([signal, timeout])), late]);
}

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
    const refused = watchRefusal(err);
    if (!refused) throw err;
    return { refused };
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
  const aborter = useRef<AbortController | null>(null);
  // The identities of this page's connections: the people count leaves them out.
  const own = useRef(new Set<string>());
  const tracks = useCastTracks(session);
  const stage = useWatcherStage(tracks);
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

  const join = useCallback(async (auto: boolean): Promise<WatchJoin> => {
    if (!identity) {
      // The sign-in dialog has the visitor: the lobby stays behind it, with no notice.
      openSignIn();
      return null;
    }
    aborter.current?.abort();
    const controller = new AbortController();
    aborter.current = controller;
    const joined = await answeredIn(auto ? REJOIN_TIMEOUT_MS : JOIN_TIMEOUT_MS, controller.signal, (signal) =>
      joinAsWatcher(identity, { location, parcel }, signal),
    );
    // Abandoned: the page left or a newer join started. Never settles, so nothing renders.
    if (joined === null) return new Promise<WatchJoin>(() => {});
    if ("refused" in joined) return { refused: joined.refused };
    if (joined.credentials.placeName) setPlaceName(joined.credentials.placeName);
    setJoinedPlace({ placeName: joined.credentials.placeName || location, location, isWorld: isWorldLocation(location) });
    // The room a new join leaves (a join for a codec) is never connected after it.
    const earlier = sessionRef.current;
    own.current.add(joined.session.identity());
    sessionRef.current = joined.session;
    setSession(joined.session);
    void earlier?.disconnect();
    return { roomId: joined.credentials.roomId };
  }, [identity, location, parcel]);

  const leave = useCallback(() => {
    aborter.current?.abort();
    const s = sessionRef.current;
    sessionRef.current = null;
    setSession(null);
    void s?.disconnect();
  }, []);

  const bindLost = useCallback(
    (onLost: (why: WatchLoss) => void) => {
      if (!session) return () => {};
      return onRoomLost(session, (removed) => {
        // Our own leave clears the ref first; anything else is a lost room.
        if (sessionRef.current !== session) return;
        sessionRef.current = null;
        setSession(null);
        onLost(removed ? "removed" : "network");
      });
    },
    [session],
  );

  const audioBlocked = session ? !session.canPlaybackAudio() : false;
  // A whole reconnection empties the room, so it is asked before what is on air.
  const onAir = session?.onAir();
  const away = session?.reconnecting() === "all" || (onAir && stalledCast(tracks, stage.shown));
  // LIVE needs the picture of the stage; a Cast with no video has none to wait for.
  const picture = stage.shown ? stage.shown.picture : !session?.videoOnAir();
  const codec = session?.undecodable() ?? null;
  const signal = away ? "reconnecting" : !onAir ? "none" : picture ? "live" : codec ? "undecodable" : "arriving";

  // The video comes only on a connection made after the browser has the codec: the page joins
  // again then, and the room it leaves plays its sound until the new one is there.
  const codecJoins = useRef(CODEC_REJOINS);
  useEffect(() => {
    if (!session || !codec) return;
    const timer = setInterval(() => {
      if (codecJoins.current === 0 || !canDecode(codec)) return;
      codecJoins.current -= 1;
      join(true).catch(() => {});
    }, CODEC_RETRY_MS);
    return () => clearInterval(timer);
  }, [session, codec, join]);
  const live = useMemo<WatchLive>(
    () => ({
      join,
      leave,
      bindLost,
      stage: (muted) => <WatcherStage tracks={tracks} stage={stage} muted={muted} />,
      participantCount: session ? session.participantCount(own.current) : 0,
      signal,
      audioBlocked,
      enableAudio: () => void session?.startAudio(),
    }),
    [join, leave, bindLost, tracks, stage, session, audioBlocked, signal],
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
