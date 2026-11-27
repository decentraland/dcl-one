import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { CatalystError } from "../client";
import {
  fetchCommunityRequests,
  fetchHubCommunities,
  fetchHubDetail,
  leaveCommunity,
  publishPost,
} from "./community-hub";

vi.mock("../../auth/signer", () => ({
  signedFetch: (_identity: unknown, url: string, init: RequestInit) => fetch(url, init),
}));

const identity = {
  signer: "0xABC",
  ephemeral: { address: "0x0", privateKey: "0x00" as const },
  expiration: "",
  authChain: [] as never[],
};
const community = { id: "club", name: "Music Club", privacy: "public", membersCount: 4, role: "member" };

type Reply = { status?: number; body: unknown };
let replies: Map<string, Reply>;
const calls = () => vi.mocked(fetch).mock.calls.map(([url, init]) => ({ url: String(url), init: init as RequestInit }));

beforeEach(() => {
  replies = new Map();
  vi.stubGlobal(
    "fetch",
    vi.fn(async (url: string) => {
      const path = new URL(url).pathname + new URL(url).search;
      const match = [...replies.entries()].find(([prefix]) => path.startsWith(prefix));
      const reply = match?.[1] ?? { status: 404, body: { message: "Not found" } };
      return new Response(JSON.stringify(reply.body), { status: reply.status ?? 200 });
    }),
  );
});
afterEach(() => {
  vi.resetAllMocks();
  vi.unstubAllGlobals();
});

describe("fetchHubCommunities", () => {
  it("should keep the valid rows and count every row the page held", async () => {
    replies.set("/v1/communities?", { body: { data: { results: [community, { id: 7 }], total: 3 } } });
    const page = await fetchHubCommunities({ identity: null, offset: 0 });
    expect(page).toEqual({ rows: [expect.objectContaining({ id: "club", role: "member" })], total: 3, received: 2 });
  });

  it("should read a search from the offset given", async () => {
    replies.set("/v1/communities?", { body: { data: { results: [], total: 0 } } });
    await fetchHubCommunities({ identity: null, offset: 100, search: "río club" });
    expect(calls()[0]?.url).toContain("/v1/communities?limit=100&offset=100&search=r%C3%ADo%20club");
  });

  it("should read a visitor without a role", async () => {
    replies.set("/v1/communities?", { body: { data: { results: [{ ...community, role: "none" }], total: 1 } } });
    const page = await fetchHubCommunities({ identity: null, offset: 0 });
    expect(page.rows[0]?.role).toBeNull();
  });

  it("should refuse a list it cannot read", async () => {
    replies.set("/v1/communities?", { body: { data: { nothing: true } } });
    await expect(fetchHubCommunities({ identity: null, offset: 0 })).rejects.toBeInstanceOf(CatalystError);
  });
});

describe("fetchHubDetail", () => {
  it("should read the community with its members and shared places", async () => {
    replies.set("/v1/communities/club/members", { body: { data: { results: [{ memberAddress: "0x1", role: "Owner" }], total: 1 } } });
    replies.set("/v1/communities/club/places", {
      body: {
        data: {
          results: [
            { id: "plaza", title: "Plaza", base_position: "-66,-56", world: false, world_name: null },
            { id: "hall", title: "Hall", base_position: "0,0", world: true, world_name: "hall.dcl.eth" },
          ],
        },
      },
    });
    replies.set("/v1/communities/club", { body: { data: community } });
    const detail = await fetchHubDetail("club", { identity: null });
    expect(detail.community?.name).toBe("Music Club");
    expect(detail.members).toEqual([expect.objectContaining({ memberAddress: "0x1", role: "owner" })]);
    expect(detail.places.map((place) => [place.id, place.location, place.addedBy])).toEqual([
      ["plaza", "-66,-56", null],
      ["hall", "hall.dcl.eth", null],
    ]);
  });

  it("should fall back to the discovery row when the full read is refused, and read no members of a private community", async () => {
    replies.set("/v1/communities/club", { status: 403, body: { message: "Members only" } });
    replies.set("/v1/communities?communityId=club", {
      body: { data: { results: [{ ...community, privacy: "private", role: null }], total: 1 } },
    });
    const detail = await fetchHubDetail("club", { identity: null });
    expect(detail).toEqual({ id: "club", community: expect.objectContaining({ privacy: "private" }), members: [], places: [] });
    expect(calls().some(({ url }) => url.includes("/members"))).toBe(false);
  });

  it("should open nothing when the discovery row is missing too", async () => {
    replies.set("/v1/communities/club", { status: 404, body: {} });
    replies.set("/v1/communities?communityId=club", { body: { data: { results: [], total: 0 } } });
    expect((await fetchHubDetail("club", { identity: null })).community).toBeNull();
  });

  it("should keep any other failure", async () => {
    replies.set("/v1/communities/club", { status: 500, body: {} });
    await expect(fetchHubDetail("club", { identity: null })).rejects.toMatchObject({ status: 500 });
  });
});

describe("fetchCommunityRequests", () => {
  it("should read every page until the total", async () => {
    let served = 0;
    vi.mocked(fetch).mockImplementation(async () => {
      served += 1;
      const results = served === 1 ? Array.from({ length: 100 }, (_, i) => ({ id: `r${i}`, communityId: "club", type: "invite", status: "pending" })) : [{ id: "last", communityId: "club", type: "request_to_join", status: "pending" }];
      return new Response(JSON.stringify({ data: { results, total: 101 } }));
    });
    const rows = await fetchCommunityRequests(identity, "club");
    expect(rows).toHaveLength(101);
    expect(calls().map(({ url }) => new URL(url).search)).toEqual(["?limit=100&offset=0", "?limit=100&offset=100"]);
  });

  it("should stop on an empty page", async () => {
    vi.mocked(fetch).mockImplementation(async () => new Response(JSON.stringify({ data: { results: [], total: 50 } })));
    expect(await fetchCommunityRequests(identity, "club")).toEqual([]);
    expect(calls()).toHaveLength(1);
  });
});

describe("the hub's writes", () => {
  it("should leave as the signed-in member", async () => {
    replies.set("/v1/communities/club/members/0xabc", { body: {} });
    await leaveCommunity(identity, "club");
    expect(calls()[0]).toMatchObject({ url: expect.stringContaining("/v1/communities/club/members/0xabc"), init: { method: "DELETE" } });
  });

  it("should publish with the idempotency key", async () => {
    replies.set("/v1/communities/club/posts", { body: {} });
    await publishPost(identity, "club", "Hello", "key-1");
    const init = calls()[0]!.init;
    expect(init.body).toBe(JSON.stringify({ content: "Hello" }));
    expect((init.headers as Record<string, string>)["Idempotency-Key"]).toBe("key-1");
  });
});
