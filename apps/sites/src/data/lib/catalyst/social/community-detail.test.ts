import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { CatalystError } from "../client";
import {
  CommunityNotFoundError,
  fetchCommunity,
  fetchCommunityEvents,
  fetchCommunityMembers,
  fetchJoinRequests,
  requestToJoin,
} from "./community-detail";

vi.mock("../../auth/signer", () => ({
  signedFetch: (_identity: unknown, url: string, init: RequestInit) => fetch(url, init),
}));

const ID = "e99471aa-31c4-4952-abf6-99905445f43b";
const identity = { signer: "0xABC", ephemeral: { address: "0x0", privateKey: "0x00" as const }, expiration: "", authChain: [] };
let body: unknown;
let status: number;

beforeEach(() => {
  status = 200;
  vi.stubGlobal("fetch", vi.fn(async () => new Response(JSON.stringify(body), { status })));
});
afterEach(() => vi.unstubAllGlobals());

const row = (memberAddress: string, extra: Record<string, unknown> = {}) => ({ memberAddress, role: "member", ...extra });

describe("fetchCommunity", () => {
  it("treats a refused id (400) as not found", async () => {
    status = 400;
    body = { error: "Bad request" };
    await expect(fetchCommunity(ID)).rejects.toBeInstanceOf(CommunityNotFoundError);
  });

  it("keeps other failures as unavailable", async () => {
    status = 500;
    body = {};
    await expect(fetchCommunity(ID)).rejects.toBeInstanceOf(CatalystError);
  });

  it("reads the member's role when signed", async () => {
    body = { data: { id: ID, name: "N", ownerAddress: "0x1", privacy: "private", membersCount: 3, role: "moderator" } };
    expect((await fetchCommunity(ID, { identity })).role).toBe("moderator");
  });

  it("drops a role it does not know instead of failing", async () => {
    body = { data: { id: ID, name: "N", ownerAddress: "0x1", privacy: "public", membersCount: 3, role: "superuser" } };
    expect((await fetchCommunity(ID)).role).toBeNull();
  });
});

describe("fetchCommunityMembers", () => {
  it("keeps valid rows and drops malformed ones", async () => {
    body = { data: { results: [row("0x1"), { role: "member" }, row("0x2", { name: "Bob" })], total: 3, page: 1, pages: 1 } };
    const page = await fetchCommunityMembers(ID, 0);
    expect(page.members.map((m) => m.memberAddress)).toEqual(["0x1", "0x2"]);
    expect(page.hasMore).toBe(false);
  });

  it("uses page and pages when given", async () => {
    body = { data: { results: [row("0x1")], total: 30, page: 1, pages: 3 } };
    expect((await fetchCommunityMembers(ID, 0)).hasMore).toBe(true);
  });

  it("falls back to the total when page and pages are missing", async () => {
    body = { data: { results: [row("0x1")], total: 15 } };
    expect((await fetchCommunityMembers(ID, 0)).hasMore).toBe(true);
    expect((await fetchCommunityMembers(ID, 10)).hasMore).toBe(false);
  });

  it("fails on an envelope it cannot read", async () => {
    body = { results: [] };
    await expect(fetchCommunityMembers(ID, 0)).rejects.toBeInstanceOf(CatalystError);
  });
});

describe("member requests", () => {
  it("keeps only pending requests to join", async () => {
    body = {
      data: {
        results: [
          { id: "a", communityId: ID, type: "request_to_join", status: "pending" },
          { id: "b", communityId: ID, type: "invite", status: "pending" },
          { id: "c", communityId: ID, type: "request_to_join", status: "cancelled" },
          { id: "d" },
        ],
      },
    };
    expect((await fetchJoinRequests(identity)).map((r) => r.id)).toEqual(["a"]);
  });

  it("throws when a new request's reply does not parse", async () => {
    body = { data: { id: "a" } };
    await expect(requestToJoin(identity, ID)).rejects.toBeInstanceOf(CatalystError);
  });

  it("sends the member's own address", async () => {
    body = { data: { id: "a", communityId: ID, type: "request_to_join", status: "pending" } };
    await requestToJoin(identity, ID);
    const init = vi.mocked(fetch).mock.calls[0]![1] as RequestInit;
    expect(JSON.parse(String(init.body))).toEqual({ targetedAddress: "0xabc", type: "request_to_join" });
  });
});

describe("fetchCommunityEvents", () => {
  it("asks for the community's events with no list filter, as the source does (upcoming and in progress)", async () => {
    body = { ok: true, data: { events: [], total: 0 } };
    await fetchCommunityEvents(ID, 12);
    const url = new URL(String(vi.mocked(fetch).mock.calls[0]![0]));
    expect(url.searchParams.get("community_id")).toBe(ID);
    expect(url.searchParams.get("offset")).toBe("12");
    expect(url.searchParams.has("list")).toBe(false);
  });
});
