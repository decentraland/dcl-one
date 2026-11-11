import { describe, expect, it } from "vitest";

import { destination } from "@/data/lib/catalyst/places/destination.fixture";

import { creatorAddresses, modalPlace, placeCard, placeCreator } from "./cards";

const DESKTOP = { kind: "desktop", os: "macos", arch: "unknown" } as const;
const ANDROID = { kind: "mobile", os: "android" } as const;
const IOS = { kind: "mobile", os: "ios" } as const;
const WALLET = "0x00000000000000000000000000000000000000aa";

describe("when crediting a card", () => {
  it("should show the Foundation's logo and link no profile", () => {
    expect(placeCreator(destination({ contact_name: "Decentraland Foundation" }))).toEqual({ name: "Decentraland Foundation", foundation: true });
  });

  it("should credit the title when the scene declares only the SDK default", () => {
    expect(placeCreator(destination({ contact_name: "SDK", title: "Monster Recon" }))).toEqual({ name: "Monster Recon" });
  });

  it("should link the deployer's profile with their face when it was read", () => {
    const creator = placeCreator(destination({ owner: WALLET }), { [WALLET]: { name: "Ann", avatarUrl: "https://cdn.example/a.png" } });
    expect(creator).toEqual({ name: "Studio", href: `/profile/${WALLET}`, avatarUrl: "https://cdn.example/a.png" });
  });

  it("should not ask for the faces of the Foundation or of authorless scenes", () => {
    const places = [destination({ id: "a", owner: WALLET }), destination({ id: "b", contact_name: "SDK" }), destination({ id: "c", contact_name: "Decentraland Foundation" })];
    expect(creatorAddresses(places)).toEqual([WALLET]);
  });
});

describe("when mapping a destination to a card", () => {
  it("should open the place page and jump into the web explorer on desktop", () => {
    const card = placeCard(destination({ id: "a b", user_count: 3, highlighted: true }), DESKTOP);
    expect(card.href).toBe("/places/a%20b");
    expect(card.jumpHref).toMatch(/^\/play\/?\?position=1%2C2$/);
    expect(card).toMatchObject({ users: 3, usersLabel: "3 people in-world", featured: true, coords: "1,2" });
  });

  it("should carry the World as the realm and show its name as the location", () => {
    const card = placeCard(destination({ world: true, world_name: "party.dcl.eth", base_position: "0,0" }), DESKTOP);
    expect(card.coords).toBe("party.dcl.eth");
    expect(card.jumpHref).toContain("realm=party.dcl.eth");
  });

  it("should open the app on a phone", () => {
    expect(placeCard(destination(), ANDROID).jumpHref).toMatch(/^intent:\/\/open\?position=1%2C2#Intent;/);
    expect(placeCard(destination(), IOS).jumpHref).toBe("decentraland://open?position=1%2C2");
  });

  it("should name an untitled place and leave out an empty count and cover", () => {
    const card = placeCard(destination({ title: " ", image: "", user_count: null }), DESKTOP);
    expect(card.title).toBe("Untitled place");
    expect(card.image).toBeNull();
    expect(card.users).toBeUndefined();
  });

  it("should give the modal an absolute share link", () => {
    const place = destination({ description: "Come by" });
    expect(modalPlace(place, placeCard(place, DESKTOP), "https://example.org")).toMatchObject({ shareUrl: "https://example.org/places/plaza", description: "Come by" });
  });
});
