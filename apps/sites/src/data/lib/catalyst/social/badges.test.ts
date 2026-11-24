import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { fetchUserBadges, tierDescription } from "./badges";

const A = "0x4E9C4A2502FDF71E93ED8ED6CA9DDBD891D6F295";
const json = (body: unknown, status = 200) => new Response(JSON.stringify(body), { status });

describe("fetchUserBadges", () => {
  afterEach(() => {
    vi.resetAllMocks();
    vi.unstubAllGlobals();
  });

  describe("when the member has achieved badges", () => {
    let fetchMock: ReturnType<typeof vi.fn>;
    let badges: Awaited<ReturnType<typeof fetchUserBadges>>;

    beforeEach(async () => {
      fetchMock = vi.fn().mockResolvedValueOnce(
        json({
          data: {
            achieved: [
              {
                id: "walker",
                name: "Walker",
                description: "Walk 10 km",
                completedAt: "1772496000000",
                assets: { "2d": { normal: "https://example.com/walker.png" } },
                progress: {},
              },
              {
                id: "explorer",
                name: "Explorer",
                description: "Bronze: 10 scenes;Gold: 100 scenes",
                completedAt: null,
                progress: { lastCompletedTierName: "Gold", lastCompletedTierImage: "https://example.com/gold.png", lastCompletedTierAt: 1772582400000 },
              },
            ],
            notAchieved: [{ id: "x", name: "Not yet", progress: {} }],
          },
        }),
      );
      vi.stubGlobal("fetch", fetchMock);
      badges = await fetchUserBadges(A);
    });

    it("should read the lowercased member from the node's badges mount", () => {
      expect(new URL(fetchMock.mock.calls[0][0]).pathname).toBe(`/badges/users/${A.toLowerCase()}/badges`);
    });

    it("should return only the achieved badges", () => {
      expect(badges).toEqual([
        { id: "walker", name: "Walker", imageUrl: "https://example.com/walker.png", tierName: undefined, description: "Walk 10 km", unlockedAt: 1772496000000 },
        { id: "explorer", name: "Explorer", imageUrl: "https://example.com/gold.png", tierName: "Gold", description: "100 scenes", unlockedAt: 1772582400000 },
      ]);
    });
  });

  describe("when the member has no badges", () => {
    beforeEach(() => {
      vi.stubGlobal("fetch", vi.fn().mockResolvedValueOnce(json({ data: { achieved: [], notAchieved: [] } })));
    });

    it("should read them as an empty list", async () => {
      await expect(fetchUserBadges(A)).resolves.toEqual([]);
    });
  });

  describe("when the reply has no data", () => {
    beforeEach(() => {
      vi.stubGlobal("fetch", vi.fn().mockResolvedValueOnce(json({ badges: [] })));
    });

    it("should fail", async () => {
      await expect(fetchUserBadges(A)).rejects.toThrow(/data/);
    });
  });

  describe("when the reply has no achieved list", () => {
    beforeEach(() => {
      vi.stubGlobal("fetch", vi.fn().mockResolvedValueOnce(json({ data: { notAchieved: [] } })));
    });

    it("should fail", async () => {
      await expect(fetchUserBadges(A)).rejects.toThrow(/achieved/);
    });
  });

  describe("when the server answers an error", () => {
    beforeEach(() => {
      vi.stubGlobal("fetch", vi.fn().mockResolvedValueOnce(json({ error: "boom" }, 500)));
    });

    it("should fail with the status", async () => {
      await expect(fetchUserBadges(A)).rejects.toThrow("Catalyst returned 500");
    });
  });
});

describe("tierDescription", () => {
  describe("when the reached tier has a blurb", () => {
    it("should keep only that tier's blurb", () => {
      expect(tierDescription("Bronze: 10;Silver: 50", "silver")).toBe("50");
    });
  });

  describe("when no tier matches", () => {
    it("should keep the whole text", () => {
      expect(tierDescription("Bronze: 10;Silver: 50", "Gold")).toBe("Bronze: 10;Silver: 50");
    });
  });

  describe("when no tier was reached", () => {
    it("should keep the whole text", () => {
      expect(tierDescription("Walk 10 km", undefined)).toBe("Walk 10 km");
    });
  });

  describe("when there is no description", () => {
    it("should return undefined", () => {
      expect(tierDescription(undefined, "Gold")).toBeUndefined();
    });
  });
});
