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
let routes: { match: RegExp; reply: Reply }[];
const calls: string[] = [];

beforeEach(() => {
  calls.length = 0;
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
afterEach(() => vi.unstubAllGlobals());

const reply = (match: RegExp, r: Reply) => routes.push({ match, reply: r });

async function run(id: string) {
  try {
    const result = (await communityLoader(new Request(`http://localhost/social/communities/${id}`), new RouterContextProvider(), {
      id,
    })) as unknown as { data: CommunityLoaderData };
    return { ok: true as const, data: result.data };
  } catch (thrown) {
    return { ok: false as const, thrown: thrown as { init?: { status?: number }; data?: unknown } };
  }
}

const keys = (data: CommunityLoaderData) => data.dehydratedState.queries.map((q) => JSON.stringify(q.queryKey));

describe("communityLoader", () => {
  it("renders a public community with its first members and events page", async () => {
    reply(/\/v1\/communities\/[^/?]+\/members/, { status: 200, body: members });
    reply(/\/v1\/communities\/[^/?]+$/, { status: 200, body: community("public") });
    reply(/\/events\?|\/v1\/events/, { status: 200, body: events });

    const result = await run(ID);

    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.data.title).toBe("Decentraland Foundation");
    expect(result.data.description).toBe("Official updates.");
    expect(keys(result.data)).toEqual(
      expect.arrayContaining([
        JSON.stringify(communityKeys.detail(ID)),
        JSON.stringify(communityKeys.members(ID)),
        JSON.stringify(communityKeys.events(ID)),
      ]),
    );
    expect(calls.some((url) => url.includes(`community_id=${ID}`))).toBe(true);
  });

  it("does not read a private community's members or events on the server", async () => {
    reply(/\/v1\/communities\/[^/?]+$/, { status: 200, body: community("private") });

    const result = await run(ID);

    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(keys(result.data)).toEqual([JSON.stringify(communityKeys.detail(ID))]);
    expect(calls).toHaveLength(1);
  });

  it("still renders when the optional members and events reads fail", async () => {
    reply(/\/v1\/communities\/[^/?]+\/members/, { status: 500 });
    reply(/\/v1\/communities\/[^/?]+$/, { status: 200, body: community("public") });
    reply(/\/events\?|\/v1\/events/, new Error("socket hang up"));

    const result = await run(ID);

    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(keys(result.data)).toEqual([JSON.stringify(communityKeys.detail(ID))]);
  });

  it("answers 404 for an id the social API does not know", async () => {
    reply(/\/v1\/communities\//, { status: 404, body: { error: "Not Found" } });

    const result = await run(ID);

    expect(result.ok).toBe(false);
    if (result.ok) return;
    expect(result.thrown.init?.status).toBe(404);
    expect(result.thrown.data).toEqual({ reason: "not-found" });
  });

  it("answers 404 for a malformed id without reading the social API", async () => {
    const result = await run("not-a-uuid");

    expect(result.ok).toBe(false);
    if (result.ok) return;
    expect(result.thrown.init?.status).toBe(404);
    expect(calls).toEqual([]);
  });

  it("answers 503 when the social API fails", async () => {
    reply(/\/v1\/communities\//, { status: 502 });

    const result = await run(ID);

    expect(result.ok).toBe(false);
    if (result.ok) return;
    expect(result.thrown.init?.status).toBe(503);
    expect(result.thrown.data).toEqual({ reason: "unavailable" });
  });

  it("answers 503 when the social API is unreachable", async () => {
    reply(/\/v1\/communities\//, new Error("connect ECONNREFUSED"));

    const result = await run(ID);

    expect(result.ok).toBe(false);
    if (result.ok) return;
    expect(result.thrown.init?.status).toBe(503);
  });

  it("answers 503 when the community reply does not parse", async () => {
    reply(/\/v1\/communities\//, { status: 200, body: { data: { id: ID } } });

    const result = await run(ID);

    expect(result.ok).toBe(false);
    if (result.ok) return;
    expect(result.thrown.init?.status).toBe(503);
  });
});
