import { useCallback, useEffect, useRef, useState, useSyncExternalStore } from "react";

import type { CastSession, StageTrack } from "./session";
import { streamerStage, watcherStage } from "./stage";

const NONE: StageTrack[] = [];

/** The session's tracks, re-read on every room event. */
export function useCastTracks(session: CastSession | null): StageTrack[] {
  const cache = useRef<{ session: CastSession | null; version: number; tracks: StageTrack[] }>({
    session: null,
    version: -1,
    tracks: NONE,
  });
  const version = useRef(0);
  const subscribe = useCallback(
    (onChange: () => void) => {
      if (!session) return () => {};
      const changed = () => {
        version.current += 1;
        onChange();
      };
      const stop = session.subscribe(changed);
      // A room event between the render and this call had no listener: the room is read again.
      changed();
      return stop;
    },
    [session],
  );
  const snapshot = useCallback(() => {
    if (!session) return NONE;
    const c = cache.current;
    if (c.session !== session || c.version !== version.current) {
      cache.current = { session, version: version.current, tracks: session.tracks() };
    }
    return cache.current.tracks;
  }, [session]);
  return useSyncExternalStore(subscribe, snapshot, () => NONE);
}

/** `unseen`: drawn, so that its media flows and its first frame is known, and not visible. */
function TrackVideo({ track, mirror, unseen }: { track: StageTrack; mirror: boolean; unseen?: boolean }) {
  const ref = useRef<HTMLVideoElement>(null);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    track.track.attach(el);
    return () => {
      track.track.detach(el);
    };
  }, [track.track]);
  return (
    <video
      ref={ref}
      autoPlay
      playsInline
      muted
      data-source={track.source}
      style={unseen ? { position: "absolute", inset: 0, opacity: 0 } : mirror ? { transform: "scaleX(-1)" } : undefined}
    />
  );
}

function TrackAudio({ track, muted }: { track: StageTrack; muted: boolean }) {
  const ref = useRef<HTMLAudioElement>(null);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    track.track.attach(el);
    return () => {
      track.track.detach(el);
    };
  }, [track.track]);
  return <audio ref={ref} autoPlay muted={muted} />;
}

/** Everyone else's audio in the room; the page's own microphone is never played back. */
function RoomAudio({ tracks, muted }: { tracks: StageTrack[]; muted: boolean }) {
  return (
    <>
      {tracks
        .filter((t) => !t.local && (t.source === "microphone" || t.source === "screen_share_audio"))
        .map((t) => (
          <TrackAudio key={t.key} track={t} muted={muted} />
        ))}
    </>
  );
}

export type Stage = ReturnType<typeof watcherStage<StageTrack>>;

/** `watcherStage` of the session's tracks; it remembers the track on the stage. */
export function useWatcherStage(tracks: StageTrack[]): Stage {
  const [owner, setOwner] = useState<string | null>(null);
  const stage = watcherStage(tracks, owner);
  const key = stage.shown?.key ?? null;
  if (key !== owner) setOwner(key);
  return stage;
}

/** A watcher's view: the stage's track, the one that waits for its first picture, and the room's
 * audio. A track keeps its element when it takes the stage, so its picture stays. */
export function WatcherStage({ tracks, stage, muted }: { tracks: StageTrack[]; stage: Stage; muted: boolean }) {
  return (
    <>
      {[stage.shown, stage.next].map(
        (t) => t && <TrackVideo key={t.key} track={t} mirror={false} unseen={t === stage.next} />,
      )}
      <RoomAudio tracks={tracks} muted={muted} />
    </>
  );
}

/** A streamer's view of what it publishes, and the room's audio. */
export function StreamerStage({ tracks, muted }: { tracks: StageTrack[]; muted: boolean }) {
  return (
    <>
      {streamerStage(tracks).map((t) => (
        <TrackVideo key={t.key} track={t} mirror={t.source === "camera"} />
      ))}
      <RoomAudio tracks={tracks} muted={muted} />
    </>
  );
}
