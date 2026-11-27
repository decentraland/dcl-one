import { beforeEach, describe, expect, it } from "vitest";
import type { LiveNowCard, Profile } from "@/data/lib/catalyst/places/live-now.server";
import { whatsOnCards } from "./cards.server";

describe("when building a Home live event card", () => {
  const address = "0x1111111111111111111111111111111111111111";
  const now = Date.parse("2026-11-27T10:05:50.000Z");
  let card: LiveNowCard;
  let profiles: Record<string, Profile>;

  beforeEach(() => {
    card = {
      id: "event",
      title: "Live event",
      users: 8,
      image: "",
      coordinates: "1,2",
      startedAt: "2026-11-27T10:00:00.000Z",
      attendeeAddresses: [address],
      isGenesisPlaza: false,
      place: false,
    };
    profiles = {
      [address]: {
        address,
        name: "Ada",
        hasClaimedName: true,
        face: "/profile-images/entities/ada/face.png",
      },
    };
  });

  it("should floor elapsed minutes and use the available RSVP face", () => {
    expect(whatsOnCards([card], profiles, now)[0]).toMatchObject({
      started: {
        dateTime: "2026-11-27T10:00:00.000Z",
        labels: { en: "Started 5 min. ago", zh: "5分钟前开始" },
      },
      attendees: [{ id: address, name: "Ada", avatarUrl: "/profile-images/entities/ada/face.png" }],
      share: true,
    });
  });

  describe("and the occurrence start is in the future", () => {
    beforeEach(() => {
      card.startedAt = "2026-11-27T10:06:00.000Z";
    });

    it("should omit the elapsed time", () => {
      expect(whatsOnCards([card], profiles, now)[0]?.started).toBeUndefined();
    });
  });

  describe("and the attendee has no profile", () => {
    beforeEach(() => {
      profiles = {};
    });

    it("should omit unavailable faces", () => {
      expect(whatsOnCards([card], profiles, now)[0]?.attendees).toEqual([]);
    });
  });
});
