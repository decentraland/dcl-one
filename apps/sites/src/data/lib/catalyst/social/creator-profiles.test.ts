import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { creatorBatches, creatorProfilesQuery, fetchCreatorProfiles } from "./creator-profiles";

const A = "0x" + "a".repeat(40);
const B = "0x" + "b".repeat(40);
const C = "0x" + "c".repeat(40);

describe("creatorBatches", () => {
  describe("when groups repeat wallets in different cases", () => {
    it("should give each group only the wallets no earlier group asked for, lowercased", () => {
      expect(creatorBatches([[A, B.toUpperCase().replace("0X", "0x")], [B, C], [A]])).toEqual([[A, B], [C]]);
    });
  });

  describe("when groups hold values that are not wallets", () => {
    it("should drop them and leave out empty groups", () => {
      expect(creatorBatches([["Decentraland Foundation", null, undefined, ""], [A]])).toEqual([[A]]);
    });
  });
});

describe("creatorProfilesQuery", () => {
  describe("when the same wallets come in any order or case", () => {
    it("should share one entry", () => {
      expect(creatorProfilesQuery([B, A.toUpperCase().replace("0X", "0x"), A]).queryKey).toEqual(
        creatorProfilesQuery([A, B]).queryKey,
      );
    });
  });
});

describe("fetchCreatorProfiles", () => {
  afterEach(() => {
    vi.resetAllMocks();
    vi.unstubAllGlobals();
  });

  describe("when reading a batch", () => {
    let fetchMock: ReturnType<typeof vi.fn>;
    let profiles: Awaited<ReturnType<typeof fetchCreatorProfiles>>;

    beforeEach(async () => {
      fetchMock = vi.fn(async () =>
        Response.json([
          {
            avatars: [
              {
                ethAddress: A.toUpperCase().replace("0X", "0x"),
                name: "Ana",
                avatar: { snapshots: { face256: "https://profile-images.decentraland.org/entities/bafyface/face.png" } },
              },
            ],
          },
          { avatars: [] },
        ]),
      );
      vi.stubGlobal("fetch", fetchMock);
      profiles = await fetchCreatorProfiles([A, B]);
    });

    it("should read the whole batch with one request", () => {
      expect(fetchMock).toHaveBeenCalledOnce();
    });

    it("should POST the batch's wallets to the profiles lambda", () => {
      const [url, init] = fetchMock.mock.calls[0] as unknown as [string, RequestInit];
      expect(url).toMatch(/\/lambdas\/profiles$/);
      expect(JSON.parse(String(init.body))).toEqual({ ids: [A, B] });
    });

    it("should keep each wallet's name", () => {
      expect(profiles[A]).toMatchObject({ name: "Ana" });
    });

    it("should keep each wallet's face through the local image path", () => {
      expect(profiles[A]?.avatarUrl).toMatch(/\/profile-images\/entities\/bafyface\/face\.png$/);
    });

    it("should leave out a wallet without a profile", () => {
      expect(profiles[B]).toBeUndefined();
    });
  });

  describe("when the batch is empty", () => {
    let fetchMock: ReturnType<typeof vi.fn>;

    beforeEach(() => {
      fetchMock = vi.fn();
      vi.stubGlobal("fetch", fetchMock);
    });

    it("should resolve to no profiles", async () => {
      await expect(fetchCreatorProfiles([])).resolves.toEqual({});
    });

    it("should not read anything", async () => {
      await fetchCreatorProfiles([]);
      expect(fetchMock).not.toHaveBeenCalled();
    });
  });

  describe("when the upstream fails", () => {
    beforeEach(() => {
      vi.stubGlobal(
        "fetch",
        vi.fn(async () => new Response("down", { status: 502 })),
      );
    });

    it("should fail rather than report no faces", async () => {
      await expect(fetchCreatorProfiles([A])).rejects.toThrow("502");
    });
  });
});
