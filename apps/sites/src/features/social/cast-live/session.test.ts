import { describe, expect, it, vi } from "vitest";
import type { Room, RoomEvent as RoomEventEnum } from "livekit-client";

import { sessionOf } from "./session";

// The string values of livekit-client's RoomEvent members this module listens to.
const RoomEvent = {
  TrackPublished: "trackPublished",
  TrackUnpublished: "trackUnpublished",
  TrackSubscribed: "trackSubscribed",
  TrackUnsubscribed: "trackUnsubscribed",
  TrackMuted: "trackMuted",
  TrackUnmuted: "trackUnmuted",
  LocalTrackPublished: "localTrackPublished",
  LocalTrackUnpublished: "localTrackUnpublished",
  ParticipantConnected: "participantConnected",
  ParticipantDisconnected: "participantDisconnected",
  AudioPlaybackStatusChanged: "audioPlaybackChanged",
  Disconnected: "disconnected",
} as unknown as typeof RoomEventEnum;

type Pub = { source: string; trackSid: string; isMuted: boolean; track: object | undefined };

function standInRoom() {
  const handlers = new Map<string, Array<() => void>>();
  const local = { identity: "stream:p:1", trackPublications: new Map<string, Pub>() } as {
    identity: string;
    trackPublications: Map<string, Pub>;
    setScreenShareEnabled: ReturnType<typeof vi.fn>;
  };
  const room = {
    canPlaybackAudio: true,
    localParticipant: local,
    remoteParticipants: new Map<string, { identity: string; trackPublications: Map<string, Pub> }>(),
    on: (ev: string, fn: () => void) => {
      handlers.set(ev, [...(handlers.get(ev) ?? []), fn]);
      return room;
    },
    emit: (ev: string) => handlers.get(ev)?.forEach((fn) => fn()),
    disconnect: vi.fn(async () => {}),
    startAudio: vi.fn(async () => {
      room.canPlaybackAudio = true;
    }),
  };
  local.setScreenShareEnabled = vi.fn();
  return room;
}

describe("sessionOf", () => {
  it("lists local and remote tracks with their source, mute and identity, skipping unsubscribed ones", () => {
    const room = standInRoom();
    room.localParticipant.trackPublications.set("a", { source: "camera", trackSid: "TR_a", isMuted: false, track: {} });
    room.remoteParticipants.set("w", {
      identity: "0xabc",
      trackPublications: new Map([
        ["b", { source: "microphone", trackSid: "TR_b", isMuted: true, track: {} }],
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

  it("is already disconnected when its listeners hear the room close, and a later leave does not call the room", async () => {
    const room = standInRoom();
    const s = sessionOf(room as unknown as Room, RoomEvent);
    const seen: boolean[] = [];
    s.subscribe(() => seen.push(s.disconnected()));
    room.emit("disconnected");
    expect(seen).toEqual([true]);
    await s.disconnect();
    expect(room.disconnect).not.toHaveBeenCalled();
  });

  it("reports blocked audio and unblocks it on request", async () => {
    const room = standInRoom();
    room.canPlaybackAudio = false;
    const s = sessionOf(room as unknown as Room, RoomEvent);
    const listener = vi.fn();
    s.subscribe(listener);
    expect(s.canPlaybackAudio()).toBe(false);
    await s.startAudio();
    expect(s.canPlaybackAudio()).toBe(true);
    expect(listener).toHaveBeenCalled();
  });

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
});
