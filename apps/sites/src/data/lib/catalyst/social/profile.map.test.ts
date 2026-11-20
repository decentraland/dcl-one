import { beforeEach, describe, expect, it } from "vitest";

import { getAvatarBackgroundColor, getDisplayName } from "@ui/shop/discovery/lib/avatarColor";

import { emptyProfile, mapProfile, wornCollectibleUrns } from "./profile";

const A = "0x4e9c4a2502fdf71e93ed8ed6ca9ddbd891d6f295";
const ITEM = "urn:decentraland:matic:collections-v2:0x242e85d51f975e0fd3c6208e3e7be852bc8a11a0:0";
const avatar = (over: Record<string, unknown> = {}) =>
  ({ name: "Alice", hasClaimedName: true, description: "", avatar: { snapshots: {} }, ...over }) as never;

describe("mapProfile", () => {
  describe("when the profile stores a name color", () => {
    let vm: ReturnType<typeof mapProfile>;

    beforeEach(() => {
      vm = mapProfile(avatar({ nameColor: "#000000" }), A);
    });

    it("should color the name from the display name, as every client does", () => {
      expect(vm.nameColor).toBe(getAvatarBackgroundColor(getDisplayName("Alice", true, A)));
    });

    it("should not use the stored color", () => {
      expect(vm.nameColor).not.toBe("#000000");
    });
  });

  describe("when reading the full-body snapshot through the face's local image rules", () => {
    describe("and the snapshot is a profile-images URL", () => {
      it("should serve it from the local profile-images path", () => {
        const fromHost = mapProfile(
          avatar({ avatar: { snapshots: { body: "https://profile-images.decentraland.org/entities/bafyabc/body.png" } } }),
          A,
        );
        expect(fromHost.bodyUrl).toMatch(/\/profile-images\/entities\/bafyabc\/body\.png$/);
      });
    });

    describe("and the snapshot is a content hash", () => {
      it("should serve it from the content server", () => {
        expect(mapProfile(avatar({ avatar: { snapshots: { body: "bafyhash" } } }), A).bodyUrl).toMatch(/\/content\/contents\/bafyhash$/);
      });
    });

    describe("and there is no snapshot", () => {
      it("should leave the body URL empty", () => {
        expect(mapProfile(avatar(), A).bodyUrl).toBe("");
      });
    });
  });

  describe("when the profile has links", () => {
    let vm: ReturnType<typeof mapProfile>;

    beforeEach(() => {
      vm = mapProfile(
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
    });

    it("should keep only http(s) links and name an untitled one by its site", () => {
      expect(vm.links).toEqual([
        { title: "My blog", url: "https://blog.example.com/" },
        { title: "Example", url: "https://www.example.org/me" },
        { title: "Github", url: "https://github.com/alice" },
      ]);
    });
  });
});

describe("worn collectibles", () => {
  describe("when filtering worn URNs", () => {
    it("should keep only the marketplace's on-chain collection items, without the worn copy's token id, once each", () => {
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
  });

  describe("when the avatar wears collectibles", () => {
    it("should read them from the avatar's wearables", () => {
      expect(mapProfile(avatar({ avatar: { snapshots: {}, wearables: [`${ITEM}:42`] } }), A).wornCollectibles).toEqual([ITEM]);
    });
  });

  describe("when the avatar wears nothing", () => {
    it("should have no worn collectibles", () => {
      expect(mapProfile(avatar(), A).wornCollectibles).toEqual([]);
    });
  });

  describe("when the wallet has no profile", () => {
    it("should have no worn collectibles", () => {
      expect(emptyProfile(A).wornCollectibles).toEqual([]);
    });
  });
});
