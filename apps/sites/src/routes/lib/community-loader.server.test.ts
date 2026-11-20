import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { RouterContextProvider } from "react-router";

import { communityKeys } from "@/data/lib/community/queries";

import { communityLoader, type CommunityLoaderData } from "./community-loader.server";

const ID = "e99471aa-31c4-4952-abf6-99905445f43b";
const OWNER = "0xbcac4dafb7e215f2f6cb3312af6d5e4f9d9e7eda";

const community = (privacy: "public" | "private") => ({
  data: {
    id: ID,
    name: "Decentraland Foundation",
    description: "Official updates.",
    ownerAddress: OWNER,
    ownerName: "DCLOfficial",
    privacy,
    active: true,
    membersCount: 2,
  },
});
const members = {
  data: {
    results: [
      { communityId: ID, memberAddress: OWNER, role: "owner", name: "DCLOfficial", profilePictureUrl: "", hasClaimedName: true, joinedAt: "", friendshipStatus: 7 },
    ],
    total: 1,
    page: 1,
    pages: 1,
    limit: 10,
  },
};
const events = { ok: true, data: { events: [], total: 0 } };

type Reply = { status: number; body?: unknown } | Error;
type Thrown = { init?: { status?: number }; data?: unknown };
type Outcome = { ok: true; data: CommunityLoaderData } | { ok: false; thrown: Thrown };

let routes: { match: RegExp; reply: Reply }[];
let calls: string[];

const reply = (match: RegExp, r: Reply) => routes.push({ match, reply: r });

async function run(id: string): Promise<Outcome> {
  try {
    const result = (await communityLoader(new Request(`http://localhost/social/communities/${id}`), new RouterContextProvider(), {
      id,
    })) as unknown as { data: CommunityLoaderData };
    return { ok: true as const, data: result.data };
  } catch (thrown) {
    return { ok: false as const, thrown: thrown as Thrown };
  }
}

/** The loaded data, failing the test when the loader threw instead. */
function loaded(result: Outcome): CommunityLoaderData {
  if (!result.ok) throw new Error("expected the loader to resolve");
  return result.data;
}

/** What the loader threw, failing the test when it resolved instead. */
function failure(result: Outcome): Thrown {
  if (result.ok) throw new Error("expected the loader to throw");
  return result.thrown;
}

const keys = (data: CommunityLoaderData) => data.dehydratedState.queries.map((q) => JSON.stringify(q.queryKey));

describe("communityLoader", () => {
  let result: Outcome;

  beforeEach(() => {
    calls = [];
    routes = [];
    vi.stubGlobal(
      "fetch",
      vi.fn(async (input: string | URL) => {
        const url = String(input);
        calls.push(url);
        const route = routes.find((r) => r.match.test(url));
        if (!route) return new Response("not mocked", { status: 500 });
        if (route.reply instanceof Error) throw route.reply;
        return new Response(JSON.stringify(route.reply.body ?? {}), { status: route.reply.status });
      }),
    );
  });

  afterEach(() => {
    vi.resetAllMocks();
    vi.unstubAllGlobals();
  });

  describe("when the community is public", () => {
    beforeEach(async () => {
      reply(/\/v1\/communities\/[^/?]+\/members/, { status: 200, body: members });
      reply(/\/v1\/communities\/[^/?]+$/, { status: 200, body: community("public") });
      reply(/\/events\?|\/v1\/events/, { status: 200, body: events });

      result = await run(ID);
    });

    it("should resolve", () => {
      expect(result.ok).toBe(true);
    });

    it("should title the page after the community", () => {
      expect(loaded(result).title).toBe("Decentraland Foundation");
    });

    it("should describe the page with the community's description", () => {
      expect(loaded(result).description).toBe("Official updates.");
    });

    it("should dehydrate the community with its first members and events page", () => {
      expect(keys(loaded(result))).toEqual(
        expect.arrayContaining([
          JSON.stringify(communityKeys.detail(ID)),
          JSON.stringify(communityKeys.members(ID)),
          JSON.stringify(communityKeys.events(ID)),
        ]),
      );
    });

    it("should read the community's events", () => {
      expect(calls.some((url) => url.includes(`community_id=${ID}`))).toBe(true);
    });
  });

  describe("when the community is private", () => {
    beforeEach(async () => {
      reply(/\/v1\/communities\/[^/?]+$/, { status: 200, body: community("private") });

      result = await run(ID);
    });

    it("should resolve", () => {
      expect(result.ok).toBe(true);
    });

    it("should dehydrate only the community", () => {
      expect(keys(loaded(result))).toEqual([JSON.stringify(communityKeys.detail(ID))]);
    });

    it("should not read its members or events on the server", () => {
      expect(calls).toHaveLength(1);
    });

    // The page hides a private community's description from non-members; so does its meta.
    it("should hide its description", () => {
      expect(loaded(result).description).toBe("");
    });
  });

  describe("when the optional members and events reads fail", () => {
    beforeEach(async () => {
      reply(/\/v1\/communities\/[^/?]+\/members/, { status: 500 });
      reply(/\/v1\/communities\/[^/?]+$/, { status: 200, body: community("public") });
      reply(/\/events\?|\/v1\/events/, new Error("socket hang up"));

      result = await run(ID);
    });

    it("should still resolve", () => {
      expect(result.ok).toBe(true);
    });

    it("should dehydrate only the community", () => {
      expect(keys(loaded(result))).toEqual([JSON.stringify(communityKeys.detail(ID))]);
    });
  });

  describe("when the social API does not know the id", () => {
    beforeEach(async () => {
      reply(/\/v1\/communities\//, { status: 404, body: { error: "Not Found" } });

      result = await run(ID);
    });

    it("should throw", () => {
      expect(result.ok).toBe(false);
    });

    it("should answer 404", () => {
      expect(failure(result).init?.status).toBe(404);
    });

    it("should give not-found as the reason", () => {
      expect(failure(result).data).toEqual({ reason: "not-found" });
    });
  });

  describe("when the id is malformed", () => {
    beforeEach(async () => {
      result = await run("not-a-uuid");
    });

    it("should throw", () => {
      expect(result.ok).toBe(false);
    });

    it("should answer 404", () => {
      expect(failure(result).init?.status).toBe(404);
    });

    it("should not read the social API", () => {
      expect(calls).toEqual([]);
    });
  });

  describe("when the social API fails", () => {
    beforeEach(async () => {
      reply(/\/v1\/communities\//, { status: 502 });

      result = await run(ID);
    });

    it("should throw", () => {
      expect(result.ok).toBe(false);
    });

    it("should answer 503", () => {
      expect(failure(result).init?.status).toBe(503);
    });

    it("should give unavailable as the reason", () => {
      expect(failure(result).data).toEqual({ reason: "unavailable" });
    });
  });

  describe("when the social API is unreachable", () => {
    beforeEach(async () => {
      reply(/\/v1\/communities\//, new Error("connect ECONNREFUSED"));

      result = await run(ID);
    });

    it("should throw", () => {
      expect(result.ok).toBe(false);
    });

    it("should answer 503", () => {
      expect(failure(result).init?.status).toBe(503);
    });
  });

  describe("when the community reply does not parse", () => {
    beforeEach(async () => {
      reply(/\/v1\/communities\//, { status: 200, body: { data: { id: ID } } });

      result = await run(ID);
    });

    it("should throw", () => {
      expect(result.ok).toBe(false);
    });

    it("should answer 503", () => {
      expect(failure(result).init?.status).toBe(503);
    });
  });
});
