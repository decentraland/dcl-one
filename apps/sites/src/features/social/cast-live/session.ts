// One Cast connection to the scene's LiveKit room. Browser only: livekit-client is loaded on
// first use, so the server bundle never imports it.
import type { Room, RoomEvent as RoomEventEnum, Track, TrackPublication } from "livekit-client";

import type { CastCredentials } from "@/data/lib/catalyst/cast";

import type { CastSource, StageCandidate } from "./stage";

export type StageTrack = StageCandidate & {
  /** Stable key for rendering. */
  key: string;
  track: Track;
};

export type CastSession = {
  /** Every subscribed or local track, live. */
  tracks: () => StageTrack[];
  participantCount: () => number;
  /** Fires on any change of tracks, participants, audio playback or connection. */
  subscribe: (listener: () => void) => () => void;
  setMicrophone: (on: boolean) => Promise<void>;
  setCamera: (on: boolean) => Promise<void>;
  /** False when the browser refused or the viewer cancelled the picker. */
  setScreenShare: (on: boolean) => Promise<boolean>;
  isPublishing: (source: "microphone" | "camera" | "screen_share") => boolean;
  /** False while the browser blocks playback until a user gesture (Safari). */
  canPlaybackAudio: () => boolean;
  /** Unblocks audio playback; call it from a click. */
  startAudio: () => Promise<void>;
  /** True once the room is gone: the server closed it, removed us, or reconnection gave up. */
  disconnected: () => boolean;
  disconnect: () => Promise<void>;
};

/** What a streamer publishes on joining. */
export type Publish = { microphone: boolean; camera: boolean };

function candidate(pub: TrackPublication, identity: string, local: boolean): StageTrack | null {
  const track = pub.track;
  if (!track) return null;
  return {
    key: `${identity}/${pub.trackSid}`,
    identity,
    source: pub.source as CastSource,
    muted: pub.isMuted,
    local,
    track,
  };
}

/** The events that change what a Cast page shows. */
function watchedEvents(RoomEvent: typeof RoomEventEnum): RoomEventEnum[] {
  return [
    RoomEvent.TrackPublished,
    RoomEvent.TrackUnpublished,
    RoomEvent.TrackSubscribed,
    RoomEvent.TrackUnsubscribed,
    RoomEvent.TrackMuted,
    RoomEvent.TrackUnmuted,
    RoomEvent.LocalTrackPublished,
    RoomEvent.LocalTrackUnpublished,
    RoomEvent.ParticipantConnected,
    RoomEvent.ParticipantDisconnected,
    RoomEvent.AudioPlaybackStatusChanged,
  ];
}

/** Wraps a connected room. Exported for tests, which pass a stand-in room. */
export function sessionOf(room: Room, RoomEvent: typeof RoomEventEnum): CastSession {
  const listeners = new Set<() => void>();
  const notify = () => listeners.forEach((l) => l());
  let closed = false;
  for (const ev of watchedEvents(RoomEvent)) room.on(ev, notify);
  // The flag is set before anyone is told, so a listener reads `disconnected() === true`.
  room.on(RoomEvent.Disconnected, () => {
    closed = true;
    notify();
  });

  return {
    tracks: () => {
      const out: StageTrack[] = [];
      const local = room.localParticipant;
      for (const pub of local.trackPublications.values()) {
        const c = candidate(pub, local.identity, true);
        if (c) out.push(c);
      }
      for (const p of room.remoteParticipants.values()) {
        for (const pub of p.trackPublications.values()) {
          const c = candidate(pub, p.identity, false);
          if (c) out.push(c);
        }
      }
      return out;
    },
    participantCount: () => room.remoteParticipants.size + 1,
    subscribe: (listener) => {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    setMicrophone: async (on) => {
      await room.localParticipant.setMicrophoneEnabled(on);
      notify();
    },
    setCamera: async (on) => {
      await room.localParticipant.setCameraEnabled(on);
      notify();
    },
    setScreenShare: async (on) => {
      try {
        const pub = await room.localParticipant.setScreenShareEnabled(on, { audio: true });
        return on ? pub !== undefined : true;
      } catch {
        return false;
      } finally {
        notify();
      }
    },
    isPublishing: (source) => {
      for (const pub of room.localParticipant.trackPublications.values()) {
        if (pub.source === source && pub.track && !pub.isMuted) return true;
      }
      return false;
    },
    canPlaybackAudio: () => room.canPlaybackAudio,
    startAudio: async () => {
      await room.startAudio();
      notify();
    },
    disconnected: () => closed,
    disconnect: async () => {
      if (!closed) await room.disconnect();
      closed = true;
      notify();
    },
  };
}

/** Connects with the token the gatekeeper minted. A streamer then publishes what `publish` names
 * (a missing device is skipped, not fatal: a microphone and a screen share make a Cast); a
 * watcher's token is subscribe-only and publishes nothing. */
export async function joinCast(
  credentials: CastCredentials,
  opts: { publish?: Publish; signal?: AbortSignal } = {},
): Promise<CastSession> {
  const { Room, RoomEvent } = await import("livekit-client");
  const room: Room = new Room({ adaptiveStream: true, dynacast: true });
  const session = sessionOf(room, RoomEvent);
  await room.connect(credentials.url, credentials.token);
  if (opts.signal?.aborted) {
    await session.disconnect();
    throw new DOMException("Join cancelled", "AbortError");
  }
  const publish = opts.publish;
  if (publish) {
    const enabled = await Promise.allSettled([
      publish.microphone ? room.localParticipant.setMicrophoneEnabled(true) : Promise.resolve(),
      publish.camera ? room.localParticipant.setCameraEnabled(true) : Promise.resolve(),
    ]);
    if (enabled.every((r) => r.status === "rejected")) {
      await session.disconnect();
      throw (enabled[0] as PromiseRejectedResult).reason;
    }
  }
  return session;
}

/** Which of the camera and microphone this browser may open, each asked for on its own so a
 * machine without a camera still streams its microphone and screen. */
export async function probeDevices(): Promise<Publish> {
  const md = typeof navigator !== "undefined" ? navigator.mediaDevices : undefined;
  if (!md?.getUserMedia) return { microphone: false, camera: false };
  const probe = async (constraints: MediaStreamConstraints) => {
    try {
      const stream = await md.getUserMedia(constraints);
      stream.getTracks().forEach((t) => t.stop());
      return true;
    } catch {
      return false;
    }
  };
  const microphone = await probe({ audio: true });
  const camera = await probe({ video: true });
  return { microphone, camera };
}
