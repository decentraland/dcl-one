import { QueryClient } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { AuthIdentity } from "../auth/types";
import {
  PROFILE_STALE_MS,
  canReadProfile,
  creationsQuery,
  myCommunitiesQuery,
  ownedNamesQuery,
  ownedWearablesQuery,
  photosQuery,
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

  it("puts the member's own communities under the private account prefix", () => {
    expect(profileKeys.myCommunities(ADDRESS)).toEqual(["private", LOWER, "profile", "communities"]);
  });
});

describe("options factories", () => {
  it("declare their freshness, so loader data and the page agree on it", () => {
    const identity = { signer: ADDRESS } as AuthIdentity;
    const all = [
      profileQuery(ADDRESS),
      creationsQuery(ADDRESS),
      placesQuery(ADDRESS),
      photosQuery(ADDRESS),
      ownedWearablesQuery(ADDRESS),
      ownedNamesQuery(ADDRESS),
      myCommunitiesQuery(identity),
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

describe("ownedWearablesQuery", () => {
  it("asks for the first page of the address's wearables", async () => {
    respond = () => json({ ok: true, data: { elements: [], total: 0, page: 0, pages: 0, limit: 48 } });
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });

    const owned = await client.fetchQuery(ownedWearablesQuery(ADDRESS));

    expect(calls[0].url.pathname).toBe(`/marketplace-api/v1/users/${LOWER}/wearables`);
    expect(calls[0].url.searchParams.get("first")).toBe("48");
    expect(owned).toEqual({ elements: [], total: 0 });
  });
});
