import { afterEach, describe, expect, it, vi } from "vitest";

import { communityHref, communityThumbnailUrl, fetchCommunitiesPage } from "./communities-list";

const BASE = "http://node.test";

function answer(body: unknown, status = 200) {
  const fetchMock = vi.fn(async () => new Response(JSON.stringify(body), { status, statusText: status === 200 ? "OK" : "Bad Gateway" }));
  vi.stubGlobal("fetch", fetchMock);
  return fetchMock;
}

const row = (overrides: Record<string, unknown> = {}) => ({
  id: "c1",
  name: "Builders Guild",
  description: "We build things",
  ownerAddress: "0xowner",
  ownerName: "Jane",
  privacy: "public",
  active: true,
  membersCount: 42,
  ...overrides,
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("fetchCommunitiesPage", () => {
  it("should read the public list with the page and the search", async () => {
    const fetchMock = answer({ data: { results: [row()], total: 1, page: 1, pages: 1, limit: 24 } });
    await fetchCommunitiesPage({ offset: 24, search: "guild" }, { base: BASE });
    const url = new URL(String((fetchMock.mock.calls[0] as unknown as [string])[0]));
    expect(url.pathname).toBe("/v1/communities");
    expect(url.searchParams.get("limit")).toBe("24");
    expect(url.searchParams.get("offset")).toBe("24");
    expect(url.searchParams.get("search")).toBe("guild");
  });

  it("should not send an empty search", async () => {
    const fetchMock = answer({ data: { results: [], total: 0 } });
    await fetchCommunitiesPage({ offset: 0, search: "" }, { base: BASE });
    const url = new URL(String((fetchMock.mock.calls[0] as unknown as [string])[0]));
    expect(url.searchParams.has("search")).toBe(false);
  });

  it("should map each row and mark a private community", async () => {
    answer({ data: { results: [row(), row({ id: "c2", privacy: "private", ownerName: null, description: "", membersCount: undefined })], total: 2 } });
    const page = await fetchCommunitiesPage({ offset: 0, search: "" }, { base: BASE });
    expect(page).toEqual({
      rows: [
        { id: "c1", name: "Builders Guild", description: "We build things", ownerName: "Jane", privacy: "public", membersCount: 42 },
        { id: "c2", name: "Builders Guild", description: null, ownerName: null, privacy: "private", membersCount: null },
      ],
      received: 2,
      total: 2,
    });
  });

  it("should leave out a row without an id or a name but still count it as received", async () => {
    answer({ data: { results: [row({ name: "   " }), { name: "No id" }, row({ id: "ok" })], total: 3 } });
    const page = await fetchCommunitiesPage({ offset: 0, search: "" }, { base: BASE });
    expect(page.rows.map((community) => community.id)).toEqual(["ok"]);
    expect(page.received).toBe(3);
  });

  it("should fail on a body that is not a page", async () => {
    answer({ results: [] });
    await expect(fetchCommunitiesPage({ offset: 0, search: "" }, { base: BASE })).rejects.toThrow("unexpected community page");
  });

  it("should fail when the node answers an error", async () => {
    answer({ error: "boom" }, 502);
    await expect(fetchCommunitiesPage({ offset: 0, search: "" }, { base: BASE })).rejects.toThrow("Catalyst returned 502");
  });
});

describe("community links", () => {
  it("should point the cover at the node's thumbnail and encode the id", () => {
    expect(communityThumbnailUrl("a/b")).toBe("/social-api/social/communities/a%2Fb/raw-thumbnail.png");
  });

  it("should open the community page by id", () => {
    expect(communityHref("c 1")).toBe("/landings/community-detail?id=c%201");
  });
});
