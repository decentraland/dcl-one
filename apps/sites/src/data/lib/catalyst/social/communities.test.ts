import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("../client", async (original) => ({
  ...(await original<typeof import("../client")>()),
  getJSON: vi.fn(),
}));

import { getJSON } from "../client";
import { fetchCommunityDirectory } from "./communities";

const row = (id: string, membersCount: number | null, extra: Record<string, unknown> = {}) => ({
  id,
  name: `Community ${id}`,
  description: "About",
  ownerAddress: "0x1",
  ownerName: "Owner",
  privacy: "public",
  membersCount,
  ...extra,
});

afterEach(() => {
  vi.resetAllMocks();
});

describe("fetchCommunityDirectory", () => {
  it("should read one page of 200 communities, without a search when none is typed", async () => {
    vi.mocked(getJSON).mockResolvedValue({ data: { results: [], total: 0 } });
    await fetchCommunityDirectory("  ");
    expect(getJSON).toHaveBeenCalledWith("/v1/communities", expect.objectContaining({ query: { limit: 200, offset: 0 } }));
  });

  it("should pass the trimmed search", async () => {
    vi.mocked(getJSON).mockResolvedValue({ data: { results: [], total: 0 } });
    await fetchCommunityDirectory(" club ");
    expect(getJSON).toHaveBeenCalledWith(
      "/v1/communities",
      expect.objectContaining({ query: { limit: 200, offset: 0, search: "club" } }),
    );
  });

  it("should put the most members first, keep the read order among equals and count a missing count as zero", async () => {
    vi.mocked(getJSON).mockResolvedValue({ data: { results: [row("a", 3), row("b", null), row("c", 10), row("d", 3)] } });
    expect((await fetchCommunityDirectory("")).map((c) => c.id)).toEqual(["c", "a", "d", "b"]);
  });

  it("should leave out a row that is not a community, and read a row without its privacy as public", async () => {
    vi.mocked(getJSON).mockResolvedValue({ data: { results: [{ id: 1 }, row("a", 1, { privacy: undefined })] } });
    expect(await fetchCommunityDirectory("")).toEqual([
      { id: "a", name: "Community a", description: "About", ownerName: "Owner", thumbnailUrl: null, membersCount: 1, privacy: "public" },
    ]);
  });

  it("should fail when the response is not a list", async () => {
    vi.mocked(getJSON).mockResolvedValue({ error: "nope" });
    await expect(fetchCommunityDirectory("")).rejects.toThrow("communities shape");
  });
});
