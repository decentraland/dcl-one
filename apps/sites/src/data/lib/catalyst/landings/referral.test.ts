import { beforeEach, describe, expect, it, vi } from "vitest";

import { getJSON } from "../client";
import { fetchReferrerProfile, referrerFromAddress } from "./referral";

vi.mock("../client", async (actual) => ({ ...(await actual<typeof import("../client")>()), getJSON: vi.fn() }));

const ADDRESS = "0x0000000000000000000000000000000000000abc";
const FACE = "https://profile-images.decentraland.org/entities/bafkreiexample/face.png";

const profile = (face256: unknown) => ({
  avatars: [{ name: "Jane", hasClaimedName: true, avatar: { snapshots: face256 === undefined ? {} : { face256 } } }],
});

describe("when fetching a referrer's profile", () => {
  beforeEach(() => vi.mocked(getJSON).mockReset());

  describe("and the avatar has an https face snapshot", () => {
    it("should carry the face", async () => {
      vi.mocked(getJSON).mockResolvedValue(profile(FACE));
      expect(await fetchReferrerProfile(ADDRESS)).toEqual({
        name: "Jane",
        address: ADDRESS,
        hasClaimedName: true,
        resolved: true,
        face256: FACE,
      });
    });
  });

  describe.each([
    ["missing", undefined],
    ["null", null],
    ["a bare content hash", "bafkreiexample"],
    ["plain http", "http://profile-images.decentraland.org/entities/x/face.png"],
  ])("and the face snapshot is %s", (_label, face) => {
    it("should carry no face", async () => {
      vi.mocked(getJSON).mockResolvedValue(profile(face));
      expect((await fetchReferrerProfile(ADDRESS)).face256).toBeNull();
    });
  });

  describe("and the profile has no avatar", () => {
    it("should be unresolved and carry no face", async () => {
      vi.mocked(getJSON).mockResolvedValue({ avatars: [] });
      expect(await fetchReferrerProfile(ADDRESS)).toMatchObject({ resolved: false, face256: null });
    });
  });
});

describe("when only the address is known", () => {
  it("should carry no face", () => {
    expect(referrerFromAddress(ADDRESS).face256).toBeNull();
  });
});
