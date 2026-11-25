// One Cast connection to the scene's LiveKit room. Browser only: livekit-client is loaded on
// first use, so the server bundle never imports it.
import type { ConnectionState, Room, RoomEvent as RoomEventEnum, Track, TrackPublication } from "livekit-client";

import type { CastCredentials } from "@/data/lib/catalyst/cast";

import { claimPresenter } from "./presenter";
import { isPerson, type CastSource, type StageCandidate } from "./stage";

/** Remote media that did not advance for this long is stalled (seen 6 to 8 s after its last frame). */
export const STALL_MS = 6_000;
const SAMPLE_MS = 2_000;
/** A reconnection that lasts this long ends the session, and the page joins again or says so.
 * LiveKit's own end can never come: Firefox closes a refused signal socket with no `open` and no
 * `error` event, livekit-client 2.22.4 waits for one of the two with its connect lock held, and
 * every later attempt waits for that lock. A watcher loses nothing in a new join. A streamer
 * loses its screen share and must press Rejoin, so it waits as long as LiveKit's own retries
 * are known to succeed (a 90 s outage). */
export const RECONNECT_LIMIT_MS = { watcher: 30_000, streamer: 120_000 };
/** livekit's `DisconnectReason` values the server chose: DUPLICATE_IDENTITY, PARTICIPANT_REMOVED,
 * ROOM_DELETED, ROOM_CLOSED. Any other end of the room is a lost connection. */
const SERVER_ENDED = [2, 4, 5, 10];
/** livekit's `SubscriptionError.SE_CODEC_UNSUPPORTED`. */
const CODEC_UNSUPPORTED = 1;
/** livekit's `ParticipantKind.INGRESS`: the RTMP ingress of a stream key. */
const INGRESS = 1;
/** An RTMP ingress publishes its sound first and its video 0 to 11.2 s later (40 pushes). One
 * with no video after this long is a Cast with no video. */
export const INGRESS_VIDEO_MS = 15_000;

export type StageTrack = StageCandidate & {
  /** Stable key for rendering. */
  key: string;
  track: Track;
};

export type CastSession = {
  /** Every subscribed or local track, live. */
  tracks: () => StageTrack[];
  /** This connection's LiveKit identity. */
  identity: () => string;
  /** The people in the room (`isPerson`), this page included one time. `own`: the identities of
   * the page's earlier connections, which the room lists until the server reports that they left
   * (seen 2.7 s after a watcher left). */
  participantCount: (own?: ReadonlySet<string>) => number;
  /** Another participant has an unmuted track: the Cast is live. */
  onAir: () => boolean;
  /** Another participant has an unmuted video, subscribed or not, or an RTMP ingress on air has
   * `INGRESS_VIDEO_MS` to publish its video: the stage must get a picture. */
  videoOnAir: () => boolean;
  /** The codec (`video/H264`) of a video on air that the server refused to send, because this
   * browser listed no decoder for it when it connected. The refusal holds for the connection,
   * also when the browser gets the decoder later: only a new join gets the video. */
  undecodable: () => string | null;
  /** What LiveKit restores: `signal` (media can still flow), or `all` (none does; also while the
   * server reports this page's media lost). Null while connected. */
  reconnecting: () => "signal" | "all" | null;
  /** Fires on any change of tracks, participants, audio playback or connection. */
  subscribe: (listener: () => void) => () => void;
  setMicrophone: (on: boolean) => Promise<void>;
  setCamera: (on: boolean) => Promise<void>;
  /** False when the browser refused or the viewer cancelled the picker. */
  setScreenShare: (on: boolean) => Promise<boolean>;
  isPublishing: (source: "microphone" | "camera" | "screen_share") => boolean;
  /** False while the browser blocks playback until a user gesture (Safari): the room says so and
   * a remote audio element is paused. The room alone can be wrong: it also says so when its
   * AudioContext is not running 200 ms after the join (Firefox), with every element playing. */
  canPlaybackAudio: () => boolean;
  /** Unblocks audio playback; call it from a click. */
  startAudio: () => Promise<void>;
  /** True once the room is gone: the server closed it, removed us, or reconnection gave up or
   * lasted its `RECONNECT_LIMIT_MS`. */
  disconnected: () => boolean;
  /** The server ended it on purpose (a kick, a ban, a deleted room): a new join needs a click. */
  removed: () => boolean;
  disconnect: () => Promise<void>;
};

/** What a streamer publishes on joining. */
export type Publish = { microphone: boolean; camera: boolean };

function candidate(pub: TrackPublication, identity: string, local: boolean, stalled: Set<Track>): StageTrack | null {
  const track = pub.track;
  if (!track) return null;
  return {
    key: `${identity}/${pub.trackSid}`,
    identity,
    source: pub.source as CastSource,
    muted: pub.isMuted,
    local,
    stalled: stalled.has(track),
    picture: track.attachedElements.some((element) => (element as HTMLVideoElement).videoWidth > 0),
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
    // `TrackUnsubscribed` comes while the publication still has its track; this one comes after.
    RoomEvent.TrackSubscriptionStatusChanged,
    RoomEvent.TrackMuted,
    RoomEvent.TrackUnmuted,
    RoomEvent.LocalTrackPublished,
    RoomEvent.LocalTrackUnpublished,
    RoomEvent.ParticipantConnected,
    RoomEvent.ParticipantDisconnected,
    RoomEvent.AudioPlaybackStatusChanged,
  ];
}

/** A count that grows while a remote track's media arrives: decoded frames, or audio packets. */
async function progressOf(track: Track): Promise<number> {
  let count = NaN;
  const stats = await track.getRTCStatsReport().catch(() => undefined);
  stats?.forEach((r) => {
    // A report with neither field is NaN, as no report is.
    if (r.type === "inbound-rtp") count = Number(r.framesDecoded ?? r.packetsReceived ?? NaN);
  });
  return count;
}

/** Wraps a connected room. Exported for tests, which pass a stand-in room. */
export function sessionOf(
  room: Room,
  RoomEvent: typeof RoomEventEnum,
  reconnectLimitMs = RECONNECT_LIMIT_MS.watcher,
): CastSession {
  const listeners = new Set<() => void>();
  const notify = () => listeners.forEach((l) => l());
  for (const ev of watchedEvents(RoomEvent)) room.on(ev, notify);
  // Only this page's own quality is read (`reconnecting`).
  room.on(RoomEvent.ConnectionQualityChanged, (_quality, participant) => {
    if (participant === room.localParticipant) notify();
  });
  const remotes = () => [...room.remoteParticipants.values()];
  const remotePubs = () => remotes().flatMap((p) => [...p.trackPublications.values()]);

  // Each remote track's count and when it last moved.
  const progress = new WeakMap<Track, { count: number; at: number }>();
  let stalled = new Set<Track>();
  const sample = async () => {
    const next = new Set<Track>();
    for (const { track, isMuted, isEnabled } of remotePubs()) {
      if (!track) continue;
      // A muted track, and one the page switched off (not on the stage, a hidden tab), sends
      // nothing by design. NaN equals nothing, so such a track is never stalled.
      const count = isMuted || !isEnabled ? NaN : await progressOf(track);
      const last = progress.get(track);
      if (last?.count !== count) progress.set(track, { count, at: Date.now() });
      else if (Date.now() - last.at >= STALL_MS) next.add(track);
    }
    if (next.size !== stalled.size || [...next].some((t) => !stalled.has(t))) {
      stalled = next;
      notify();
    }
  };
  const sampler = setInterval(() => void sample(), SAMPLE_MS);

  // An element that starts to play changes `canPlaybackAudio`, one that has its first frame
  // changes `picture`, and no room event says so.
  room.on(RoomEvent.TrackSubscribed, (track) =>
    track.on("elementAttached", (element) => {
      element.addEventListener("playing", notify);
      element.addEventListener("loadedmetadata", notify);
    }),
  );
  const audioElements = () =>
    remotePubs().flatMap((pub) => (pub.kind === "audio" ? (pub.track?.attachedElements ?? []) : []));

  const refused = new Set<TrackPublication>();
  room.on(RoomEvent.TrackSubscriptionFailed, (sid, participant, reason) => {
    const pub = participant.trackPublications.get(sid);
    if (pub && reason === CODEC_UNSUPPORTED) refused.add(pub);
    notify();
  });

  // When each RTMP ingress was first on air with no video. A read starts the wait: the room has
  // no event for an ingress that was there before this page.
  const soundOnlySince = new WeakMap<object, number>();
  const videoToCome = () =>
    remotes().some((p) => {
      if (p.kind !== INGRESS || ![...p.trackPublications.values()].some((pub) => !pub.isMuted)) return false;
      const since = soundOnlySince.get(p) ?? Date.now();
      if (!soundOnlySince.has(p)) {
        soundOnlySince.set(p, since);
        setTimeout(notify, INGRESS_VIDEO_MS);
      }
      return Date.now() - since < INGRESS_VIDEO_MS;
    });

  // livekit-client's `ConnectionState`, set before anyone is told: a listener reads
  // `disconnected() === true`.
  let link: `${ConnectionState}` = "connected";
  let limit: ReturnType<typeof setTimeout> | undefined;
  const settle = (to: `${ConnectionState}`) => {
    link = to;
    if (to === "reconnecting" || to === "signalReconnecting") {
      limit ??= setTimeout(() => void room.disconnect(), reconnectLimitMs);
    } else {
      clearTimeout(limit);
      limit = undefined;
    }
    if (to === "disconnected") clearInterval(sampler);
    notify();
  };
  // The room gives its reason with `Disconnected`, after the state change: the end settles there.
  let removed = false;
  room.on(RoomEvent.ConnectionStateChanged, (to) => {
    if (to !== "disconnected") settle(to);
  });
  room.on(RoomEvent.Disconnected, (reason) => {
    removed = SERVER_ENDED.includes(reason ?? 0);
    settle("disconnected");
  });

  return {
    tracks: () => {
      const out: StageTrack[] = [];
      const local = room.localParticipant;
      for (const pub of local.trackPublications.values()) {
        const c = candidate(pub, local.identity, true, stalled);
        if (c) out.push(c);
      }
      for (const p of room.remoteParticipants.values()) {
        for (const pub of p.trackPublications.values()) {
          const c = candidate(pub, p.identity, false, stalled);
          if (c) out.push(c);
        }
      }
      return out;
    },
    identity: () => room.localParticipant.identity,
    participantCount: (own) =>
      1 + remotes().filter((p) => !own?.has(p.identity) && isPerson(p.identity, p.trackPublications.size > 0)).length,
    onAir: () => remotePubs().some((pub) => !pub.isMuted),
    videoOnAir: () => remotePubs().some((pub) => !pub.isMuted && pub.kind === "video") || videoToCome(),
    // A refused track still comes for a moment: it is undecodable while it has none.
    undecodable: () => remotePubs().find((pub) => refused.has(pub) && !pub.track && !pub.isMuted)?.mimeType ?? null,
    reconnecting: () =>
      link === "signalReconnecting"
        ? "signal"
        : link === "reconnecting" || room.localParticipant.connectionQuality === "lost"
          ? "all"
          : null,
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
    canPlaybackAudio: () => room.canPlaybackAudio || !audioElements().some((element) => element.paused),
    startAudio: async () => {
      await room.startAudio();
      notify();
    },
    disconnected: () => link === "disconnected",
    removed: () => removed,
    disconnect: async () => {
      if (link !== "disconnected") await room.disconnect();
      settle("disconnected");
    },
  };
}

/** Calls `onLost` one time when the session's room is gone, with `removed()`. It reads the room
 * when it starts to listen: a room that ended between the join and this call had no listener.
 * Answers the unsubscribe. */
export function onRoomLost(session: CastSession, onLost: (removed: boolean) => void): () => void {
  let told = false;
  const read = () => {
    if (told || !session.disconnected()) return;
    told = true;
    onLost(session.removed());
  };
  const stop = session.subscribe(read);
  read();
  return stop;
}

/** Connects with the token the gatekeeper minted. A streamer then publishes what `publish` names
 * (a missing device is skipped, not fatal: a microphone and a screen share make a Cast) and
 * claims the room's presentation; a watcher's token is subscribe-only and publishes nothing. */
export async function joinCast(
  credentials: CastCredentials,
  opts: { publish?: Publish; signal?: AbortSignal } = {},
): Promise<CastSession> {
  const { Room, RoomEvent } = await import("livekit-client");
  const room: Room = new Room({ adaptiveStream: true, dynacast: true });
  const session = sessionOf(room, RoomEvent, RECONNECT_LIMIT_MS[opts.publish ? "streamer" : "watcher"]);
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
    claimPresenter(room, RoomEvent);
  }
  return session;
}

/** This browser lists a decoder for `mimeType`. A new Firefox profile lists H.264 only after it
 * has fetched the codec, in about its first minute. */
export function canDecode(mimeType: string): boolean {
  const codecs = globalThis.RTCRtpReceiver?.getCapabilities?.("video")?.codecs ?? [];
  return codecs.some((codec) => codec.mimeType.toLowerCase() === mimeType.toLowerCase());
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
