import { afterEach, describe, expect, it, vi } from "vitest";
import type { Room, RoomEvent as RoomEventEnum } from "livekit-client";

import { INGRESS_VIDEO_MS, RECONNECT_LIMIT_MS, STALL_MS, canDecode, onRoomLost, sessionOf } from "./session";

// The string values of livekit-client's RoomEvent members this module listens to.
const RoomEvent = {
  TrackPublished: "trackPublished",
  TrackUnpublished: "trackUnpublished",
  TrackSubscribed: "trackSubscribed",
  TrackUnsubscribed: "trackUnsubscribed",
  TrackSubscriptionStatusChanged: "trackSubscriptionStatusChanged",
  TrackSubscriptionFailed: "trackSubscriptionFailed",
  TrackMuted: "trackMuted",
  TrackUnmuted: "trackUnmuted",
  LocalTrackPublished: "localTrackPublished",
  LocalTrackUnpublished: "localTrackUnpublished",
  ParticipantConnected: "participantConnected",
  ParticipantDisconnected: "participantDisconnected",
  AudioPlaybackStatusChanged: "audioPlaybackChanged",
  ConnectionStateChanged: "connectionStateChanged",
  Disconnected: "disconnected",
  ConnectionQualityChanged: "connectionQualityChanged",
} as unknown as typeof RoomEventEnum;

type Pub = { source: string; trackSid: string; isMuted: boolean; track: object | undefined; isEnabled?: boolean; kind?: string; mimeType?: string };

function standInRoom() {
  const handlers = new Map<string, Array<(...args: unknown[]) => void>>();
  const local = { identity: "stream:p:1", connectionQuality: "excellent", trackPublications: new Map<string, Pub>() } as {
    identity: string;
    connectionQuality: string;
    trackPublications: Map<string, Pub>;
    setScreenShareEnabled: ReturnType<typeof vi.fn>;
  };
  const room = {
    canPlaybackAudio: true,
    state: "connected",
    localParticipant: local,
    remoteParticipants: new Map<string, { identity: string; kind?: number; trackPublications: Map<string, Pub> }>(),
    on: (ev: string, fn: (...args: unknown[]) => void) => {
      handlers.set(ev, [...(handlers.get(ev) ?? []), fn]);
      return room;
    },
    emit: (ev: string, ...args: unknown[]) => handlers.get(ev)?.forEach((fn) => fn(...args)),
    disconnect: vi.fn(async () => {}),
    startAudio: vi.fn(async () => {
      room.canPlaybackAudio = true;
    }),
  };
  local.setScreenShareEnabled = vi.fn();
  return room;
}

/** A remote camera whose decoded-frame counter the test moves. */
function remoteCamera(room: ReturnType<typeof standInRoom>, pub: Partial<Pub> = {}) {
  const media = { frames: 0 };
  const track = {
    attachedElements: [] as Array<{ videoWidth: number }>,
    getRTCStatsReport: async () => new Map([["in", { type: "inbound-rtp", framesDecoded: media.frames }]]),
  };
  const all: Pub = { source: "camera", kind: "video", trackSid: "TR_v", isMuted: false, isEnabled: true, track, ...pub };
  room.remoteParticipants.set("s", { identity: "stream:place:uuid", trackPublications: new Map([["v", all]]) });
  return Object.assign(media, { track });
}

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe("sessionOf", () => {
  it("counts people: itself, watchers and publishing streamers, not the bot, the ingress, the scene's server or a silent stream connection", () => {
    const room = standInRoom();
    const pubs = new Map([["a", { source: "camera", trackSid: "TR_a", isMuted: false, track: {} }]]);
    for (const [identity, trackPublications] of [
      ["presentation-bot:room:access:uuid", pubs],
      ["authoritative-server", pubs],
      ["0b9d2f6e-1c57-4c1e-9a53-0d7c1c2a4e11-streamer", pubs],
      ["stream:place:access:control", new Map<string, Pub>()],
      ["watch:room:uuid", new Map<string, Pub>()],
      ["stream:place:access:speaker", pubs],
    ] as const) {
      room.remoteParticipants.set(identity, { identity, trackPublications });
    }
    expect(sessionOf(room as unknown as Room, RoomEvent).participantCount()).toBe(3);
  });

  it("counts the page one time while the room of a new join lists the page's earlier connection", () => {
    // The page joined again: the server reports 2.7 s later that the first connection left.
    const room = standInRoom();
    room.localParticipant.identity = "watch:room:second";
    for (const identity of ["watch:room:other", "0b9d2f6e-1c57-4c1e-9a53-0d7c1c2a4e11-streamer", "watch:room:first"]) {
      room.remoteParticipants.set(identity, { identity, trackPublications: new Map<string, Pub>() });
    }
    const s = sessionOf(room as unknown as Room, RoomEvent);
    const own = new Set(["watch:room:first"]);
    expect(s.participantCount(own)).toBe(2);
    room.remoteParticipants.delete("watch:room:first");
    room.emit(RoomEvent.ParticipantDisconnected);
    expect(s.participantCount(own)).toBe(2);
    expect(s.identity()).toBe("watch:room:second");
  });

  it("is on air only while another participant has an unmuted track", () => {
    const room = standInRoom();
    const pub = (isMuted: boolean): Pub => ({ source: "camera", trackSid: "TR_a", isMuted, track: undefined });
    const s = sessionOf(room as unknown as Room, RoomEvent);
    const theirs = new Map([["a", pub(true)]]);
    room.localParticipant.trackPublications.set("a", pub(false));
    room.remoteParticipants.set("s", { identity: "stream:place:access:uuid", trackPublications: theirs });
    expect(s.onAir()).toBe(false);
    theirs.set("a", pub(false));
    expect(s.onAir()).toBe(true);
  });

  /** An RTMP ingress (livekit's `ParticipantKind.INGRESS`) that joins and publishes its sound. */
  function ingressWithSound(room: ReturnType<typeof standInRoom>) {
    const pubs = new Map<string, Pub>();
    room.remoteParticipants.set("i", { identity: "0b9d2f6e-1c57-4c1e-9a53-0d7c1c2a4e11-streamer", kind: 1, trackPublications: pubs });
    room.emit(RoomEvent.ParticipantConnected);
    vi.advanceTimersByTime(25);
    pubs.set("a", { source: "microphone", kind: "audio", trackSid: "TR_a", isMuted: false, track: {} });
    room.emit(RoomEvent.TrackPublished);
    return pubs;
  }

  it("must get a picture from an RTMP ingress on air whose video is published 4.6 s after its sound", () => {
    vi.useFakeTimers();
    const room = standInRoom();
    const s = sessionOf(room as unknown as Room, RoomEvent);
    const pubs = ingressWithSound(room);
    expect([s.onAir(), s.videoOnAir()]).toEqual([true, true]);
    vi.advanceTimersByTime(4_590);
    expect(s.videoOnAir()).toBe(true);
    pubs.set("v", { source: "camera", kind: "video", trackSid: "TR_v", isMuted: false, track: undefined });
    room.emit(RoomEvent.TrackPublished);
    vi.advanceTimersByTime(INGRESS_VIDEO_MS);
    expect(s.videoOnAir()).toBe(true);
  });

  it("takes an RTMP ingress with no video after INGRESS_VIDEO_MS for a Cast with no video, and says so", () => {
    vi.useFakeTimers();
    const room = standInRoom();
    const s = sessionOf(room as unknown as Room, RoomEvent);
    ingressWithSound(room);
    expect(s.videoOnAir()).toBe(true);
    const heard = vi.fn();
    s.subscribe(heard);
    vi.advanceTimersByTime(INGRESS_VIDEO_MS - 1);
    expect([s.videoOnAir(), heard.mock.calls.length]).toEqual([true, 0]);
    vi.advanceTimersByTime(1);
    expect([s.onAir(), s.videoOnAir(), heard.mock.calls.length]).toEqual([true, false, 1]);
  });

  it("waits for no video of a streamer who publishes a microphone and no camera", () => {
    const room = standInRoom();
    const microphone: Pub = { source: "microphone", kind: "audio", trackSid: "TR_a", isMuted: false, track: {} };
    room.remoteParticipants.set("s", { identity: "stream:place:access:uuid", kind: 0, trackPublications: new Map([["a", microphone]]) });
    const s = sessionOf(room as unknown as Room, RoomEvent);
    expect([s.onAir(), s.videoOnAir()]).toEqual([true, false]);
  });

  it("lists local and remote tracks with their source, mute and identity, skipping unsubscribed ones", () => {
    const room = standInRoom();
    const track = { attachedElements: [] };
    room.localParticipant.trackPublications.set("a", { source: "camera", trackSid: "TR_a", isMuted: false, track });
    room.remoteParticipants.set("w", {
      identity: "0xabc",
      trackPublications: new Map([
        ["b", { source: "microphone", trackSid: "TR_b", isMuted: true, track }],
        ["c", { source: "camera", trackSid: "TR_c", isMuted: false, track: undefined }],
      ]),
    });
    const s = sessionOf(room as unknown as Room, RoomEvent);
    expect(s.tracks().map(({ key, source, muted, local }) => ({ key, source, muted, local }))).toEqual([
      { key: "stream:p:1/TR_a", source: "camera", muted: false, local: true },
      { key: "0xabc/TR_b", source: "microphone", muted: true, local: false },
    ]);
    expect(s.participantCount()).toBe(2);
    expect(s.isPublishing("camera")).toBe(true);
    expect(s.isPublishing("screen_share")).toBe(false);
  });

  it("has a video on air while another participant has an unmuted one, subscribed or not", () => {
    const room = standInRoom();
    const s = sessionOf(room as unknown as Room, RoomEvent);
    const pub: Pub = { source: "screen_share", kind: "video", trackSid: "TR_v", isMuted: true, track: undefined };
    room.remoteParticipants.set("s", { identity: "stream:place:uuid", trackPublications: new Map([["v", pub], ["a", { ...pub, kind: "audio", isMuted: false }]]) });
    expect(s.videoOnAir()).toBe(false);
    pub.isMuted = false;
    expect(s.videoOnAir()).toBe(true);
  });

  it("says a track has a picture once an element it is attached to shows a frame, and tells its listeners", () => {
    const room = standInRoom();
    const { track } = remoteCamera(room);
    const s = sessionOf(room as unknown as Room, RoomEvent);
    const on = vi.fn();
    room.emit("trackSubscribed", { on });
    const listener = vi.fn();
    s.subscribe(listener);
    const element = { videoWidth: 0, addEventListener: vi.fn() };
    track.attachedElements.push(element);
    on.mock.calls[0]![1](element);
    expect(s.tracks()[0]!.picture).toBe(false);

    element.videoWidth = 640;
    element.addEventListener.mock.calls.find(([event]) => event === "loadedmetadata")![1]();
    expect(listener).toHaveBeenCalledTimes(1);
    expect(s.tracks()[0]!.picture).toBe(true);
  });

  it("names the codec of a video that the server refused for it, while the video is on air with no track", () => {
    const room = standInRoom();
    const s = sessionOf(room as unknown as Room, RoomEvent);
    const listener = vi.fn();
    s.subscribe(listener);
    const pub: Pub = { source: "camera", kind: "video", mimeType: "video/H264", trackSid: "TR_v", isMuted: false, track: undefined };
    const ingress = { identity: "uuid-streamer", trackPublications: new Map([["TR_v", pub]]) };
    room.remoteParticipants.set("i", ingress);
    // livekit's SubscriptionError: 2 = the track was not found, 1 = no codec of it for this browser.
    room.emit("trackSubscriptionFailed", "TR_v", ingress, 2);
    expect(s.undecodable()).toBeNull();
    room.emit("trackSubscriptionFailed", "TR_v", ingress, 1);
    expect(s.undecodable()).toBe("video/H264");
    expect(listener).toHaveBeenCalledTimes(2);

    // The refused track still comes, and goes at once: only the status change tells it is gone.
    pub.track = {};
    expect(s.undecodable()).toBeNull();
    pub.track = undefined;
    room.emit("trackSubscriptionStatusChanged");
    expect(listener).toHaveBeenCalledTimes(3);
    expect(s.undecodable()).toBe("video/H264");
    pub.isMuted = true;
    expect(s.undecodable()).toBeNull();
  });

  it("is already disconnected when its listeners hear the room close, and a later leave does not call the room", async () => {
    const room = standInRoom();
    const s = sessionOf(room as unknown as Room, RoomEvent);
    const seen: boolean[] = [];
    s.subscribe(() => seen.push(s.disconnected()));
    room.emit("connectionStateChanged", "disconnected");
    room.emit("disconnected");
    expect(seen).toEqual([true]);
    await s.disconnect();
    expect(room.disconnect).not.toHaveBeenCalled();
  });

  it("tells of a lost room one time, also of a room that was gone before it listened", () => {
    for (const early of [true, false]) {
      const room = standInRoom();
      const s = sessionOf(room as unknown as Room, RoomEvent);
      const end = () => {
        room.emit("connectionStateChanged", "disconnected");
        room.emit("disconnected", 4);
      };
      const onLost = vi.fn();
      if (early) end();
      const stop = onRoomLost(s, onLost);
      room.emit("participantConnected");
      expect(onLost).toHaveBeenCalledTimes(early ? 1 : 0);
      if (!early) end();
      room.emit("participantConnected");
      expect(onLost.mock.calls).toEqual([[true]]);
      stop();
    }
  });

  it("tells a removal by the server from a lost connection when its listeners hear the room close", () => {
    // livekit's DisconnectReason: none when its retries end, 1 at the page's limit, 3 a server that stops.
    const reasons = [[undefined, false], [1, false], [3, false], [2, true], [4, true], [5, true], [10, true]] as const;
    for (const [reason, removed] of reasons) {
      const room = standInRoom();
      const s = sessionOf(room as unknown as Room, RoomEvent);
      const seen: boolean[] = [];
      s.subscribe(() => seen.push(s.disconnected() && s.removed()));
      room.emit("connectionStateChanged", "disconnected");
      room.emit("disconnected", reason);
      expect(seen).toEqual([removed]);
    }
  });

});

/** A remote microphone attached to one audio element. */
function remoteMicrophone(room: ReturnType<typeof standInRoom>) {
  const element = { paused: true, addEventListener: vi.fn() };
  const track = { attachedElements: [element], on: vi.fn() };
  const pub: Pub = { source: "microphone", kind: "audio", trackSid: "TR_a", isMuted: false, track };
  room.remoteParticipants.set("s", { identity: "stream:place:uuid", trackPublications: new Map([["a", pub]]) });
  return { element, track };
}

describe("sessionOf audio", () => {
  it("reports blocked audio while an audio element is paused, and unblocks it on request", async () => {
    const room = standInRoom();
    room.canPlaybackAudio = false;
    remoteMicrophone(room);
    const s = sessionOf(room as unknown as Room, RoomEvent);
    const listener = vi.fn();
    s.subscribe(listener);
    expect(s.canPlaybackAudio()).toBe(false);
    await s.startAudio();
    expect(s.canPlaybackAudio()).toBe(true);
    expect(listener).toHaveBeenCalled();
  });

  it("follows the audio elements, not the room, and tells its listeners when one starts to play", () => {
    const room = standInRoom();
    room.canPlaybackAudio = false;
    const s = sessionOf(room as unknown as Room, RoomEvent);
    expect(s.canPlaybackAudio()).toBe(true);

    const { element, track } = remoteMicrophone(room);
    room.emit("trackSubscribed", track);
    expect(track.on.mock.calls[0]![0]).toBe("elementAttached");
    track.on.mock.calls[0]![1](element);
    expect(s.canPlaybackAudio()).toBe(false);

    const listener = vi.fn();
    s.subscribe(listener);
    element.paused = false;
    expect(element.addEventListener.mock.calls[0]![0]).toBe("playing");
    element.addEventListener.mock.calls[0]![1]();
    expect(listener).toHaveBeenCalledTimes(1);
    expect(s.canPlaybackAudio()).toBe(true);
  });
});

describe("sessionOf", () => {

  it("answers false for a cancelled screen picker and true once a share publishes", async () => {
    const room = standInRoom();
    const s = sessionOf(room as unknown as Room, RoomEvent);
    room.localParticipant.setScreenShareEnabled.mockRejectedValueOnce(new Error("NotAllowedError"));
    expect(await s.setScreenShare(true)).toBe(false);
    room.localParticipant.setScreenShareEnabled.mockResolvedValueOnce({});
    expect(await s.setScreenShare(true)).toBe(true);
    room.localParticipant.setScreenShareEnabled.mockResolvedValueOnce(undefined);
    expect(await s.setScreenShare(false)).toBe(true);
  });

  it("is reconnecting while LiveKit restores the signal or the connection, and while the server lost this page's media", () => {
    const room = standInRoom();
    const s = sessionOf(room as unknown as Room, RoomEvent);
    const seen: Array<string | null> = [];
    s.subscribe(() => seen.push(s.reconnecting()));
    for (const state of ["signalReconnecting", "connected", "reconnecting", "connected"]) {
      room.emit("connectionStateChanged", state);
    }
    room.localParticipant.connectionQuality = "lost";
    room.emit("connectionQualityChanged", "lost", room.localParticipant);
    expect(seen).toEqual(["signal", null, "all", null, "all"]);
  });

  it("does not tell its listeners of the connection quality of another participant", () => {
    const room = standInRoom();
    const other = { identity: "watch:room:uuid", trackPublications: new Map<string, Pub>() };
    room.remoteParticipants.set("w", other);
    const s = sessionOf(room as unknown as Room, RoomEvent);
    const listener = vi.fn();
    s.subscribe(listener);
    room.emit("connectionQualityChanged", "poor", other);
    expect(listener).not.toHaveBeenCalled();
    room.emit("connectionQualityChanged", "poor", room.localParticipant);
    expect(listener).toHaveBeenCalledTimes(1);
  });

  it("calls a remote track stalled when its media stops for STALL_MS, and fresh when it advances again", async () => {
    vi.useFakeTimers();
    const room = standInRoom();
    const media = remoteCamera(room);
    const s = sessionOf(room as unknown as Room, RoomEvent);
    const listener = vi.fn();
    s.subscribe(listener);
    const stalled = () => s.tracks()[0]!.stalled;

    await vi.advanceTimersByTimeAsync(STALL_MS);
    expect(stalled()).toBe(false);
    await vi.advanceTimersByTimeAsync(2_000);
    expect(stalled()).toBe(true);
    expect(listener).toHaveBeenCalledTimes(1);

    media.frames = 30;
    await vi.advanceTimersByTimeAsync(2_000);
    expect(stalled()).toBe(false);
    expect(listener).toHaveBeenCalledTimes(2);
    await s.disconnect();
  });

  it("reads a stats report with no frame or packet count as no report, and still samples the other tracks", async () => {
    vi.useFakeTimers();
    const room = standInRoom();
    const bare = { attachedElements: [], getRTCStatsReport: async () => new Map([["in", { type: "inbound-rtp" }]]) };
    const pub: Pub = { source: "microphone", kind: "audio", trackSid: "TR_a", isMuted: false, isEnabled: true, track: bare };
    room.remoteParticipants.set("a", { identity: "stream:place:other", trackPublications: new Map([["a", pub]]) });
    remoteCamera(room);
    const s = sessionOf(room as unknown as Room, RoomEvent);
    await vi.advanceTimersByTimeAsync(STALL_MS + 2_000);
    expect(s.tracks().map((t) => [t.source, t.stalled])).toEqual([["microphone", false], ["camera", true]]);
    await s.disconnect();
  });

  it("never calls a muted track, or one the page switched off, stalled", async () => {
    vi.useFakeTimers();
    for (const pub of [{ isMuted: true }, { isEnabled: false }]) {
      const room = standInRoom();
      remoteCamera(room, pub);
      const s = sessionOf(room as unknown as Room, RoomEvent);
      await vi.advanceTimersByTimeAsync(STALL_MS * 3);
      expect(s.tracks()[0]!.stalled).toBe(false);
      await s.disconnect();
    }
  });

  it("ends a reconnection that lasts its limit, and not one that ends before", async () => {
    vi.useFakeTimers();
    for (const limit of [undefined, RECONNECT_LIMIT_MS.streamer]) {
      const ms = limit ?? RECONNECT_LIMIT_MS.watcher;
      const room = standInRoom();
      sessionOf(room as unknown as Room, RoomEvent, limit);
      room.emit("connectionStateChanged", "signalReconnecting");
      await vi.advanceTimersByTimeAsync(ms - 1);
      room.emit("connectionStateChanged", "connected");
      await vi.advanceTimersByTimeAsync(ms);
      expect(room.disconnect).not.toHaveBeenCalled();

      room.emit("connectionStateChanged", "signalReconnecting");
      await vi.advanceTimersByTimeAsync(ms / 2);
      room.emit("connectionStateChanged", "reconnecting");
      await vi.advanceTimersByTimeAsync(ms / 2);
      expect(room.disconnect).toHaveBeenCalledTimes(1);
    }
  });
});

describe("canDecode", () => {
  it("is true only for a codec this browser lists, in any letter case", () => {
    expect(canDecode("video/H264")).toBe(false);
    vi.stubGlobal("RTCRtpReceiver", { getCapabilities: () => ({ codecs: [{ mimeType: "video/VP8" }, { mimeType: "video/h264" }] }) });
    expect(canDecode("video/H264")).toBe(true);
    expect(canDecode("video/AV1")).toBe(false);
  });
});
