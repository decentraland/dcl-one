import { describe, expect, it, vi } from "vitest";
import type { Room, RoomEvent as RoomEventEnum } from "livekit-client";

import { CLAIM, claimPresenter } from "./presenter";

const RoomEvent = {
  ParticipantConnected: "participantConnected",
  ParticipantDisconnected: "participantDisconnected",
  ParticipantMetadataChanged: "participantMetadataChanged",
} as unknown as typeof RoomEventEnum;

const ME = "stream:place:me";
const BOT = "presentation-bot:room:uuid";
const SLIDE = { url: "https://node.test/slide.png", width: 1280, height: 720 };

/** A room with the bot's metadata `state`, after `claimPresenter` looked at it. */
function roomWith(state: object | string, others: string[] = []) {
  const handlers: Array<() => void> = [];
  const text = (next: object | string) => (typeof next === "string" ? next : JSON.stringify({ role: "presentation", ...next }));
  const bot = { identity: BOT, metadata: text(state) };
  const publishData = vi.fn(async (_packet: Uint8Array, _opts: object) => {});
  const room = {
    localParticipant: { identity: ME, publishData },
    remoteParticipants: new Map<string, { identity: string; metadata?: string }>([[BOT, bot]]),
    on: (_ev: string, fn: () => void) => handlers.push(fn),
  };
  for (const identity of others) room.remoteParticipants.set(identity, { identity });
  claimPresenter(room as unknown as Room, RoomEvent);
  const change = (next: object) => {
    bot.metadata = text(next);
    handlers.forEach((fn) => fn());
  };
  return { publishData, change };
}

describe("claimPresenter", () => {
  it("claims once, with the packet the bot reads, when a slide has no presenter in the room", () => {
    const { publishData, change } = roomWith({ slide: SLIDE, presenterIdentity: null });
    change({ slide: SLIDE, presenterIdentity: null, currentSlide: 1 });
    expect(publishData).toHaveBeenCalledTimes(1);
    expect(publishData).toHaveBeenCalledWith(CLAIM, { reliable: true });
    // Packet { scene(6) { data(2) } }, data [3][topic length, u16 LE][topic][json].
    expect([...CLAIM.subarray(0, 7)]).toEqual([0x32, CLAIM.length - 2, 0x12, CLAIM.length - 4, 3, 12, 0]);
    expect(new TextDecoder().decode(CLAIM.subarray(7))).toBe('presentation{"type":"presentation:presenter:claim"}');
  });

  it("claims again for a presenter who is not in the room", () => {
    const { publishData, change } = roomWith({ slide: SLIDE, presenterIdentity: null });
    change({ slide: SLIDE, presenterIdentity: "stream:place:gone" });
    expect(publishData).toHaveBeenCalledTimes(2);
  });

  it("claims again after a claim that was not sent", async () => {
    const { publishData, change } = roomWith({ slide: null });
    publishData.mockRejectedValueOnce(new Error("not connected"));
    change({ slide: SLIDE });
    await Promise.resolve();
    change({ slide: SLIDE });
    expect(publishData).toHaveBeenCalledTimes(2);
  });

  it("does not claim with no slide, or when the presenter is this streamer or another participant", () => {
    for (const [state, others] of [
      ["not the bot's state", []],
      [{ presenterIdentity: null }, []],
      [{ slide: null, presenterIdentity: null }, []],
      [{ slide: SLIDE, presenterIdentity: ME }, []],
      [{ slide: SLIDE, presenterIdentity: "stream:place:other" }, ["stream:place:other"]],
    ] as const) {
      expect(roomWith(state, [...others]).publishData).not.toHaveBeenCalled();
    }
  });
});
