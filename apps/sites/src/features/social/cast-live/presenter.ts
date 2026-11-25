// Names this streamer as the presenter of the room's presentation, as production's Cast page
// does: a client that draws the presenter's camera beside a slide reads who that is from the bot.
import type { Room, RoomEvent as RoomEventEnum } from "livekit-client";

/** `presentation:presenter:claim` as the bot reads it (`cast_presenter/wire.rs`): the RFC-4
 * `Packet { scene(6) { data(2) } }`, data `[3][topic length, u16 LE][topic][json]`. Every length
 * is under 128, so each is one byte. */
export const CLAIM = (() => {
  const text = new TextEncoder();
  const topic = text.encode("presentation");
  const data = [3, topic.length, 0, ...topic, ...text.encode('{"type":"presentation:presenter:claim"}')];
  return new Uint8Array([0x32, data.length + 2, 0x12, data.length, ...data]);
})();

/** Sends the claim when the presentation bot shows a slide whose presenter is not in the room,
 * once for each presenter the bot names. A bot that composes no slide names none: nothing is sent. */
export function claimPresenter(room: Room, RoomEvent: typeof RoomEventEnum): void {
  let claimedFor: string | null | undefined;
  const look = () => {
    for (const bot of room.remoteParticipants.values()) {
      if (!bot.identity.startsWith("presentation-bot:")) continue;
      let state: { slide?: unknown; presenterIdentity?: unknown } | null = null;
      try {
        state = JSON.parse(bot.metadata ?? "");
      } catch {
        // Not the bot's state.
      }
      if (!state?.slide) continue;
      const named = state.presenterIdentity;
      const presenter = typeof named === "string" && named !== "" ? named : null;
      const here = presenter === room.localParticipant.identity || room.remoteParticipants.has(presenter ?? "");
      if (here || claimedFor === presenter) return;
      claimedFor = presenter;
      room.localParticipant.publishData(CLAIM, { reliable: true }).catch(() => {
        if (claimedFor === presenter) claimedFor = undefined;
      });
      return;
    }
  };
  for (const ev of [RoomEvent.ParticipantConnected, RoomEvent.ParticipantDisconnected, RoomEvent.ParticipantMetadataChanged]) {
    room.on(ev, look);
  }
  look();
}
