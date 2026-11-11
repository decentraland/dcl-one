import { describe, expect, it } from "vitest";

import { fetchReferrerProfile } from "./referral";

const ADDRESS = "0x2222222222222222222222222222222222222222";

function serving(avatar: Record<string, unknown>) {
  return (async () =>
    new Response(JSON.stringify({ avatars: [avatar] }), { status: 200 })) as unknown as typeof fetch;
}

const avatar = (nameColor: unknown) => ({
  userId: ADDRESS,
  name: "ExampleFriend",
  ethAddress: ADDRESS,
  hasClaimedName: true,
  nameColor,
});

describe("fetchReferrerProfile", () => {
  it("resolves a referrer whose nameColor arrives as an explicit null", async () => {
    const vm = await fetchReferrerProfile(ADDRESS, { fetchImpl: serving(avatar(null)) });

    expect(vm).toEqual({ name: "ExampleFriend", address: ADDRESS, hasClaimedName: true, resolved: true });
  });

  it("resolves a referrer that omits nameColor", async () => {
    const raw: Record<string, unknown> = avatar(null);
    delete raw.nameColor;

    const vm = await fetchReferrerProfile(ADDRESS, { fetchImpl: serving(raw) });

    expect(vm?.name).toBe("ExampleFriend");
  });

  it("still rejects a nameColor of the wrong type", async () => {
    await expect(
      fetchReferrerProfile(ADDRESS, { fetchImpl: serving(avatar(7)) }),
    ).rejects.toThrow();
  });
});
