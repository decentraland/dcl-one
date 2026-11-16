import { afterEach, describe, expect, it, vi } from "vitest";

import { fetchUserBadges, tierDescription } from "./badges";

const A = "0x4E9C4A2502FDF71E93ED8ED6CA9DDBD891D6F295";
const json = (body: unknown, status = 200) => new Response(JSON.stringify(body), { status });

afterEach(() => vi.unstubAllGlobals());

describe("fetchUserBadges", () => {
  it("reads the lowercased member's achieved badges from the node's badges mount", async () => {
    const fetchMock = vi.fn().mockResolvedValue(
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

    const badges = await fetchUserBadges(A);

    expect(new URL(fetchMock.mock.calls[0][0]).pathname).toBe(`/badges/users/${A.toLowerCase()}/badges`);
    expect(badges).toEqual([
      { id: "walker", name: "Walker", imageUrl: "https://example.com/walker.png", tierName: undefined, description: "Walk 10 km", unlockedAt: 1772496000000 },
      { id: "explorer", name: "Explorer", imageUrl: "https://example.com/gold.png", tierName: "Gold", description: "100 scenes", unlockedAt: 1772582400000 },
    ]);
  });

  it("reads a member with no badges as an empty list", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(json({ data: { achieved: [], notAchieved: [] } })));

    await expect(fetchUserBadges(A)).resolves.toEqual([]);
  });

  it("fails on an unreadable reply or a server error", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(json({ badges: [] })));
    await expect(fetchUserBadges(A)).rejects.toThrow();

    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(json({ data: { notAchieved: [] } })));
    await expect(fetchUserBadges(A)).rejects.toThrow();

    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(json({ error: "boom" }, 500)));
    await expect(fetchUserBadges(A)).rejects.toThrow();
  });
});

describe("tierDescription", () => {
  it("keeps only the reached tier's blurb, or the whole text without a match", () => {
    expect(tierDescription("Bronze: 10;Silver: 50", "silver")).toBe("50");
    expect(tierDescription("Bronze: 10;Silver: 50", "Gold")).toBe("Bronze: 10;Silver: 50");
    expect(tierDescription("Walk 10 km", undefined)).toBe("Walk 10 km");
    expect(tierDescription(undefined, "Gold")).toBeUndefined();
  });
});
