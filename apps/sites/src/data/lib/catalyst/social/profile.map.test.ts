import { describe, expect, it } from "vitest";

import { getAvatarBackgroundColor, getDisplayName } from "@ui/shop/discovery/lib/avatarColor";

import { emptyProfile, mapProfile, wornCollectibleUrns } from "./profile";

const A = "0x4e9c4a2502fdf71e93ed8ed6ca9ddbd891d6f295";
const avatar = (over: Record<string, unknown> = {}) =>
  ({ name: "Alice", hasClaimedName: true, description: "", avatar: { snapshots: {} }, ...over }) as never;

describe("mapProfile", () => {
  it("colors the name from the display name, as every client does, not from the stored color", () => {
    const vm = mapProfile(avatar({ nameColor: "#000000" }), A);

    expect(vm.nameColor).toBe(getAvatarBackgroundColor(getDisplayName("Alice", true, A)));
    expect(vm.nameColor).not.toBe("#000000");
  });

  it("takes the full-body snapshot through the same local image rules as the face", () => {
    const fromHost = mapProfile(
      avatar({ avatar: { snapshots: { body: "https://profile-images.decentraland.org/entities/bafyabc/body.png" } } }),
      A,
    );
    const fromHash = mapProfile(avatar({ avatar: { snapshots: { body: "bafyhash" } } }), A);
    const none = mapProfile(avatar(), A);

    expect(fromHost.bodyUrl).toMatch(/\/profile-images\/entities\/bafyabc\/body\.png$/);
    expect(fromHash.bodyUrl).toMatch(/\/content\/contents\/bafyhash$/);
    expect(none.bodyUrl).toBe("");
  });

  it("keeps only http(s) links and names an untitled one by its site", () => {
    const vm = mapProfile(
      avatar({
        links: [
          { title: "My blog", url: "https://blog.example.com/" },
          { title: "https://www.example.org/me", url: "https://www.example.org/me" },
          { title: "  ", url: "https://github.com/alice" },
          { title: "click", url: "javascript:alert(1)" },
        ],
      }),
      A,
    );

    expect(vm.links).toEqual([
      { title: "My blog", url: "https://blog.example.com/" },
      { title: "Example", url: "https://www.example.org/me" },
      { title: "Github", url: "https://github.com/alice" },
    ]);
  });
});

describe("worn collectibles", () => {
  const ITEM = "urn:decentraland:matic:collections-v2:0x242e85d51f975e0fd3c6208e3e7be852bc8a11a0:0";

  it("keep only the marketplace's on-chain collection items, without the worn copy's token id, once each", () => {
    expect(
      wornCollectibleUrns([
        "urn:decentraland:off-chain:base-avatars:eyebrows_00",
        `${ITEM}:105312291668557186697918027683670432318895095400549111254310977537`,
        ITEM.toUpperCase().replace("URN:DECENTRALAND:MATIC:COLLECTIONS-V2", "urn:decentraland:matic:collections-v2"),
        "urn:decentraland:ethereum:collections-v1:halloween_2019:funny_skull_mask",
        "urn:decentraland:matic:collections-thirdparty:brand:collection:item",
        "urn:decentraland:mumbai:collections-v2:0xabc:1",
        "not a urn",
      ]),
    ).toEqual([
      ITEM,
      "urn:decentraland:ethereum:collections-v1:halloween_2019:funny_skull_mask",
    ]);
  });

  it("are read from the avatar's wearables, and are none for a wallet with no profile", () => {
    expect(mapProfile(avatar({ avatar: { snapshots: {}, wearables: [`${ITEM}:42`] } }), A).wornCollectibles).toEqual([ITEM]);
    expect(mapProfile(avatar(), A).wornCollectibles).toEqual([]);
    expect(emptyProfile(A).wornCollectibles).toEqual([]);
  });
});
