import { describe, expect, it, vi } from "vitest";

import type { Event } from "../places/events";
import type { Place } from "../schema";
import {
  buildWhatsOnCards,
  FOUNDATION_NAME,
  withCreators,
  type WhatsOnCard,
} from "./events.server";

const ADDRESS = "0x1111111111111111111111111111111111111111";

function place(id: string, users: number, extra: Partial<Place> = {}): Place {
  return {
    id,
    title: `Place ${id}`,
    image: `https://img.example/${id}.png`,
    owner: null,
    positions: [`${id},0`],
    base_position: `${id},0`,
    contact_name: null,
    user_count: users,
    world: false,
    ...extra,
  } as Place;
}

function event(
  id: string,
  x: number,
  y: number,
  extra: Partial<Event> = {},
): Event {
  return {
    id,
    name: `Event ${id}`,
    image: null,
    x,
    y,
    user_name: "Host",
    live: true,
    ...extra,
  } as Event;
}

describe("buildWhatsOnCards", () => {
  it("puts crowded scenes hosting a live event first, then crowded scenes by users", () => {
    const cards = buildWhatsOnCards(
      [event("e1", 2, 0)],
      [place("1", 50), place("2", 10), place("3", 30)],
    );

    expect(cards.map((card) => [card.type, card.id, card.users])).toEqual([
      ["event", "e1", 10],
      ["place", "1", 50],
      ["place", "3", 30],
    ]);
    expect(cards[0]).toMatchObject({
      creatorName: "Host",
      href: "/events?id=e1",
      coordinates: "2,0",
    });
    expect(cards[1]).toMatchObject({ href: "/places/1" });
  });

  it("credits an event card to its organizer's address, and only a valid one", () => {
    const [card] = buildWhatsOnCards(
      [event("e1", 1, 0, { user: ADDRESS.toUpperCase().replace("0X", "0x") })],
      [place("1", 9)],
    );
    expect(card).toMatchObject({
      type: "event",
      creatorName: "Host",
      creatorAddress: ADDRESS,
    });

    const [unknown] = buildWhatsOnCards(
      [event("e2", 1, 0, { user: "not-an-address" })],
      [place("1", 9)],
    );
    expect(unknown).not.toHaveProperty("creatorAddress");
  });

  it("ignores scenes under 5 users and worlds", () => {
    const cards = buildWhatsOnCards(
      [],
      [place("1", 4), place("2", 99, { world: true }), place("3", 5)],
    );
    expect(
      cards.filter((card) => !card.isGenesisPlaza).map((card) => card.id),
    ).toEqual(["3"]);
  });

  it("fills up with Genesis Plaza, credited to the Foundation", () => {
    const cards = buildWhatsOnCards(
      [],
      [
        place("1", 8),
        place("9", 0, {
          title: "Genesis Plaza",
          base_position: "0,0",
          positions: ["0,0", "1,0"],
        }),
      ],
    );

    expect(cards.at(-1)).toMatchObject({
      id: "9",
      image: "https://img.example/9.png",
      users: 0,
      coordinates: "0,0",
      href: "/places/9",
      isGenesisPlaza: true,
      creatorName: FOUNDATION_NAME,
    });
  });

  it("does not credit the Foundation to a scene that only names itself Genesis Plaza", () => {
    const spoof = place("7", 0, {
      title: "Genesis Plaza",
      base_position: "40,40",
      positions: ["40,40"],
    });
    const world = place("8", 0, {
      title: "Genesis Plaza",
      world: true,
      positions: ["0,0"],
    });
    const cards = buildWhatsOnCards([], [spoof, world]);

    expect(
      cards.find((card) => card.id === "7" || card.id === "8"),
    ).toBeUndefined();
    expect(cards).toEqual([]);
  });

  it("does not credit the Foundation to a crowded scene named Genesis Plaza off parcel 0,0", () => {
    const spoof = place("7", 30, {
      title: "Genesis Plaza",
      base_position: "40,40",
      positions: ["40,40"],
    });
    const card = buildWhatsOnCards([], [spoof]).find(
      (entry) => entry.id === "7",
    );

    expect(card).toMatchObject({
      title: "Genesis Plaza",
      isGenesisPlaza: false,
    });
    expect(card).not.toHaveProperty("creatorName");
  });

  it("returns no cards when the places list is empty", () => {
    expect(buildWhatsOnCards([], [])).toEqual([]);
  });

  it("does not invent Genesis when supplied places contain only uncrowded scenes", () => {
    expect(buildWhatsOnCards([], [place("1", 4)])).toEqual([]);
  });

  it("retains only supplied crowded places when Genesis is absent", () => {
    const cards = buildWhatsOnCards([], [place("1", 8)]);
    expect(cards).toHaveLength(1);
    expect(cards[0]).toMatchObject({
      id: "1",
      image: "https://img.example/1.png",
      users: 8,
      isGenesisPlaza: false,
    });
  });

  it("does not turn unmatched live events into fabricated place cards", () => {
    expect(buildWhatsOnCards([event("e1", 0, 0)], [])).toEqual([]);
  });

  it("never returns more than 3 cards", () => {
    expect(
      buildWhatsOnCards(
        [],
        ["1", "2", "3", "4", "5"].map((id) => place(id, 10)),
      ),
    ).toHaveLength(3);
  });
});

describe("withCreators", () => {
  const card = (id: string, extra: Partial<WhatsOnCard> = {}): WhatsOnCard => ({
    type: "place",
    id,
    title: id,
    users: 10,
    image: null,
    coordinates: `${id},0`,
    href: "/play",
    isGenesisPlaza: false,
    ...extra,
  });

  it("skips all creator lookups when no real cards are available", async () => {
    const deployment = vi.fn();
    const profiles = vi.fn();
    await expect(
      withCreators(buildWhatsOnCards([], []), {}, { deployment, profiles }),
    ).resolves.toEqual([]);
    expect(deployment).not.toHaveBeenCalled();
    expect(profiles).not.toHaveBeenCalled();
  });

  it("names a place card after its scene deployer's profile, against this node", async () => {
    const deployment = vi.fn(async () => ({
      entityId: "bafy",
      title: null,
      deployer: ADDRESS.toUpperCase().replace("0X", "0x"),
    }));
    const profiles = vi.fn(async () => ({
      [ADDRESS]: { name: "Builder", picture: null, claimed: true },
    }));

    const [named] = await withCreators(
      [card("1")],
      { base: "http://node.example" },
      { deployment, profiles },
    );

    expect(named).toMatchObject({
      creatorAddress: ADDRESS,
      creatorName: "Builder",
    });
    expect(deployment).toHaveBeenCalledWith(
      "1,0",
      expect.objectContaining({ base: "http://node.example" }),
    );
    expect(profiles).toHaveBeenCalledWith(
      [ADDRESS],
      expect.objectContaining({ base: "http://node.example" }),
    );
  });

  it("pictures every creator from one profile lookup, at the public origin, keeping event names", async () => {
    const OTHER = "0x2222222222222222222222222222222222222222";
    const deployment = vi.fn(async () => ({
      entityId: "bafy",
      title: null,
      deployer: OTHER,
    }));
    const profiles = vi.fn(async () => ({
      [ADDRESS]: {
        name: "Profile name",
        picture: "http://loopback:5000/content/contents/bafyface",
        claimed: true,
      },
      [OTHER]: {
        name: "Builder",
        picture: "https://cdn.example/face.png",
        claimed: true,
      },
    }));
    const cards = [
      card("e", {
        type: "event",
        creatorName: "Host",
        creatorAddress: ADDRESS,
      }),
      card("1"),
    ];

    const named = await withCreators(
      cards,
      { base: "http://loopback:5000" },
      { deployment, profiles, publicBase: "https://node.example" },
    );

    expect(profiles).toHaveBeenCalledTimes(1);
    expect(profiles).toHaveBeenCalledWith(
      [ADDRESS, OTHER],
      expect.objectContaining({ base: "http://loopback:5000" }),
    );
    expect(named[0]).toMatchObject({
      creatorName: "Host",
      creatorAvatarUrl: "https://node.example/content/contents/bafyface",
    });
    expect(named[1]).toMatchObject({
      creatorName: "Builder",
      creatorAddress: OTHER,
      creatorAvatarUrl: "https://cdn.example/face.png",
    });
  });

  it("publishes faces the real profile lookup built against the request's node, and passes external ones through", async () => {
    const OTHER = "0x2222222222222222222222222222222222222222";
    const fetchImpl = vi.fn(async (url: string) => {
      expect(url).toBe("http://loopback:5000/lambdas/profiles");
      return new Response(
        JSON.stringify([
          {
            avatars: [
              {
                ethAddress: ADDRESS,
                name: "Hashed",
                avatar: { snapshots: { face256: "bafyhashface" } },
              },
            ],
          },
          {
            avatars: [
              {
                ethAddress: OTHER,
                name: "External",
                avatar: {
                  snapshots: { face256: "https://images.test/face.png" },
                },
              },
            ],
          },
        ]),
      );
    }) as unknown as typeof fetch;
    const cards = [
      card("e", {
        type: "event",
        creatorName: "Host",
        creatorAddress: ADDRESS,
      }),
      card("f", { type: "event", creatorName: "Guest", creatorAddress: OTHER }),
    ];

    const named = await withCreators(
      cards,
      { base: "http://loopback:5000", fetchImpl },
      { publicBase: "https://node.example" },
    );

    expect(named[0]).toMatchObject({
      creatorAvatarUrl: "https://node.example/content/contents/bafyhashface",
    });
    expect(named[1]).toMatchObject({
      creatorAvatarUrl: "https://images.test/face.png",
    });
  });

  it("leaves the avatar out when the profile has no face, so the card shows the initial", async () => {
    const cards = [
      card("e", {
        type: "event",
        creatorName: "Host",
        creatorAddress: ADDRESS,
      }),
    ];
    const [named] = await withCreators(
      cards,
      {},
      {
        profiles: vi.fn(async () => ({
          [ADDRESS]: { name: "", picture: null, claimed: false },
        })),
      },
    );

    expect(named).toEqual(cards[0]);
  });

  it("keeps the address when the profile has no name", async () => {
    const [named] = await withCreators(
      [card("1")],
      {},
      {
        deployment: vi.fn(async () => ({
          entityId: "x",
          title: null,
          deployer: ADDRESS,
        })),
        profiles: vi.fn(async () => ({})),
      },
    );
    expect(named).toEqual({ ...card("1"), creatorAddress: ADDRESS });
  });

  it("leaves event cards and cards with a creator untouched, without lookups", async () => {
    const deployment = vi.fn();
    const cards = [
      card("e", { type: "event" }),
      card("g", { creatorName: FOUNDATION_NAME }),
    ];

    await expect(
      withCreators(cards, {}, { deployment, profiles: vi.fn() }),
    ).resolves.toBe(cards);
    expect(deployment).not.toHaveBeenCalled();
  });

  it("returns the cards unchanged when the content server or profiles fail", async () => {
    const cards = [card("1")];
    await expect(
      withCreators(
        cards,
        {},
        {
          deployment: vi.fn(async () => {
            throw new Error("502");
          }),
          profiles: vi.fn(),
        },
      ),
    ).resolves.toEqual(cards);
    await expect(
      withCreators(
        cards,
        {},
        {
          deployment: vi.fn(async () => ({
            entityId: "x",
            title: null,
            deployer: ADDRESS,
          })),
          profiles: vi.fn(async () => {
            throw new Error("timeout");
          }),
        },
      ),
    ).resolves.toEqual([{ ...card("1"), creatorAddress: ADDRESS }]);
  });
});
