import { QueryClient } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { AuthIdentity } from "../auth/types";
import {
  PROFILE_COLLECTIBLES_PAGE,
  PROFILE_STALE_MS,
  assetsQuery,
  canReadProfile,
  creationsQuery,
  myCommunitiesQuery,
  myFavoritePlacesQuery,
  nextCollectiblesPage,
  photosQuery,
  placeOwnersQuery,
  placesQuery,
  profileKeys,
  profileQuery,
} from "./queries";

const ADDRESS = "0x17A253C2ac0d5BA92cadBBF665e3390C9913dC5D";
const LOWER = ADDRESS.toLowerCase();

let calls: { url: URL; signal: AbortSignal | null | undefined }[];
let respond: (url: URL) => Response;

beforeEach(() => {
  calls = [];
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: string, init?: RequestInit) => {
      const url = new URL(input);
      calls.push({ url, signal: init?.signal });
      return respond(url);
    }),
  );
});
afterEach(() => {
  vi.unstubAllGlobals();
});

const json = (body: unknown, status = 200) =>
  new Response(JSON.stringify(body), { status });

describe("profileKeys", () => {
  it("names every public read by part and lowercased address", () => {
    expect(profileKeys.profile(ADDRESS)).toEqual(["profile", "profile", LOWER]);
    expect(profileKeys.photos(` ${ADDRESS} `)).toEqual(["profile", "photos", LOWER]);
  });

  it("keys creations and assets by category, so each filter is its own list", () => {
    expect(profileKeys.creations(ADDRESS, "emote")).toEqual(["profile", "creations", LOWER, "emote"]);
    expect(profileKeys.creations(ADDRESS, "wearable")).not.toEqual(profileKeys.creations(ADDRESS, "emote"));
    expect(profileKeys.assets(ADDRESS, null)).toEqual(["profile", "assets", LOWER, "all"]);
    expect(profileKeys.assets(ADDRESS, "ens")).toEqual(["profile", "assets", LOWER, "ens"]);
  });

  it("puts the member's own communities under the private account prefix", () => {
    expect(profileKeys.myCommunities(ADDRESS)).toEqual(["private", LOWER, "profile", "communities"]);
  });

  it("puts the member's saved places under the private account prefix", () => {
    expect(profileKeys.myFavoritePlaces(ADDRESS)).toEqual(["private", LOWER, "profile", "favorite-places"]);
  });

  it("names one owners entry per set of wallets, whatever their order, case or repeats", () => {
    const other = "0x1111111111111111111111111111111111111111";
    expect(profileKeys.placeOwners([ADDRESS, other, LOWER])).toEqual(["profile", "place-owners", [other, LOWER]]);
    expect(profileKeys.placeOwners([other, ADDRESS])).toEqual(profileKeys.placeOwners([LOWER, other]));
  });
});

describe("options factories", () => {
  it("declare their freshness, so loader data and the page agree on it", () => {
    const identity = { signer: ADDRESS } as AuthIdentity;
    const all = [
      profileQuery(ADDRESS),
      creationsQuery(ADDRESS, "wearable"),
      placesQuery(ADDRESS),
      photosQuery(ADDRESS),
      assetsQuery(ADDRESS, null),
      myCommunitiesQuery(identity),
      myFavoritePlacesQuery(identity),
      placeOwnersQuery([ADDRESS]),
    ];

    for (const options of all) expect(options.staleTime).toBe(PROFILE_STALE_MS);
  });
});

describe("canReadProfile", () => {
  it("accepts a wallet address in any case and with surrounding spaces", () => {
    expect(canReadProfile(` ${ADDRESS} `)).toBe(true);
  });

  it.each(["", "alice", "0x123", `${LOWER}00`])("rejects %j", (value) => {
    expect(canReadProfile(value)).toBe(false);
  });
});

describe("profileQuery", () => {
  it("reads the lowercased address, passes the query's signal and maps the first avatar", async () => {
    respond = () =>
      json({
        avatars: [{ name: "Alice", hasClaimedName: true, description: "hi", country: "AR" }],
      });
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });

    const profile = await client.fetchQuery(profileQuery(ADDRESS));

    expect(calls).toHaveLength(1);
    expect(calls[0].url.pathname).toBe(`/lambdas/profile/${LOWER}`);
    expect(calls[0].signal).toBeInstanceOf(AbortSignal);
    expect(profile).toMatchObject({
      address: LOWER,
      name: "Alice",
      hasClaimedName: true,
      bio: "hi",
      info: [{ key: "country", value: "AR" }],
    });
  });

  it("returns null when the address has no avatar", async () => {
    respond = () => json({ avatars: [] });
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });

    await expect(client.fetchQuery(profileQuery(ADDRESS))).resolves.toBeNull();
  });

  it("fails instead of caching an empty profile when the server errors", async () => {
    respond = () => json({ error: "down" }, 503);
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });

    await expect(client.fetchQuery(profileQuery(ADDRESS))).rejects.toThrow();
    expect(client.getQueryData(profileKeys.profile(ADDRESS))).toBeUndefined();
  });

  it("fails on a body that is not a profile envelope", async () => {
    respond = () => json({ avatars: "nope" });
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });

    await expect(client.fetchQuery(profileQuery(ADDRESS))).rejects.toThrow();
  });
});

describe("placeOwnersQuery", () => {
  it("reads every owner in one batch and indexes them by lowercased wallet", async () => {
    respond = () =>
      json([{ avatars: [{ ethAddress: ADDRESS, name: "Alice", hasClaimedName: true, avatar: { snapshots: {} } }] }]);
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });

    const owners = await client.fetchQuery(placeOwnersQuery([ADDRESS]));

    expect(calls).toHaveLength(1);
    expect(calls[0].url.pathname).toBe("/lambdas/profiles");
    expect(owners[LOWER]).toMatchObject({ name: "Alice" });
  });

  it("fails when the batch read errors, so no owner is invented", async () => {
    respond = () => json({ error: "down" }, 503);
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });

    await expect(client.fetchQuery(placeOwnersQuery([ADDRESS]))).rejects.toThrow();
  });
});

describe("photosQuery", () => {
  it("maps each image to the gallery view, keeping the image order", async () => {
    respond = () =>
      json({
        images: [
          {
            id: "a",
            url: "https://example.com/a.png",
            thumbnailUrl: "https://example.com/a-thumb.png",
            isPublic: true,
            metadata: {
              userName: "Alice",
              userAddress: LOWER,
              dateTime: "1700000000",
              realm: "main",
              scene: { name: "Plaza", location: { x: "1", y: "2" } },
              visiblePeople: [],
              placeId: "place-1",
            },
          },
        ],
        currentImages: 1,
        maxImages: 500,
      });
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });

    const photos = await client.fetchQuery(photosQuery(ADDRESS));

    expect(calls[0].url.pathname).toBe(`/camera-reel/api/users/${LOWER}/images`);
    expect(photos).toHaveLength(1);
    expect(photos[0]).toMatchObject({
      id: "a",
      metadata: { userName: "Alice", scene: { name: "Plaza", location: { x: "1", y: "2" } } },
    });
    expect(photos[0].grad).toContain("a-thumb.png");
  });
});

describe("nextCollectiblesPage", () => {
  const page = (rows: number, total: number, received = rows) => ({ items: Array.from({ length: rows }, (_, n) => n), received, total });

  it("starts the next page after every row already requested", () => {
    const first = page(PROFILE_COLLECTIBLES_PAGE, 60);
    expect(nextCollectiblesPage(first, [first])).toBe(PROFILE_COLLECTIBLES_PAGE);
    expect(nextCollectiblesPage(first, [first, first])).toBe(2 * PROFILE_COLLECTIBLES_PAGE);
  });

  it("ends when every row the catalogue holds was requested", () => {
    const last = page(12, 36);
    expect(nextCollectiblesPage(last, [page(24, 36), last])).toBeUndefined();
  });

  it("keeps going past a page whose rows all failed to parse", () => {
    const unreadable = page(0, 60, PROFILE_COLLECTIBLES_PAGE);
    expect(nextCollectiblesPage(unreadable, [unreadable])).toBe(PROFILE_COLLECTIBLES_PAGE);
  });

  it("ends on an empty page even when the total says there are more", () => {
    expect(nextCollectiblesPage(page(0, 500), [page(24, 500), page(0, 500)])).toBeUndefined();
  });
});

describe("assetsQuery", () => {
  it("reads one category of the lowercased owner's tokens, newest first", async () => {
    respond = () => json({ data: [], total: 0 });
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });

    const owned = await client.fetchInfiniteQuery(assetsQuery(ADDRESS, "ens"));

    expect(calls[0].url.pathname).toBe("/marketplace-api/v1/nfts");
    expect(Object.fromEntries(calls[0].url.searchParams)).toEqual({
      owner: LOWER,
      category: "ens",
      first: String(PROFILE_COLLECTIBLES_PAGE),
      skip: "0",
      sortBy: "newest",
    });
    expect(calls[0].signal).toBeInstanceOf(AbortSignal);
    expect(owned.pages).toEqual([{ items: [], received: 0, total: 0 }]);
  });
});
