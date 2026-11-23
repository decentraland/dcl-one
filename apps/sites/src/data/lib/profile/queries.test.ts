import { QueryClient } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { AuthIdentity } from "../auth/types";
import {
  PROFILE_COLLECTIBLES_PAGE,
  PROFILE_STALE_MS,
  assetsQuery,
  canReadProfile,
  communitiesQuery,
  creationsQuery,
  myFavoritePlacesQuery,
  nextCollectiblesPage,
  photosQuery,
  placeOwnersQuery,
  placesQuery,
  profileKeys,
  profileQuery,
  signsFor,
} from "./queries";

const ADDRESS = "0x17A253C2ac0d5BA92cadBBF665e3390C9913dC5D";
const LOWER = ADDRESS.toLowerCase();
const OTHER = "0x1111111111111111111111111111111111111111";

let calls: { url: URL; signal: AbortSignal | null | undefined }[];
let respond: (url: URL) => Response;
let client: QueryClient;

beforeEach(() => {
  calls = [];
  client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
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
  client.clear();
  vi.resetAllMocks();
  vi.unstubAllGlobals();
});

const json = (body: unknown, status = 200) =>
  new Response(JSON.stringify(body), { status });

describe("profileKeys", () => {
  describe("when naming public reads", () => {
    it("should name the profile by part and lowercased address", () => {
      expect(profileKeys.profile(ADDRESS)).toEqual(["profile", "profile", LOWER]);
    });

    it("should name the photos by part and trimmed, lowercased address", () => {
      expect(profileKeys.photos(` ${ADDRESS} `)).toEqual(["profile", "photos", LOWER]);
    });
  });

  describe("when keying creations and assets by category", () => {
    it("should key creations by category", () => {
      expect(profileKeys.creations(ADDRESS, "emote")).toEqual(["profile", "creations", LOWER, "emote"]);
    });

    it("should give each creations filter its own list", () => {
      expect(profileKeys.creations(ADDRESS, "wearable")).not.toEqual(profileKeys.creations(ADDRESS, "emote"));
    });

    it("should key assets without a category as all", () => {
      expect(profileKeys.assets(ADDRESS, null)).toEqual(["profile", "assets", LOWER, "all"]);
    });

    it("should key assets by category", () => {
      expect(profileKeys.assets(ADDRESS, "ens")).toEqual(["profile", "assets", LOWER, "ens"]);
    });
  });

  it("should put the member's own communities under the private account prefix", () => {
    expect(profileKeys.myCommunities(ADDRESS)).toEqual(["private", LOWER, "profile", "communities"]);
  });

  describe("when keying a member's communities by who reads them", () => {
    describe("and the member reads with their own identity", () => {
      it("should key them under the private account prefix", () => {
        expect(communitiesQuery(ADDRESS, { signer: ADDRESS } as AuthIdentity).queryKey).toEqual(["private", LOWER, "profile", "communities"]);
      });
    });

    describe("and anyone reads without an identity", () => {
      it("should key them as the public list", () => {
        expect(communitiesQuery(ADDRESS).queryKey).toEqual(["profile", "communities", LOWER]);
      });
    });

    describe("and another wallet reads with its identity", () => {
      it("should key them as the public list, never a private one", () => {
        expect(communitiesQuery(ADDRESS, { signer: `0x${"1".repeat(40)}` } as AuthIdentity).queryKey).toEqual(["profile", "communities", LOWER]);
      });
    });
  });

  it("should put the member's saved places under the private account prefix", () => {
    expect(profileKeys.myFavoritePlaces(ADDRESS)).toEqual(["private", LOWER, "profile", "favorite-places"]);
  });

  describe("when naming owners entries", () => {
    it("should dedupe and lowercase the wallets", () => {
      expect(profileKeys.placeOwners([ADDRESS, OTHER, LOWER])).toEqual(["profile", "place-owners", [OTHER, LOWER]]);
    });

    it("should name one entry per set of wallets, whatever their order or case", () => {
      expect(profileKeys.placeOwners([OTHER, ADDRESS])).toEqual(profileKeys.placeOwners([LOWER, OTHER]));
    });
  });
});

describe("options factories", () => {
  let identity: AuthIdentity;

  beforeEach(() => {
    identity = { signer: ADDRESS } as AuthIdentity;
  });

  it.each([
    ["profileQuery", () => profileQuery(ADDRESS)],
    ["creationsQuery", () => creationsQuery(ADDRESS, "wearable")],
    ["placesQuery", () => placesQuery(ADDRESS)],
    ["photosQuery", () => photosQuery(ADDRESS)],
    ["assetsQuery", () => assetsQuery(ADDRESS, null)],
    ["communitiesQuery", () => communitiesQuery(ADDRESS)],
    ["communitiesQuery signed", () => communitiesQuery(ADDRESS, identity)],
    ["myFavoritePlacesQuery", () => myFavoritePlacesQuery(identity)],
    ["placeOwnersQuery", () => placeOwnersQuery([ADDRESS])],
  ])("should declare the %s freshness, so loader data and the page agree on it", (_name, make) => {
    expect(make().staleTime).toBe(PROFILE_STALE_MS);
  });
});

describe("canReadProfile", () => {
  describe("when the value is a wallet address in any case with surrounding spaces", () => {
    it("should accept it", () => {
      expect(canReadProfile(` ${ADDRESS} `)).toBe(true);
    });
  });

  describe("when the value is not a wallet address", () => {
    it.each(["", "alice", "0x123", `${LOWER}00`])("should reject %j", (value) => {
      expect(canReadProfile(value)).toBe(false);
    });
  });
});

describe("profileQuery", () => {
  describe("when the address has an avatar", () => {
    let profile: unknown;

    beforeEach(async () => {
      respond = () =>
        json({
          avatars: [{ name: "Alice", hasClaimedName: true, description: "hi", country: "AR" }],
        });
      profile = await client.fetchQuery(profileQuery(ADDRESS));
    });

    it("should read once", () => {
      expect(calls).toHaveLength(1);
    });

    it("should read the lowercased address", () => {
      expect(calls[0].url.pathname).toBe(`/lambdas/profile/${LOWER}`);
    });

    it("should pass the query's signal", () => {
      expect(calls[0].signal).toBeInstanceOf(AbortSignal);
    });

    it("should map the first avatar", () => {
      expect(profile).toMatchObject({
        address: LOWER,
        name: "Alice",
        hasClaimedName: true,
        bio: "hi",
        info: [{ key: "country", value: "AR" }],
      });
    });
  });

  describe("when the address has no avatar", () => {
    beforeEach(() => {
      respond = () => json({ avatars: [] });
    });

    it("should return null", async () => {
      await expect(client.fetchQuery(profileQuery(ADDRESS))).resolves.toBeNull();
    });
  });

  describe("when the server errors", () => {
    beforeEach(() => {
      respond = () => json({ error: "down" }, 503);
    });

    it("should fail", async () => {
      await expect(client.fetchQuery(profileQuery(ADDRESS))).rejects.toThrow("503");
    });

    it("should not cache an empty profile", async () => {
      await client.fetchQuery(profileQuery(ADDRESS)).catch(() => undefined);
      expect(client.getQueryData(profileKeys.profile(ADDRESS))).toBeUndefined();
    });
  });

  describe("when the body is not a profile envelope", () => {
    beforeEach(() => {
      respond = () => json({ avatars: "nope" });
    });

    it("should fail", async () => {
      await expect(client.fetchQuery(profileQuery(ADDRESS))).rejects.toThrow(/avatars/);
    });
  });
});

describe("placeOwnersQuery", () => {
  describe("when the batch read succeeds", () => {
    const fetchOwners = () => client.fetchQuery(placeOwnersQuery([ADDRESS]));
    let owners: Awaited<ReturnType<typeof fetchOwners>>;

    beforeEach(async () => {
      respond = () =>
        json([{ avatars: [{ ethAddress: ADDRESS, name: "Alice", hasClaimedName: true, avatar: { snapshots: {} } }] }]);
      owners = await fetchOwners();
    });

    it("should read every owner in one batch", () => {
      expect(calls).toHaveLength(1);
      expect(calls[0].url.pathname).toBe("/lambdas/profiles");
    });

    it("should index the owners by lowercased wallet", () => {
      expect(owners[LOWER]).toMatchObject({ name: "Alice" });
    });
  });

  describe("when the batch read errors", () => {
    beforeEach(() => {
      respond = () => json({ error: "down" }, 503);
    });

    it("should fail, so no owner is invented", async () => {
      await expect(client.fetchQuery(placeOwnersQuery([ADDRESS]))).rejects.toThrow("down");
    });
  });
});

describe("photosQuery", () => {
  const image = (id: string) => ({
    id,
    url: `https://example.com/${id}.png`,
    thumbnailUrl: `https://example.com/${id}-thumb.png`,
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
  });

  describe("when reading the public gallery", () => {
    let options: ReturnType<typeof photosQuery>;
    let data: Awaited<ReturnType<QueryClient["fetchInfiniteQuery"]>>;

    beforeEach(async () => {
      respond = () => json({ images: [image("a")], currentImages: 30, maxImages: 500 });
      options = photosQuery(ADDRESS);
      data = await client.fetchInfiniteQuery(options);
    });

    it("should key it under the public key", () => {
      expect(options.queryKey).toEqual(profileKeys.photos(ADDRESS));
    });

    it("should read the lowercased address's gallery, unsigned", () => {
      expect(calls[0].url.pathname).toBe(`/camera-reel-service/api/users/${LOWER}/images`);
    });

    it("should read the first page", () => {
      expect(Object.fromEntries(calls[0].url.searchParams)).toEqual({ limit: String(PROFILE_COLLECTIBLES_PAGE), offset: "0" });
    });

    it("should pass the query's signal", () => {
      expect(calls[0].signal).toBeInstanceOf(AbortSignal);
    });

    it("should map the page's photos", () => {
      expect(data.pages[0]).toMatchObject({ items: [{ id: "a", scene: { name: "Plaza", x: "1", y: "2" } }], total: 30 });
    });
  });

  describe("when the owner signs the read", () => {
    let identity: AuthIdentity;

    beforeEach(() => {
      identity = { signer: ADDRESS } as AuthIdentity;
    });

    it("should key the signed gallery under their account's private prefix", () => {
      expect(photosQuery(ADDRESS, identity).queryKey).toEqual(["private", LOWER, "profile", "photos"]);
    });

    it("should match the owner's own photos key", () => {
      expect(profileKeys.myPhotos(ADDRESS)).toEqual(["private", LOWER, "profile", "photos"]);
    });
  });

  describe("when another wallet's identity is given", () => {
    let other: AuthIdentity;

    beforeEach(() => {
      other = { signer: OTHER } as AuthIdentity;
    });

    it("should never use it for the profile's gallery", () => {
      expect(photosQuery(ADDRESS, other).queryKey).toEqual(profileKeys.photos(ADDRESS));
    });

    it("should not sign for the profile", () => {
      expect(signsFor(other, ADDRESS)).toBe(false);
    });
  });

  describe("when there is no identity", () => {
    it("should not sign for the profile", () => {
      expect(signsFor(null, ADDRESS)).toBe(false);
    });
  });

  describe("when the identity is the profile's wallet in another spelling", () => {
    it("should sign for the profile", () => {
      expect(signsFor({ signer: ` ${ADDRESS} ` } as AuthIdentity, LOWER)).toBe(true);
    });
  });

  describe("when reading two pages", () => {
    let data: Awaited<ReturnType<QueryClient["fetchInfiniteQuery"]>>;

    beforeEach(async () => {
      respond = (url) =>
        json({ images: url.searchParams.get("offset") === "0" ? [image("a")] : [image("b")], currentImages: 30, maxImages: 500 });
      data = await client.fetchInfiniteQuery({ ...photosQuery(ADDRESS), pages: 2 });
    });

    it("should ask for the next page after every row already requested", () => {
      expect(calls.map((call) => call.url.searchParams.get("offset"))).toEqual(["0", String(PROFILE_COLLECTIBLES_PAGE)]);
    });

    it("should answer both pages", () => {
      expect(data.pages).toHaveLength(2);
    });
  });

  describe("when the gallery is unavailable", () => {
    beforeEach(() => {
      respond = () => json({ error: "down" }, 503);
    });

    it("should fail the read", async () => {
      await expect(client.fetchInfiniteQuery(photosQuery(ADDRESS))).rejects.toThrow("503");
    });
  });
});

describe("nextCollectiblesPage", () => {
  const page = (rows: number, total: number, received = rows) => ({ items: Array.from({ length: rows }, (_, n) => n), received, total });

  describe("when rows remain", () => {
    let first: ReturnType<typeof page>;

    beforeEach(() => {
      first = page(PROFILE_COLLECTIBLES_PAGE, 60);
    });

    it("should start the second page after the first", () => {
      expect(nextCollectiblesPage(first, [first])).toBe(PROFILE_COLLECTIBLES_PAGE);
    });

    it("should start the third page after every row already requested", () => {
      expect(nextCollectiblesPage(first, [first, first])).toBe(2 * PROFILE_COLLECTIBLES_PAGE);
    });
  });

  describe("when every row the catalogue holds was requested", () => {
    it("should end", () => {
      const last = page(12, 36);
      expect(nextCollectiblesPage(last, [page(24, 36), last])).toBeUndefined();
    });
  });

  describe("when every row of the page failed to parse", () => {
    it("should keep going", () => {
      const unreadable = page(0, 60, PROFILE_COLLECTIBLES_PAGE);
      expect(nextCollectiblesPage(unreadable, [unreadable])).toBe(PROFILE_COLLECTIBLES_PAGE);
    });
  });

  describe("when the page is empty even though the total says there are more", () => {
    it("should end", () => {
      expect(nextCollectiblesPage(page(0, 500), [page(24, 500), page(0, 500)])).toBeUndefined();
    });
  });
});

describe("assetsQuery", () => {
  describe("when reading one category of the owner's tokens", () => {
    let owned: Awaited<ReturnType<QueryClient["fetchInfiniteQuery"]>>;

    beforeEach(async () => {
      respond = () => json({ data: [], total: 0 });
      owned = await client.fetchInfiniteQuery(assetsQuery(ADDRESS, "ens"));
    });

    it("should read the marketplace's NFTs", () => {
      expect(calls[0].url.pathname).toBe("/marketplace-api/v1/nfts");
    });

    it("should ask for the lowercased owner's category, newest first", () => {
      expect(Object.fromEntries(calls[0].url.searchParams)).toEqual({
        owner: LOWER,
        category: "ens",
        first: String(PROFILE_COLLECTIBLES_PAGE),
        skip: "0",
        sortBy: "newest",
      });
    });

    it("should pass the query's signal", () => {
      expect(calls[0].signal).toBeInstanceOf(AbortSignal);
    });

    it("should answer the pages", () => {
      expect(owned.pages).toEqual([{ items: [], received: 0, total: 0 }]);
    });
  });
});
