import { beforeEach, describe, expect, it, vi } from "vitest";

import { CatalystError, getJSON } from "../client";
import { fetchReferrerProfile, referrerFromAddress, resolveReferrerAddress } from "./referral";

vi.mock("../client", async (actual) => ({ ...(await actual<typeof import("../client")>()), getJSON: vi.fn() }));

const ADDRESS = "0x0000000000000000000000000000000000000abc";
const FACE = "https://profile-images.decentraland.org/entities/bafkreiexample/face.png";

const profile = (face256: unknown) => ({
  avatars: [{ name: "Jane", hasClaimedName: true, avatar: { snapshots: face256 === undefined ? {} : { face256 } } }],
});

describe("when fetching a referrer's profile", () => {
  beforeEach(() => vi.mocked(getJSON).mockReset());

  describe("and the avatar has a face snapshot", () => {
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
    ["blank", "  "],
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

describe("when resolving an invite's referrer", () => {
  beforeEach(() => {
    vi.mocked(getJSON).mockReset();
  });

  describe("and it is an address", () => {
    it("should lowercase it without a NAME lookup", async () => {
      expect(await resolveReferrerAddress("0x0000000000000000000000000000000000000ABC")).toBe(ADDRESS);
      expect(getJSON).not.toHaveBeenCalled();
    });
  });

  describe("and it is a NAME with an owner", () => {
    it("should return the owner, lowercased", async () => {
      vi.mocked(getJSON).mockResolvedValue({ owner: ADDRESS.toUpperCase().replace("0X", "0x") });
      expect(await resolveReferrerAddress("Jane")).toBe(ADDRESS);
    });
  });

  describe("and the NAME has no owner", () => {
    it("should return none", async () => {
      vi.mocked(getJSON).mockRejectedValue(new CatalystError("Catalyst returned 404 Not Found", "https://node.test", 404));
      expect(await resolveReferrerAddress("nobody")).toBeNull();
    });
  });

  describe.each([
    ["the node answers 503", new CatalystError("Catalyst returned 503", "https://node.test", 503)],
    ["the request fails", new CatalystError("Catalyst request failed: network error", "https://node.test")],
  ])("and %s", (_label, error) => {
    it("should throw, so the invite is not taken for unknown", async () => {
      vi.mocked(getJSON).mockRejectedValue(error);
      await expect(resolveReferrerAddress("Jane")).rejects.toBe(error);
    });
  });
});
