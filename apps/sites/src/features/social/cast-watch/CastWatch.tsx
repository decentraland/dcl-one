import { useCallback, useEffect, useState, type ReactNode } from "react";

import { StCastWatcher } from "@dcl/one-ui";

import { track, type TrackContext } from "@/core/lib/telemetry/track";
import { fallbackPlaceName, type WatchResult } from "@/data/lib/catalyst/landings/cast-watcher";
import SitesLayout from "../../components/chrome/SitesLayout";
import { failureWords } from "../cast-live/answer";

const JOIN_CLASS = "stcastwatcher__join";
const LEAVE_CLASS = "stcastwatcher__leave";
const ICON_BTN_CLASS = "stcastwatcher__iconbtn";
const SIDE_CLOSE_CLASS = "stcastwatcher__sideclose";

const ICON_MUTE = 0;
const ICON_CHAT = 1;

type WatcherState = "onboarding" | "joining" | "live" | "waiting" | "ended";

/** Waits before each automatic join after a lost connection; their number is the most tries. */
const REJOIN_WAITS_MS = [2_000, 5_000, 10_000];

/** Why a real join did not go live, or a watched room ended: the watcher page shows it in place
 * of the stream. An `error` is the end card, with Join again when `again`. */
export type WatchRefusal = { kind: "waiting" | "error"; title: string; message: string; again?: boolean };

/** How a watched room went away: the connection was lost, or the server ended it on purpose. */
export type WatchLoss = "network" | "removed";

/** A join's outcome: connected to `roomId`, refused, or null while the sign-in dialog has the
 * visitor: the lobby stays, with no notice. */
export type WatchJoin = { roomId: string; refused?: undefined } | { refused: WatchRefusal; roomId?: undefined } | null;

/** A real LiveKit session behind the watcher page; the story page leaves it out. */
export type WatchLive = {
  /** `auto`: the page joins again by itself, with a short bound. */
  join: (auto: boolean) => Promise<WatchJoin>;
  leave: () => void;
  stage: (muted: boolean) => ReactNode;
  participantCount: number;
  /** `live`: the Cast arrives. `arriving`: its first picture is not there yet. `undecodable`: its
   * sound arrives and this browser cannot decode its video. `reconnecting`: it is on air and does
   * not arrive, or the room reconnects. `none`: nothing is on air, and the joined room reads as
   * waiting. */
  signal: "live" | "arriving" | "undecodable" | "reconnecting" | "none";
  /** Calls `onLost` when the room goes away while watching; answers the unsubscribe. */
  bindLost: (onLost: (why: WatchLoss) => void) => () => void;
  /** The browser blocks playback until a click (Safari). */
  audioBlocked: boolean;
  enableAudio: () => void;
};

type CastWatchProps = {
  trackCtx: TrackContext;
  watch: WatchResult;
  demoMessages?: Array<{ name: string; time: string; body: string }>;
  participantCount?: number;
  unreadCount?: number;
  onTrack?: typeof track;
  live?: WatchLive;
};

export default function CastWatch({
  trackCtx,
  watch,
  demoMessages,
  participantCount,
  unreadCount,
  onTrack = track,
  live,
}: CastWatchProps) {
  const initialState: WatcherState = watch.status === "live" ? "onboarding" : "waiting";

  const [state, setState] = useState<WatcherState>(initialState);
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [isTabMuted, setIsTabMuted] = useState(false);
  const [refusal, setRefusal] = useState<WatchRefusal | null>(null);
  /** Automatic joins since the Cast last arrived or the viewer last joined. */
  const [tries, setTries] = useState(0);
  /** The next automatic join waits for its time. */
  const [due, setDue] = useState(false);
  /** The viewer closed the notice of a video this browser cannot play. */
  const [codecRead, setCodecRead] = useState(false);

  const roomId = watch.credentials?.roomId ?? "";
  const placeName = watch.placeName;

  // A lost connection is joined again `REJOIN_WAITS_MS.length` times and no more: only the Cast
  // that arrives, or a click, gives new tries. What the server ended waits for a click.
  const lose = useCallback((why: WatchLoss) => {
    onTrack("cast_watch_lost", { reason: why }, trackCtx);
    if (why === "network") {
      setTries((n) => n + 1);
      setState("joining");
      return setDue(true);
    }
    setRefusal({ kind: "error", title: "You were removed from this Cast", message: "The server closed your connection.", again: true });
    setState("ended");
  }, [onTrack, trackCtx]);
  // The count changes in the render that shows its cause, never in a later effect: a loss
  // between that render and its effects counts from the right number.
  if (state === "live" && live?.signal === "live" && tries) setTries(0);
  if (due && tries > REJOIN_WAITS_MS.length) {
    const message = `The Cast did not return in ${REJOIN_WAITS_MS.length} tries.`;
    setRefusal({ kind: "error", title: "The connection was lost", message, again: true });
    setState("ended");
    setDue(false);
  }

  const liveJoin = live?.join;
  const join = useCallback((auto = false) => {
    setState("joining");
    setRefusal(null);
    setDue(false);
    if (!auto) {
      setTries(0);
      setCodecRead(false);
    }
    if (!liveJoin) {
      onTrack(
        "cast_watch_joined",
        { room_id: roomId, place_name: placeName, stub: true },
        trackCtx,
      );
      setTimeout(() => setState("live"), 600);
      return;
    }
    const refuse = (refused: WatchRefusal, reason = refused.title) => {
      onTrack("cast_watch_refused", { kind: refused.kind, reason }, trackCtx);
      setRefusal(refused);
      setState(refused.kind === "waiting" ? "waiting" : "ended");
    };
    liveJoin(auto).then(
      (joined) => {
        if (!joined) return setState("onboarding");
        if (joined.refused) return refuse(joined.refused);
        onTrack(
          "cast_watch_joined",
          { room_id: joined.roomId, place_name: placeName, stub: false },
          trackCtx,
        );
        setState("live");
      },
      (err: unknown) => {
        // A join that failed is no refusal: an automatic one is a try that is used up.
        if (auto) return lose("network");
        // The card says plain words; the error's text is for telemetry and the console.
        const reason = err instanceof Error ? err.message : String(err);
        console.warn("Cast join failed:", reason);
        refuse({ kind: "error", title: "Could not join the cast", message: failureWords(reason), again: true }, reason);
      },
    );
  }, [liveJoin, onTrack, roomId, placeName, trackCtx, lose]);

  const bindLost = live?.bindLost;
  useEffect(() => bindLost?.(lose), [bindLost, lose]);

  useEffect(() => {
    if (!due) return;
    const timer = setTimeout(() => join(true), REJOIN_WAITS_MS[tries - 1]);
    return () => clearTimeout(timer);
  }, [due, tries, join]);

  const leave = useCallback(() => {
    onTrack("cast_watch_left", {}, trackCtx);
    live?.leave();
    setDue(false);
    setSidebarOpen(false);
    setState("onboarding");
  }, [live, onTrack, trackCtx]);

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
        join();
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
    participantCount?: number;
    unreadCount?: number;
    stage?: ReactNode;
    messages?: Array<{ name: string; time: string; body: string }>;
    watcherTitle?: ReactNode;
    watcherMessage?: ReactNode;
    canRejoin?: boolean;
    joiningLabel?: string;
    canLeave?: boolean;
    toasts?: Array<{
      title: ReactNode;
      message: ReactNode;
      action?: { label?: string; onClick?: () => void };
      onDismiss?: () => void;
    }>;
    reconnecting?: boolean;
  }>;

  // LIVE only while the Cast arrives: a joined room with nothing on air waits like a link with no stream.
  const signal = state === "live" && live ? live.signal : "live";
  const shown = signal === "none" ? "waiting" : state;
  // An automatic join waits or runs.
  const retrying = state === "joining" && tries > 0;
  const reconnecting = signal === "reconnecting";
  const chatFooter = `Jump into ${placeName} in Decentraland to participate in the chat.`;

  return (
    <div
      className="cast-watch"
      data-state={reconnecting || retrying ? "reconnecting" : shown}
      data-status={watch.status}
      data-location={watch.location}
      onClickCapture={onClickCapture}
    >
      <SitesLayout active="legal">
        <Watcher
          // The lobby names the place as the link does, before the join and after LEAVE.
          streamName={fallbackPlaceName(watch.location)}
          state={shown}
          reconnecting={signal !== "live"}
          sidebarOpen={shown === "live" && sidebarOpen}
          isTabMuted={isTabMuted}
          chatFooter={chatFooter}
          participantCount={live ? live.participantCount : participantCount}
          unreadCount={state === "live" && !sidebarOpen && !live ? unreadCount : 0}
          stage={live && state === "live" ? live.stage(isTabMuted) : undefined}
          messages={live ? [] : undefined}
          watcherTitle={refusal?.title}
          watcherMessage={refusal?.message}
          canRejoin={refusal?.again}
          joiningLabel={retrying ? `Reconnecting\u{2026} (${tries} of ${REJOIN_WAITS_MS.length})` : undefined}
          canLeave={retrying}
          // A notice stays while its cause stays, with no close button: the reconnection ends by
          // itself and the paused audio with its button. The codec notice can stay for the whole
          // Cast, over a part of a narrow stage, so the viewer can close it.
          toasts={[
            ...(reconnecting
              ? [{ title: "Reconnecting\u{2026}", message: "The Cast does not arrive. It returns when the connection is back." }]
              : []),
            ...(signal === "undecodable" && !codecRead
              ? [
                  {
                    title: "This browser cannot play the video of this Cast yet",
                    message:
                      "The sound plays. The video starts by itself when the browser has its video decoder. You can also open this Cast in another browser.",
                    onDismiss: () => setCodecRead(true),
                  },
                ]
              : []),
            ...(live?.audioBlocked && state === "live"
              ? [
                  {
                    title: "Audio is paused",
                    message: "Your browser needs a click before the Cast can play sound.",
                    action: { label: "Enable audio", onClick: live.enableAudio },
                  },
                ]
              : []),
          ]}
        />
      </SitesLayout>
      {demoMessages && demoMessages.length === 0 && null}
    </div>
  );
}
