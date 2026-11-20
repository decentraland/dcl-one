import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { CatalystError } from "../client";
import { fetchMemberCommunities, fetchMyCommunities } from "./profile-communities";

vi.mock("../../auth/signer", () => ({
  signedFetch: (_identity: unknown, url: string, init: RequestInit) => fetch(url, { ...init, headers: { "x-signed": "1" } }),
}));

const ADDRESS = "0xBCAC4dAfB7e215f2F6cb3312aF6D5e4F9d9E7eDA";
const ID = "e99471aa-31c4-4952-abf6-99905445f43b";
const identity = { signer: ADDRESS, ephemeral: { address: "0x0", privateKey: "0x00" as const }, expiration: "", authChain: [] };
let body: unknown;
let status: number;
let fetchMock: ReturnType<typeof vi.fn>;

const envelope = (results: unknown[], total = results.length) => ({ data: { results, total, page: 1, pages: 1, limit: 100 } });
const request = () => {
  const [input, init] = fetchMock.mock.calls[0] as [string, RequestInit | undefined];
  return { url: new URL(input), signed: new Headers(init?.headers).has("x-signed") };
};

beforeEach(() => {
  status = 200;
  fetchMock = vi.fn(async () => new Response(JSON.stringify(body), { status }));
  vi.stubGlobal("fetch", fetchMock);
});
afterEach(() => vi.unstubAllGlobals());

describe("fetchMemberCommunities", () => {
  it("reads the member's list unsigned and maps the rows the social API gives: id, name and role", async () => {
    body = envelope(
      [
        { id: ID, name: "Decentraland Foundation", ownerAddress: ADDRESS.toLowerCase(), role: "owner" },
        { id: "c2", name: "Wardens", ownerAddress: "0x1", role: "moderator" },
        { id: "c3", name: "Explorers", ownerAddress: "0x1", role: "member" },
      ],
      24,
    );

    const page = await fetchMemberCommunities(ADDRESS);

    const { url, signed } = request();
    expect(url.pathname.endsWith(`/v1/members/${ADDRESS.toLowerCase()}/communities`)).toBe(true);
    expect(url.searchParams.get("limit")).toBe("100");
    expect(signed).toBe(false);
    expect(page.total).toBe(24);
    expect(page.communities).toEqual([
      { id: ID, name: "Decentraland Foundation", role: "owner", membersCount: null, privacy: null, thumbnailUrl: null },
      { id: "c2", name: "Wardens", role: "moderator", membersCount: null, privacy: null, thumbnailUrl: null },
      { id: "c3", name: "Explorers", role: "member", membersCount: null, privacy: null, thumbnailUrl: null },
    ]);
  });

  it("leaves out a row that is not a community and reads a role it does not know as a member's", async () => {
    body = envelope([{ id: ID, name: "  " }, { name: "No id" }, null, { id: "c2", name: "Explorers", role: "superuser" }]);

    const page = await fetchMemberCommunities(ADDRESS);

    expect(page.communities.map((community) => [community.id, community.role])).toEqual([["c2", "member"]]);
  });

  it("gives a member of nothing an empty list", async () => {
    body = envelope([]);

    expect(await fetchMemberCommunities(ADDRESS)).toEqual({ communities: [], total: 0 });
  });

  it("fails on a reply that is not the list, and on a refusal, instead of an empty list", async () => {
    body = { data: { results: "none" } };
    await expect(fetchMemberCommunities(ADDRESS)).rejects.toBeInstanceOf(CatalystError);

    status = 503;
    body = { error: "down" };
    await expect(fetchMemberCommunities(ADDRESS)).rejects.toBeInstanceOf(CatalystError);
  });
});

describe("fetchMyCommunities", () => {
  it("reads the signer's communities signed, with the members count, privacy and picture", async () => {
    body = envelope([
      {
        id: ID,
        name: "Builders",
        ownerAddress: ADDRESS.toLowerCase(),
        privacy: "private",
        visibility: "unlisted",
        membersCount: 12,
        role: "owner",
        thumbnailUrl: "https://assets-cdn.decentraland.org/social/communities/x/raw-thumbnail.png",
      },
      { id: "c2", name: "Explorers", ownerAddress: "0x1", privacy: "public", membersCount: 3, role: "member" },
    ]);

    const page = await fetchMyCommunities(identity);

    const { url, signed } = request();
    expect(url.pathname.endsWith("/v1/communities")).toBe(true);
    expect(Object.fromEntries(url.searchParams)).toEqual({ onlyMemberOf: "true", limit: "100" });
    expect(signed).toBe(true);
    expect(page.communities).toEqual([
      {
        id: ID,
        name: "Builders",
        role: "owner",
        membersCount: 12,
        privacy: "private",
        thumbnailUrl: "https://assets-cdn.decentraland.org/social/communities/x/raw-thumbnail.png",
      },
      { id: "c2", name: "Explorers", role: "member", membersCount: 3, privacy: "public", thumbnailUrl: null },
    ]);
  });

  it("fails when the node refuses the signature", async () => {
    status = 401;
    body = { error: "Unauthorized" };

    await expect(fetchMyCommunities(identity)).rejects.toBeInstanceOf(CatalystError);
  });
});
