// One Cast connection to the scene's LiveKit room. Browser only: livekit-client is loaded on
// first use, so the server bundle never imports it.
import type { Room, Track, TrackPublication } from "livekit-client";

import type { CastCredentials } from "@/data/lib/catalyst/cast";

import type { CastSource, StageCandidate } from "./stage";

export type StageTrack = StageCandidate & {
  /** Stable key for rendering. */
  key: string;
  track: Track;
};

export type DeviceIds = { mic?: string; camera?: string };

export type CastSession = {
  /** Every subscribed or local video and audio track, live. */
  tracks: () => StageTrack[];
  participantCount: () => number;
  /** Fires on any change of tracks, participants or connection. */
  subscribe: (listener: () => void) => () => void;
  setMicrophone: (on: boolean) => Promise<void>;
  setCamera: (on: boolean) => Promise<void>;
  /** False when the browser refused or the viewer cancelled the picker. */
  setScreenShare: (on: boolean) => Promise<boolean>;
  isPublishing: (source: "microphone" | "camera" | "screen_share") => boolean;
  /** Retries audio playback after a user gesture, for browsers that blocked autoplay. */
  startAudio: () => Promise<void>;
  disconnected: () => boolean;
  disconnect: () => Promise<void>;
};

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

/** Connects with the token the gatekeeper minted. A streamer then publishes its microphone and
 * camera; a watcher's token is subscribe-only and publishes nothing. */
export async function joinCast(
  credentials: CastCredentials,
  opts: { publish: boolean; devices?: DeviceIds },
): Promise<CastSession> {
  const { Room, RoomEvent } = await import("livekit-client");
  const room: Room = new Room({ adaptiveStream: true, dynacast: true });
  const listeners = new Set<() => void>();
  const notify = () => listeners.forEach((l) => l());
  for (const ev of [
    RoomEvent.TrackSubscribed,
    RoomEvent.TrackUnsubscribed,
    RoomEvent.TrackMuted,
    RoomEvent.TrackUnmuted,
    RoomEvent.LocalTrackPublished,
    RoomEvent.LocalTrackUnpublished,
    RoomEvent.ParticipantConnected,
    RoomEvent.ParticipantDisconnected,
    RoomEvent.AudioPlaybackStatusChanged,
    RoomEvent.Disconnected,
  ]) {
    room.on(ev, notify);
  }

  await room.connect(credentials.url, credentials.token);
  if (opts.publish) {
    try {
      await room.localParticipant.setMicrophoneEnabled(
        true,
        opts.devices?.mic ? { deviceId: opts.devices.mic } : undefined,
      );
      await room.localParticipant.setCameraEnabled(
        true,
        opts.devices?.camera ? { deviceId: opts.devices.camera } : undefined,
      );
    } catch (err) {
      await room.disconnect();
      throw err;
    }
  }

  let closed = false;
  room.on(RoomEvent.Disconnected, () => {
    closed = true;
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
      return () => listeners.delete(listener);
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
        notify();
        return on ? pub !== undefined : true;
      } catch {
        notify();
        return false;
      }
    },
    isPublishing: (source) => {
      for (const pub of room.localParticipant.trackPublications.values()) {
        if (pub.source === source && pub.track && !pub.isMuted) return true;
      }
      return false;
    },
    startAudio: () => room.startAudio(),
    disconnected: () => closed,
    disconnect: async () => {
      closed = true;
      await room.disconnect();
      notify();
    },
  };
}
