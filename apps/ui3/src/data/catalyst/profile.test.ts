import { afterEach, beforeEach, describe, expect, expectTypeOf, it, vi } from "vitest";

import { fetchProfile, mapProfile, parseProfileEnvelope, profileFaceUrl } from "./profile";
import type { AvatarInfoWire, AvatarWire } from "./schemas/profile";

const ADDRESS = "0x1111111111111111111111111111111111111111";

// Shaped like a real `/lambdas/profile/{address}` answer for a profile that never filled its
// optional fields: lambdas sends every field upstream marks nullable as an explicit null.
function nullFieldsAvatar(): Record<string, unknown> {
  return {
    userId: ADDRESS,
    name: "ExampleUser#1111",
    description: "",
    version: 7,
    ethAddress: ADDRESS,
    tutorialStep: 256,
    hasClaimedName: false,
    hasConnectedWeb3: null,
    email: null,
    blocked: null,
    muted: null,
    interests: null,
    avatar: {
      bodyShape: "urn:decentraland:off-chain:base-avatars:BaseMale",
      eyes: { color: { r: 0.1, g: 0.2, b: 0.3, a: 1 } },
      hair: { color: { r: 0.1, g: 0.2, b: 0.3, a: 1 } },
      skin: { color: { r: 0.1, g: 0.2, b: 0.3, a: 1 } },
      wearables: ["urn:decentraland:off-chain:base-avatars:eyebrows_00"],
      emotes: null,
      forceRender: null,
      snapshots: null,
    },
    nameColor: null,
    links: null,
    country: null,
    employmentStatus: null,
    gender: null,
    pronouns: null,
    relationshipStatus: null,
    sexualOrientation: null,
    language: null,
    profession: null,
    birthdate: null,
    realName: null,
    hobbies: null,
  };
}

const envelope = (avatar: Record<string, unknown>) => ({ timestamp: 1700000000000, avatars: [avatar] });
const avatarInfo = (avatar: Record<string, unknown>) => avatar.avatar as Record<string, unknown>;
const withoutNulls = (record: Record<string, unknown>) => {
  for (const key of Object.keys(record)) if (record[key] === null) delete record[key];
};

function serve(avatar: Record<string, unknown>) {
  vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response(JSON.stringify(envelope(avatar))));
}

describe("when reading the parsed profile types", () => {
  it("should keep nameColor, links and snapshots as optional keys", () => {
    expectTypeOf<{ avatar: Record<string, never> }>().toExtend<
      Pick<AvatarWire, "nameColor" | "links"> & { avatar: Pick<AvatarInfoWire, "snapshots"> }
    >();
  });

  it("should keep nameColor optional and never null", () => {
    expectTypeOf<AvatarWire["nameColor"]>().toEqualTypeOf<
      { r?: number; g?: number; b?: number; [k: string]: unknown } | string | undefined
    >();
  });

  it("should keep links optional and never null", () => {
    expectTypeOf<AvatarWire["links"]>().toEqualTypeOf<
      Array<{ title?: string | null; url?: string | null }> | undefined
    >();
  });

  it("should keep snapshots optional and never null", () => {
    expectTypeOf<AvatarInfoWire["snapshots"]>().toEqualTypeOf<
      { face256?: string; body?: string; [k: string]: unknown } | undefined
    >();
  });
});

describe("when fetching and mapping a profile", () => {
  let avatar: Record<string, unknown>;
  beforeEach(() => {
    avatar = nullFieldsAvatar();
  });
  afterEach(() => vi.restoreAllMocks());

  describe("and the lambdas answer sends every upstream nullable field as null", () => {
    let fetched: Awaited<ReturnType<typeof fetchProfile>>;
    beforeEach(async () => {
      serve(avatar);
      fetched = await fetchProfile(ADDRESS);
    });

    it("should show the profile name instead of the address", () => {
      expect(mapProfile(fetched!, ADDRESS).name).toBe("ExampleUser#1111");
    });

    it("should use the default name color", () => {
      expect(mapProfile(fetched!, ADDRESS).nameColor).toBe("#FF8362");
    });

    it("should show no links", () => {
      expect(mapProfile(fetched!, ADDRESS).links).toEqual([]);
    });

    it("should show no info rows", () => {
      expect(mapProfile(fetched!, ADDRESS).info).toEqual([]);
    });

    it("should give no face URL", () => {
      expect(profileFaceUrl(fetched)).toBeNull();
    });

    it("should keep the equipped wearables", () => {
      expect(mapProfile(fetched!, ADDRESS).equipped).toEqual(["urn:decentraland:off-chain:base-avatars:eyebrows_00"]);
    });
  });

  describe("and the lambdas answer omits the nullable fields", () => {
    let fetched: Awaited<ReturnType<typeof fetchProfile>>;
    beforeEach(async () => {
      withoutNulls(avatar);
      withoutNulls(avatarInfo(avatar));
      serve(avatar);
      fetched = await fetchProfile(ADDRESS);
    });

    it("should show the profile name instead of the address", () => {
      expect(mapProfile(fetched!, ADDRESS).name).toBe("ExampleUser#1111");
    });

    it("should use the default name color", () => {
      expect(mapProfile(fetched!, ADDRESS).nameColor).toBe("#FF8362");
    });

    it("should show no links", () => {
      expect(mapProfile(fetched!, ADDRESS).links).toEqual([]);
    });

    it("should give no face URL", () => {
      expect(profileFaceUrl(fetched)).toBeNull();
    });
  });

  describe("and nameColor, links and snapshots are set", () => {
    let fetched: Awaited<ReturnType<typeof fetchProfile>>;
    beforeEach(async () => {
      avatar.nameColor = { r: 1, g: 0, b: 0 };
      avatar.links = [{ title: "Site", url: "https://example.com" }];
      avatarInfo(avatar).snapshots = { face256: "https://example.com/face256.png", body: "https://example.com/body.png" };
      serve(avatar);
      fetched = await fetchProfile(ADDRESS);
    });

    it("should use the profile name color", () => {
      expect(mapProfile(fetched!, ADDRESS).nameColor).toBe("#ff0000");
    });

    it("should show the links", () => {
      expect(mapProfile(fetched!, ADDRESS).links).toEqual([{ title: "Site", url: "https://example.com" }]);
    });

    it("should give the face snapshot URL", () => {
      expect(profileFaceUrl(fetched)).toBe("https://example.com/face256.png");
    });
  });

  describe.each([
    ["nameColor", 7, ["avatars", 0, "nameColor"]],
    ["links", "https://example.com", ["avatars", 0, "links"]],
    ["links", [{ title: 1, url: "https://example.com" }], ["avatars", 0, "links", 0, "title"]],
  ])("and %s has the wrong type (%j)", (field, value, path) => {
    beforeEach(() => {
      serve({ ...avatar, [field]: value });
    });

    it("should reject the profile with an issue at that field", async () => {
      await expect(fetchProfile(ADDRESS)).rejects.toMatchObject({ issues: [expect.objectContaining({ path })] });
    });
  });

  describe("and snapshots has the wrong type", () => {
    beforeEach(() => {
      avatarInfo(avatar).snapshots = "https://example.com/face256.png";
      serve(avatar);
    });

    it("should reject the profile with an issue at the snapshots", async () => {
      await expect(fetchProfile(ADDRESS)).rejects.toMatchObject({
        issues: [expect.objectContaining({ path: ["avatars", 0, "avatar", "snapshots"] })],
      });
    });
  });
});

describe("when parsing a profile envelope", () => {
  describe("and the required avatars list is null", () => {
    it("should reject it with an invalid type issue at the avatars", () => {
      expect(() => parseProfileEnvelope({ avatars: null })).toThrow(
        expect.objectContaining({ issues: [expect.objectContaining({ code: "invalid_type", path: ["avatars"] })] }),
      );
    });
  });

  describe("and a required color channel is null", () => {
    let avatar: Record<string, unknown>;
    beforeEach(() => {
      avatar = nullFieldsAvatar();
      avatarInfo(avatar).skin = { color: { r: null, g: 0.2, b: 0.3 } };
    });

    it("should reject it with an invalid type issue at that channel", () => {
      expect(() => parseProfileEnvelope(envelope(avatar))).toThrow(
        expect.objectContaining({
          issues: [expect.objectContaining({ code: "invalid_type", path: ["avatars", 0, "avatar", "skin", "color", "r"] })],
        }),
      );
    });
  });
});
