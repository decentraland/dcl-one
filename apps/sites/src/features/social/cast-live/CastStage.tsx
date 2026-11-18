import { useCallback, useEffect, useRef, useSyncExternalStore } from "react";

import type { CastSession, StageTrack } from "./session";
import { pickWatcherStage, streamerStage } from "./stage";

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
    (onChange: () => void) =>
      session
        ? session.subscribe(() => {
            version.current += 1;
            onChange();
          })
        : () => {},
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

function TrackVideo({ track, mirror }: { track: StageTrack; mirror: boolean }) {
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
      style={mirror ? { transform: "scaleX(-1)" } : undefined}
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

/** A watcher's view: the one track `pickWatcherStage` names, and the room's audio. */
export function WatcherStage({ tracks, muted }: { tracks: StageTrack[]; muted: boolean }) {
  const shown = pickWatcherStage(tracks);
  return (
    <>
      {shown ? <TrackVideo key={shown.key} track={shown} mirror={false} /> : null}
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
