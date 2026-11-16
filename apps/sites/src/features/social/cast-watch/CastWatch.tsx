import { useCallback, useEffect, useRef, useState } from "react";

import StCastWatcher from "@ui/web/pages/StCastWatcher";

import { track, type TrackContext } from "@/core/lib/telemetry/track";
import { useAuth } from "@/data/lib/auth/index";
import {
  requestWatcherCredentials,
  watchErrorFor,
  type WatchError,
  type WatchResult,
} from "@/data/lib/catalyst/landings/cast-watcher";

const JOIN_CLASS = "stcastwatcher__join";
const LEAVE_CLASS = "stcastwatcher__leave";
const ICON_BTN_CLASS = "stcastwatcher__iconbtn";
const SIDE_CLOSE_CLASS = "stcastwatcher__sideclose";

const ICON_MUTE = 0;
const ICON_CHAT = 1;

type WatcherState = "onboarding" | "joining" | "live" | "waiting";

// The empty-state texts for a join the gatekeeper refused; no_active_stream keeps the page's own.
const ERROR_TEXT: Partial<Record<WatchError, { title: string; message: string }>> = {
  access_expired: {
    title: "This Cast link has expired",
    message: "Ask the scene's administrators for a new stream link.",
  },
  banned: {
    title: "You can't watch this Cast",
    message: "This account is banned from this scene or from Decentraland.",
  },
  refused: {
    title: "Your sign-in was not accepted",
    message: "Sign in again, then join.",
  },
  unavailable: {
    title: "Couldn't join this Cast",
    message: "Try again in a moment.",
  },
};

type CastWatchProps = {
  trackCtx: TrackContext;
  watch: WatchResult;
  demoMessages?: Array<{ name: string; time: string; body: string }>;
  participantCount?: number;
  unreadCount?: number;
  onTrack?: typeof track;
};

export default function CastWatch({
  trackCtx,
  watch,
  demoMessages,
  participantCount,
  unreadCount,
  onTrack = track,
}: CastWatchProps) {
  const initialState: WatcherState = watch.status === "live" ? "onboarding" : "waiting";

  const { identity, connect } = useAuth();
  const [state, setState] = useState<WatcherState>(initialState);
  const [error, setError] = useState<WatchError | undefined>(watch.error);
  const [roomId, setRoomId] = useState("");
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [isTabMuted, setIsTabMuted] = useState(false);
  const joinAbort = useRef<AbortController | null>(null);
  useEffect(() => () => joinAbort.current?.abort(), []);

  const placeName = watch.placeName;
  const [streamName, setStreamName] = useState(placeName);

  // The watcher token is the viewer's own signed POST: sign in first when there is no identity.
  const join = useCallback(async () => {
    if (joinAbort.current) return;
    const abort = new AbortController();
    joinAbort.current = abort;
    setState("joining");
    try {
      let viewer = identity;
      if (!viewer) {
        viewer = await connect().catch(() => null);
        if (!viewer) {
          setState("onboarding");
          return;
        }
      }
      const credentials = await requestWatcherCredentials(watch.location, watch.identity, viewer, {
        signal: abort.signal,
      });
      if (abort.signal.aborted) return;
      setRoomId(credentials.roomId);
      if (credentials.placeName) setStreamName(credentials.placeName);
      setError(undefined);
      onTrack(
        "cast_watch_joined",
        { room_id: credentials.roomId, place_name: credentials.placeName || placeName, stub: true },
        trackCtx,
      );
      setState("live");
    } catch (err) {
      if (abort.signal.aborted) return;
      const reason = watchErrorFor(err);
      if (reason === "no_active_stream") onTrack("cast_watch_no_stream", { location: watch.location }, trackCtx);
      if (reason === "access_expired") onTrack("cast_watch_access_expired", { location: watch.location }, trackCtx);
      setError(reason);
      setState("waiting");
    } finally {
      if (joinAbort.current === abort) joinAbort.current = null;
    }
  }, [identity, connect, watch.location, watch.identity, onTrack, placeName, trackCtx]);

  const leave = useCallback(() => {
    onTrack("cast_watch_left", {}, trackCtx);
    setSidebarOpen(false);
    setRoomId("");
    setState("onboarding");
  }, [onTrack, trackCtx]);

  const toggleMute = useCallback(() => {
    setIsTabMuted((prev) => {
      const next = !prev;
      onTrack(next ? "cast_watch_muted" : "cast_watch_unmuted", {}, trackCtx);
      return next;
    });
  }, [onTrack, trackCtx]);

  const toggleChat = useCallback(() => {
    setSidebarOpen((prev) => {
      const next = !prev;
      if (next) onTrack("cast_watch_chat_opened", {}, trackCtx);
      return next;
    });
  }, [onTrack, trackCtx]);

  const onClickCapture = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      const target = e.target as HTMLElement | null;
      const btn = target?.closest?.("button");
      if (!btn) return;

      if (btn.classList.contains(JOIN_CLASS)) {
        void join();
        return;
      }
      if (btn.classList.contains(LEAVE_CLASS)) {
        leave();
        return;
      }
      if (btn.classList.contains(SIDE_CLOSE_CLASS)) {
        setSidebarOpen(false);
        return;
      }
      if (btn.classList.contains(ICON_BTN_CLASS)) {
        const parent = btn.parentElement;
        if (!parent) return;
        const icons = Array.from(parent.children).filter((el) =>
          el.classList.contains(ICON_BTN_CLASS),
        );
        const idx = icons.indexOf(btn);
        if (idx === ICON_MUTE) toggleMute();
        else if (idx === ICON_CHAT) toggleChat();
      }
    },
    [join, leave, toggleMute, toggleChat],
  );

  const Watcher = StCastWatcher as unknown as React.ComponentType<{
    streamName?: string;
    state?: WatcherState;
    sidebarOpen?: boolean;
    isTabMuted?: boolean;
    chatFooter?: string;
    watcherTitle?: string;
    watcherMessage?: string;
    participantCount?: number;
    unreadCount?: number;
  }>;

  const chatFooter = `Jump into ${streamName} in Decentraland to participate in the chat.`;
  const errorText = error ? ERROR_TEXT[error] : undefined;

  return (
    <div
      className="cast-watch"
      data-state={state}
      data-status={watch.status}
      data-location={watch.location}
      data-error={error}
      data-room={roomId || undefined}
      onClickCapture={onClickCapture}
    >
      <Watcher
        streamName={streamName}
        state={state}
        sidebarOpen={state === "live" && sidebarOpen}
        isTabMuted={isTabMuted}
        chatFooter={chatFooter}
        watcherTitle={errorText?.title}
        watcherMessage={errorText?.message}
        participantCount={participantCount}
        unreadCount={state === "live" && !sidebarOpen ? unreadCount : 0}
      />
      {demoMessages && demoMessages.length === 0 && null}
    </div>
  );
}
