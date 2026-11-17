import { describe, expect, it } from "vitest";

import { communityRows, effectiveCommunitySearch, nextCommunitiesOffset } from "./communities-browse";
import type { CommunitiesPage } from "./communities-list";

describe("effectiveCommunitySearch", () => {
  it("should drop a search shorter than three characters", () => {
    expect(effectiveCommunitySearch("ab")).toBe("");
    expect(effectiveCommunitySearch("  ab  ")).toBe("");
  });

  it("should keep a trimmed search of three characters or more", () => {
    expect(effectiveCommunitySearch("  abc ")).toBe("abc");
  });

  it("should treat a missing search as none", () => {
    expect(effectiveCommunitySearch(null)).toBe("");
    expect(effectiveCommunitySearch(undefined)).toBe("");
  });
});

describe("nextCommunitiesOffset", () => {
  const page = (received: number, total: number): CommunitiesPage => ({ rows: [], received, total });

  it("should continue after a full page while the total has more", () => {
    expect(nextCommunitiesOffset(page(24, 100), 0)).toBe(24);
  });

  it("should stop after a short page even when the total says otherwise", () => {
    expect(nextCommunitiesOffset(page(10, 100), 24)).toBeUndefined();
  });

  it("should stop once the total is reached", () => {
    expect(nextCommunitiesOffset(page(24, 48), 24)).toBeUndefined();
  });
});

describe("communityRows", () => {
  it("should keep a community repeated by the next page once", () => {
    const community = { id: "a", name: "A", description: null, ownerName: null, privacy: "public" as const, membersCount: 1, thumbnailUrl: null };
    const rows = communityRows([
      { rows: [community], received: 1, total: 2 },
      { rows: [community, { ...community, id: "b" }], received: 2, total: 2 },
    ]);
    expect(rows.map((r) => r.id)).toEqual(["a", "b"]);
  });

  it("should list nothing for no pages", () => {
    expect(communityRows([])).toEqual([]);
  });
});
