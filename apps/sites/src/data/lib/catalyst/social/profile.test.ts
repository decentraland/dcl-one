import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
vi.mock("../client", async (original) => ({ ...await original<typeof import("../client")>(), getJSON: vi.fn() }));
import { getAvatarBackgroundColor, getDisplayName } from "@ui/shop/discovery/lib/avatarColor";

import { getJSON } from "../client";
import { fetchProfile, mapProfile } from "./profile";
import type { ProfileVM } from "./profile";

const ADDRESS = "0x1111111111111111111111111111111111111111";

const NULLABLE_FIELDS = [
  "nameColor",
  "links",
  "country",
  "gender",
  "pronouns",
  "relationshipStatus",
  "sexualOrientation",
  "language",
  "profession",
  "realName",
  "hobbies",
  "birthdate",
] as const;

// Shaped like a real `/lambdas/profile/{address}` answer for a profile that never
// filled its optional fields: lambdas sends each of them as an explicit null.
function nullFieldsAvatar(): Record<string, unknown> {
  return {
    userId: ADDRESS,
    name: "ExampleUser#1111",
    description: "",
    version: 7,
    ethAddress: ADDRESS,
    tutorialStep: 256,
    email: null,
    interests: null,
    hasClaimedName: false,
    hasConnectedWeb3: true,
    avatar: {
      bodyShape: "urn:decentraland:off-chain:base-avatars:BaseMale",
      eyes: { color: { r: 0.1, g: 0.2, b: 0.3, a: 1 } },
      hair: { color: { r: 0.1, g: 0.2, b: 0.3, a: 1 } },
      skin: { color: { r: 0.1, g: 0.2, b: 0.3, a: 1 } },
      wearables: ["urn:decentraland:off-chain:base-avatars:eyebrows_00"],
      emotes: [],
      snapshots: { face256: "https://example.com/face256.png", body: "https://example.com/body.png" },
      forceRender: [],
      showOnlyWearables: false,
    },
    nameColor: null,
    country: null,
    gender: null,
    pronouns: null,
    relationshipStatus: null,
    sexualOrientation: null,
    language: null,
    employmentStatus: null,
    profession: null,
    realName: null,
    hobbies: null,
    birthdate: null,
    links: null,
  };
}

const envelope = (avatar: Record<string, unknown>) => ({ timestamp: 1700000000000, avatars: [avatar] });

describe("when fetching a profile", () => {
  let avatar: Record<string, unknown>;
  beforeEach(() => {
    avatar = nullFieldsAvatar();
  });
  afterEach(() => vi.resetAllMocks());

  describe("and the lambdas answer sends the unset optional fields as null", () => {
    beforeEach(() => {
      vi.mocked(getJSON).mockResolvedValueOnce(envelope(avatar));
    });

    it.each(NULLABLE_FIELDS)("should parse %s as absent", async (field) => {
      await expect(fetchProfile(ADDRESS).then((a) => a?.[field])).resolves.toBeUndefined();
    });
  });

  describe("and the avatar snapshots object is null", () => {
    beforeEach(() => {
      (avatar.avatar as Record<string, unknown>).snapshots = null;
      vi.mocked(getJSON).mockResolvedValueOnce(envelope(avatar));
    });

    it("should parse the snapshots as absent", async () => {
      await expect(fetchProfile(ADDRESS).then((a) => a?.avatar?.snapshots)).resolves.toBeUndefined();
    });
  });

  describe("and the lambdas answer omits the optional fields", () => {
    beforeEach(() => {
      for (const field of NULLABLE_FIELDS) delete avatar[field];
      vi.mocked(getJSON).mockResolvedValueOnce(envelope(avatar));
    });

    it.each(NULLABLE_FIELDS)("should parse %s as absent", async (field) => {
      await expect(fetchProfile(ADDRESS).then((a) => a?.[field])).resolves.toBeUndefined();
    });
  });

  describe("and the optional fields are set", () => {
    beforeEach(() => {
      avatar = { ...avatar, country: "Exampleland", birthdate: 946684800 };
      vi.mocked(getJSON).mockResolvedValueOnce(envelope(avatar));
    });

    it("should keep their values", async () => {
      await expect(fetchProfile(ADDRESS)).resolves.toMatchObject({ country: "Exampleland", birthdate: 946684800 });
    });
  });

  describe.each([
    ["country", 42],
    ["birthdate", "2000-01-01"],
    ["links", "https://example.com"],
    ["nameColor", 7],
    ["hobbies", false],
  ])("and %s has the wrong type", (field, value) => {
    beforeEach(() => {
      avatar = { ...avatar, [field]: value };
      vi.mocked(getJSON).mockResolvedValueOnce(envelope(avatar));
    });

    it("should reject the profile with an issue at that field", async () => {
      await expect(fetchProfile(ADDRESS)).rejects.toMatchObject({
        issues: [expect.objectContaining({ path: ["avatars", 0, field] })],
      });
    });
  });

  describe("and the snapshots value has the wrong type", () => {
    beforeEach(() => {
      (avatar.avatar as Record<string, unknown>).snapshots = "https://example.com/face256.png";
      vi.mocked(getJSON).mockResolvedValueOnce(envelope(avatar));
    });

    it("should reject the profile with an issue at the snapshots", async () => {
      await expect(fetchProfile(ADDRESS)).rejects.toMatchObject({
        issues: [expect.objectContaining({ path: ["avatars", 0, "avatar", "snapshots"] })],
      });
    });
  });

  describe("and the required name is null", () => {
    beforeEach(() => {
      avatar = { ...avatar, name: null };
      vi.mocked(getJSON).mockResolvedValueOnce(envelope(avatar));
    });

    it("should reject the profile with an invalid type issue at the name", async () => {
      await expect(fetchProfile(ADDRESS)).rejects.toMatchObject({
        issues: [expect.objectContaining({ code: "invalid_type", path: ["avatars", 0, "name"] })],
      });
    });
  });
});

describe("when mapping a fetched profile", () => {
  let avatar: Record<string, unknown>;
  let vm: ProfileVM;
  beforeEach(() => {
    avatar = nullFieldsAvatar();
  });
  afterEach(() => vi.resetAllMocks());

  describe("and its unset optional fields arrived as null", () => {
    beforeEach(async () => {
      vi.mocked(getJSON).mockResolvedValueOnce(envelope(avatar));
      vm = mapProfile((await fetchProfile(ADDRESS))!, ADDRESS);
    });

    it("should show the profile name instead of the address", () => {
      expect(vm.name).toBe("ExampleUser#1111");
    });

    // The profile page derives the name color from the display name, as the profile site does.
    it("should derive the name color from the display name", () => {
      expect(vm.nameColor).toBe(getAvatarBackgroundColor(getDisplayName("ExampleUser#1111", false, ADDRESS)));
    });

    it("should show no info rows", () => {
      expect(vm.info).toEqual([]);
    });

    it("should show no links", () => {
      expect(vm.links).toEqual([]);
    });

    it("should show the face snapshot", () => {
      expect(vm.avatarUrl).toBe("https://example.com/face256.png");
    });
  });

  describe("and its snapshots object arrived as null", () => {
    beforeEach(async () => {
      (avatar.avatar as Record<string, unknown>).snapshots = null;
      vi.mocked(getJSON).mockResolvedValueOnce(envelope(avatar));
      vm = mapProfile((await fetchProfile(ADDRESS))!, ADDRESS);
    });

    it("should show no face image", () => {
      expect(vm.avatarUrl).toBe("");
    });
  });

  describe("and its optional fields are set", () => {
    beforeEach(async () => {
      avatar = {
        ...avatar,
        country: "Exampleland",
        links: [{ title: "Site", url: "https://example.com" }],
        nameColor: { r: 1, g: 0, b: 0 },
      };
      vi.mocked(getJSON).mockResolvedValueOnce(envelope(avatar));
      vm = mapProfile((await fetchProfile(ADDRESS))!, ADDRESS);
    });

    it("should derive the name color from the display name, not the avatar's color", () => {
      expect(vm.nameColor).toBe(getAvatarBackgroundColor(getDisplayName("ExampleUser#1111", false, ADDRESS)));
    });

    it("should show the country info row", () => {
      expect(vm.info).toEqual([{ key: "country", label: "Country", value: "Exampleland", icon: "globe" }]);
    });

    it("should show the links", () => {
      expect(vm.links).toEqual([{ title: "Site", url: "https://example.com/" }]);
    });
  });
});
