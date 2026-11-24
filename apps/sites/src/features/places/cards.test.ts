import { beforeEach, describe, expect, it } from "vitest";

import type { PlaceCardData } from "@dcl/one-ui/apps/sites";

import type { Destination } from "@/data/lib/catalyst/places/destinations";
import { destination } from "@/data/lib/catalyst/places/destination.fixture";

import { getAvatarBackgroundColor, getDisplayName } from "@web-core/profile/avatarColor";

import { type CreatorProfiles, creatorAddresses, creatorBatches, modalPlace, placeCard, placeCreator } from "./cards";

const DESKTOP = { kind: "desktop", os: "macos", arch: "unknown" } as const;
const ANDROID = { kind: "mobile", os: "android" } as const;
const IOS = { kind: "mobile", os: "ios" } as const;
const WALLET = "0x00000000000000000000000000000000000000aa";

describe("when crediting a card", () => {
  describe("and the scene belongs to the Foundation", () => {
    let place: Destination;

    beforeEach(() => {
      place = destination({ contact_name: "Decentraland Foundation" });
    });

    it("should show the Foundation's logo and link no profile", () => {
      expect(placeCreator(place)).toEqual({
        name: "Decentraland Foundation",
        foundation: true,
        avatarBackground: "#9d76e3",
      });
    });
  });

  describe("and the scene declares only the SDK default", () => {
    let creator: PlaceCardData["creator"];

    beforeEach(() => {
      creator = placeCreator(destination({ contact_name: "SDK", title: "Monster Recon" }));
    });

    it("should credit the title with its avatar background", () => {
      expect(creator).toMatchObject({ name: "Monster Recon", avatarBackground: getAvatarBackgroundColor(getDisplayName("Monster Recon", false)) });
    });

    it("should link no profile", () => {
      expect(creator?.href).toBeUndefined();
    });

    it("should show a synthetic avatar", () => {
      expect(creator?.avatarUrl).toMatch(/^data:image\/svg\+xml/);
    });
  });

  describe("and the deployer's profile was read", () => {
    let place: Destination;
    let profiles: CreatorProfiles;

    beforeEach(() => {
      place = destination({ owner: WALLET });
      profiles = { [WALLET]: { name: "Ann", avatarUrl: "https://cdn.example/a.png" } };
    });

    it("should link the deployer's profile with their face", () => {
      expect(placeCreator(place, profiles)).toEqual({
        name: "Studio",
        href: `/profile/${WALLET}`,
        avatarUrl: "https://cdn.example/a.png",
        avatarBackground: getAvatarBackgroundColor(getDisplayName("Ann", false, WALLET)),
      });
    });
  });

  describe("and the creators' faces are collected", () => {
    let places: Destination[];

    beforeEach(() => {
      places = [
        destination({ id: "a", owner: WALLET }),
        destination({ id: "b", contact_name: "SDK" }),
        destination({ id: "c", contact_name: "Decentraland Foundation" }),
      ];
    });

    it("should not ask for the faces of the Foundation or of authorless scenes", () => {
      expect(creatorAddresses(places)).toEqual([WALLET]);
    });
  });
});

describe("when mapping a destination to a card", () => {
  describe("and the place is in Genesis City on desktop", () => {
    let card: PlaceCardData;

    beforeEach(() => {
      card = placeCard(destination({ id: "a b", user_count: 3, highlighted: true }), DESKTOP);
    });

    it("should open the place page", () => {
      expect(card.href).toBe("/places/a%20b");
    });

    it("should jump into the web explorer", () => {
      expect(card.jumpHref).toMatch(/^\/play\/?\?position=1%2C2$/);
    });

    it("should carry the count, the featured mark and the coordinates", () => {
      expect(card).toMatchObject({ users: 3, usersLabel: "3 people in-world", featured: true, coords: "1,2" });
    });
  });

  describe("and the place is a World", () => {
    let card: PlaceCardData;

    beforeEach(() => {
      card = placeCard(destination({ world: true, world_name: "party.dcl.eth", base_position: "0,0" }), DESKTOP);
    });

    it("should show the World's name as the location", () => {
      expect(card.coords).toBe("party.dcl.eth");
    });

    it("should carry the World as the realm", () => {
      expect(card.jumpHref).toContain("realm=party.dcl.eth");
    });
  });

  describe("and the visitor is on a phone", () => {
    let place: Destination;

    beforeEach(() => {
      place = destination();
    });

    describe("and the phone is Android", () => {
      it("should open the app through an intent", () => {
        expect(placeCard(place, ANDROID).jumpHref).toMatch(/^intent:\/\/open\?position=1%2C2#Intent;/);
      });
    });

    describe("and the phone is iOS", () => {
      it("should open the app through its scheme", () => {
        expect(placeCard(place, IOS).jumpHref).toBe("decentraland://open?position=1%2C2");
      });
    });
  });

  describe("and the place has no title, count or cover", () => {
    let card: PlaceCardData;

    beforeEach(() => {
      card = placeCard(destination({ title: " ", image: "", user_count: null }), DESKTOP);
    });

    it("should name it an untitled place", () => {
      expect(card.title).toBe("Untitled place");
    });

    it("should leave out the cover", () => {
      expect(card.image).toBeNull();
    });

    it("should leave out the count", () => {
      expect(card.users).toBeUndefined();
    });
  });

  describe("and the card opens the modal", () => {
    let place: Destination;

    beforeEach(() => {
      place = destination({ description: "Come by" });
    });

    it("should give the modal an absolute share link", () => {
      expect(modalPlace(place, placeCard(place, DESKTOP), "https://example.org")).toMatchObject({
        shareUrl: "https://example.org/places/plaza",
        description: "Come by",
      });
    });
  });
});

describe("when batching creator reads", () => {
  const A = "0x00000000000000000000000000000000000000a1";
  const B = "0x00000000000000000000000000000000000000b2";
  const C = "0x00000000000000000000000000000000000000c3";

  describe("and the live creators change", () => {
    let page: Destination[];

    beforeEach(() => {
      page = [destination({ owner: A }), destination({ id: "2", owner: B })];
    });

    it("should keep the page's batch for the first set of live creators", () => {
      expect(creatorBatches([[destination({ owner: A })], page])[1]).toEqual([A, B]);
    });

    it("should keep the page's batch for another set of live creators", () => {
      expect(creatorBatches([[destination({ owner: C })], page])[1]).toEqual([A, B]);
    });

    it("should give each page the same batch", () => {
      expect(creatorBatches([page, page])).toEqual([
        [A, B],
        [A, B],
      ]);
    });
  });

  describe("and a group has no creators", () => {
    let groups: Destination[][];

    beforeEach(() => {
      groups = [[], [destination({ contact_name: "SDK" })]];
    });

    it("should drop it", () => {
      expect(creatorBatches(groups)).toEqual([]);
    });
  });
});
