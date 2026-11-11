import { describe, expect, it } from "vitest";

import { fetchProfile, mapProfile } from "./profile";

const ADDRESS = "0x1111111111111111111111111111111111111111";

// Shaped like a real `/lambdas/profile/{address}` answer for a profile that never
// filled its optional fields: lambdas sends each of them as an explicit null.
function nullFieldsProfile(): Record<string, unknown> {
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
      snapshots: {
        face256: "https://example.com/face256.png",
        body: "https://example.com/body.png",
      },
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

function serving(avatar: Record<string, unknown>) {
  return (async () =>
    new Response(JSON.stringify({ timestamp: 1700000000000, avatars: [avatar] }), {
      status: 200,
    })) as unknown as typeof fetch;
}

describe("fetchProfile", () => {
  it("parses a profile whose unset fields arrive as explicit nulls, reading each null as absent", async () => {
    const avatar = await fetchProfile(ADDRESS, { fetchImpl: serving(nullFieldsProfile()) });

    expect(avatar).not.toBeNull();
    for (const key of [
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
    ] as const) {
      expect(avatar?.[key], key).toBeUndefined();
    }
  });

  it("renders the null-field profile as a live profile with no info rows and no links", async () => {
    const avatar = await fetchProfile(ADDRESS, { fetchImpl: serving(nullFieldsProfile()) });
    const vm = mapProfile(avatar!, ADDRESS);

    expect(vm.name).toBe("ExampleUser#1111");
    expect(vm.nameColor).toBe("#FF8362");
    expect(vm.info).toEqual([]);
    expect(vm.links).toEqual([]);
    expect(vm.avatarUrl).toBeTruthy();
  });

  it("reads a null snapshots object as absent", async () => {
    const raw = nullFieldsProfile();
    (raw.avatar as Record<string, unknown>).snapshots = null;

    const avatar = await fetchProfile(ADDRESS, { fetchImpl: serving(raw) });

    expect(avatar?.avatar?.snapshots).toBeUndefined();
    expect(mapProfile(avatar!, ADDRESS).avatarUrl).toBeFalsy();
  });

  it("parses a profile that omits the optional fields altogether", async () => {
    const raw = nullFieldsProfile();
    for (const key of Object.keys(raw)) {
      if (raw[key] === null) delete raw[key];
    }

    const avatar = await fetchProfile(ADDRESS, { fetchImpl: serving(raw) });

    expect(avatar?.country).toBeUndefined();
    expect(avatar?.links).toBeUndefined();
    expect(mapProfile(avatar!, ADDRESS).info).toEqual([]);
  });

  it("keeps the values of fields that are set", async () => {
    const raw = {
      ...nullFieldsProfile(),
      country: "Exampleland",
      birthdate: 946684800,
      links: [{ title: "Site", url: "https://example.com" }],
      nameColor: { r: 1, g: 0, b: 0 },
    };

    const avatar = await fetchProfile(ADDRESS, { fetchImpl: serving(raw) });
    const vm = mapProfile(avatar!, ADDRESS);

    expect(avatar?.birthdate).toBe(946684800);
    expect(vm.nameColor).toBe("#ff0000");
    expect(vm.info).toEqual([
      { key: "country", label: "Country", value: "Exampleland", icon: "globe" },
    ]);
    expect(vm.links).toEqual([{ title: "Site", url: "https://example.com" }]);
  });

  it.each([
    ["country", 42],
    ["birthdate", "2000-01-01"],
    ["links", "https://example.com"],
    ["nameColor", 7],
    ["hobbies", false],
  ])("still rejects a %s of the wrong type", async (key, value) => {
    const raw = { ...nullFieldsProfile(), [key]: value };

    await expect(fetchProfile(ADDRESS, { fetchImpl: serving(raw) })).rejects.toThrow();
  });

  it("still rejects a snapshots value of the wrong type", async () => {
    const raw = nullFieldsProfile();
    (raw.avatar as Record<string, unknown>).snapshots = "https://example.com/face256.png";

    await expect(fetchProfile(ADDRESS, { fetchImpl: serving(raw) })).rejects.toThrow();
  });

  it("still rejects a null required field", async () => {
    const raw = { ...nullFieldsProfile(), name: null };

    await expect(fetchProfile(ADDRESS, { fetchImpl: serving(raw) })).rejects.toThrow();
  });
});
