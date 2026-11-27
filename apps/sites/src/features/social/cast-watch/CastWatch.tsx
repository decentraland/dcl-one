import { useCallback, useEffect, useState, type ReactNode } from "react";

import { Button, StCastWatcher } from "@dcl/one-ui";

import { track, type TrackContext } from "@/core/lib/telemetry/track";
import type { WatchResult } from "@/data/lib/catalyst/landings/cast-watcher";
import SitesLayout from "../../components/chrome/SitesLayout";

const JOIN_CLASS = "stcastwatcher__join";
const LEAVE_CLASS = "stcastwatcher__leave";
const ICON_BTN_CLASS = "stcastwatcher__iconbtn";
const SIDE_CLOSE_CLASS = "stcastwatcher__sideclose";

const ICON_MUTE = 0;
const ICON_CHAT = 1;

type WatcherState = "onboarding" | "joining" | "live" | "waiting";

/** Why a real join did not go live: the watcher page shows it in place of the stream. */
export type WatchRefusal = { kind: "waiting" | "error"; title: string; message: string };

/** A join's outcome: connected to `roomId`, or refused. */
export type WatchJoin = { roomId: string; refused?: undefined } | { refused: WatchRefusal; roomId?: undefined };

/** A real LiveKit session behind the watcher page; the story page leaves it out. */
export type WatchLive = {
  join: () => Promise<WatchJoin>;
  chooseScene?: {
    scenes: Array<{ base: string; title: string }>;
    onChoose: (base: string) => void;
  };
  leave: () => void;
  stage: (muted: boolean) => ReactNode;
  participantCount: number;
  /** Calls `onLost` when the room goes away while watching; answers the unsubscribe. */
  bindLost: (onLost: (why: WatchRefusal) => void) => () => void;
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

  const roomId = watch.credentials?.roomId ?? "";
  const placeName = watch.placeName;

  const join = useCallback(() => {
    setState("joining");
    setRefusal(null);
    if (!live) {
      onTrack(
        "cast_watch_joined",
        { room_id: roomId, place_name: placeName, stub: true },
        trackCtx,
      );
      setTimeout(() => setState("live"), 600);
      return;
    }
    live.join().then(
      (joined) => {
        if (joined.refused) {
          const refused = joined.refused;
          onTrack("cast_watch_refused", { kind: refused.kind, reason: refused.title }, trackCtx);
          setRefusal(refused);
          setState(refused.kind === "waiting" ? "waiting" : "onboarding");
          return;
        }
        onTrack(
          "cast_watch_joined",
          { room_id: joined.roomId, place_name: placeName, stub: false },
          trackCtx,
        );
        setState("live");
      },
      (err: unknown) => {
        const reason = err instanceof Error ? err.message : "join failed";
        onTrack("cast_watch_refused", { kind: "error", reason }, trackCtx);
        setRefusal({ kind: "error", title: "Could not join the cast", message: "Try again in a moment." });
        setState("onboarding");
      },
    );
  }, [live, onTrack, roomId, placeName, trackCtx]);

  const bindLost = live?.bindLost;
  useEffect(
    () =>
      bindLost?.((why) => {
        onTrack("cast_watch_lost", { reason: why.title }, trackCtx);
        setSidebarOpen(false);
        setRefusal(why);
        setState("waiting");
      }),
    [bindLost, onTrack, trackCtx],
  );

  const leave = useCallback(() => {
    onTrack("cast_watch_left", {}, trackCtx);
    live?.leave();
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
    toasts?: Array<{ title: ReactNode; message: ReactNode; action?: { label?: string; onClick?: () => void } }>;
  }>;

  const chooseScene = live?.chooseScene;
  const shownState = chooseScene ? "waiting" : state;
  const chatFooter = `Jump into ${placeName} in Decentraland to participate in the chat.`;

  return (
    <div
      className="cast-watch"
      data-state={shownState}
      data-status={watch.status}
      data-location={watch.location}
      onClickCapture={onClickCapture}
    >
      <SitesLayout active="legal">
        <Watcher
          streamName={placeName}
          state={shownState}
          sidebarOpen={state === "live" && sidebarOpen}
          isTabMuted={isTabMuted}
          chatFooter={chatFooter}
          participantCount={live ? live.participantCount : participantCount}
          unreadCount={state === "live" && !sidebarOpen && !live ? unreadCount : 0}
          stage={live && state === "live" ? live.stage(isTabMuted) : undefined}
          messages={live ? [] : undefined}
          watcherTitle={chooseScene ? "Choose a scene to watch" : refusal?.kind === "waiting" ? refusal.title : undefined}
          watcherMessage={chooseScene ? (
            <>
              This world has several scenes. Pick the one that is casting.
              <br />
              {chooseScene.scenes.map((scene) => (
                <Button key={scene.base} type="button" variant="secondary" size="sm" className="cast-watch__scene" onClick={() => {
                  setRefusal(null);
                  setState("onboarding");
                  chooseScene.onChoose(scene.base);
                }}>
                  {scene.title} {scene.base}
                </Button>
              ))}
            </>
          ) : refusal?.kind === "waiting" ? refusal.message : undefined}
          toasts={[
            ...(refusal?.kind === "error"
              ? [{ title: refusal.title, message: refusal.message, action: { label: "Retry", onClick: join } }]
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
