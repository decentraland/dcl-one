import { afterEach, describe, expect, it, vi } from "vitest";

import { creatorBatches, creatorProfilesQuery, fetchCreatorProfiles } from "./creator-profiles";

const A = "0x" + "a".repeat(40);
const B = "0x" + "b".repeat(40);
const C = "0x" + "c".repeat(40);

afterEach(() => vi.unstubAllGlobals());

describe("creatorBatches", () => {
  it("should give each group only the wallets no earlier group asked for, lowercased", () => {
    expect(creatorBatches([[A, B.toUpperCase().replace("0X", "0x")], [B, C], [A]])).toEqual([[A, B], [C]]);
  });

  it("should drop values that are not wallets and leave out empty groups", () => {
    expect(creatorBatches([["Decentraland Foundation", null, undefined, ""], [A]])).toEqual([[A]]);
  });
});

describe("creatorProfilesQuery", () => {
  it("should share one entry for the same wallets in any order or case", () => {
    expect(creatorProfilesQuery([B, A.toUpperCase().replace("0X", "0x"), A]).queryKey).toEqual(
      creatorProfilesQuery([A, B]).queryKey,
    );
  });
});

describe("fetchCreatorProfiles", () => {
  it("should read a whole batch with one POST and keep each wallet's name and face", async () => {
    const fetchMock = vi.fn(async () =>
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
    const profiles = await fetchCreatorProfiles([A, B]);
    expect(fetchMock).toHaveBeenCalledOnce();
    const [url, init] = fetchMock.mock.calls[0] as unknown as [string, RequestInit];
    expect(url).toMatch(/\/lambdas\/profiles$/);
    expect(JSON.parse(String(init.body))).toEqual({ ids: [A, B] });
    expect(profiles[A]).toMatchObject({ name: "Ana" });
    expect(profiles[A]?.avatarUrl).toMatch(/\/profile-images\/entities\/bafyface\/face\.png$/);
    expect(profiles[B]).toBeUndefined();
  });

  it("should not read anything for an empty batch", async () => {
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);
    await expect(fetchCreatorProfiles([])).resolves.toEqual({});
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("should fail on an upstream error rather than report no faces", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => new Response("down", { status: 502 })),
    );
    await expect(fetchCreatorProfiles([A])).rejects.toThrow("502");
  });
});
