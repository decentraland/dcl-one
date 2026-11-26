import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { z } from "zod";

vi.mock("../client", async (original) => ({
  ...(await original<typeof import("../client")>()),
  getJSON: vi.fn(),
  signedGetJSON: vi.fn(),
}));
vi.mock("../schema-drift", () => ({ reportSchemaDrift: vi.fn() }));

import type { AuthIdentity } from "../../auth/types";
import { CatalystError, getJSON, signedGetJSON } from "../client";
import type { DestinationSchema } from "../generated-schemas/places";
import { fetchFavoritePlaces, fetchProfilePlaces, placeLocation } from "./profile-places";

const OWNER = "0x09BC69A986E1B37FB01EF36124F4C2E14FA0AB6B";

function row(over: Partial<z.infer<typeof DestinationSchema>> = {}): z.infer<typeof DestinationSchema> {
  return {
    id: "place-1", kind: "place", title: " ABC Clubhouse ", description: null, image: "https://example.com/a.png",
    owner: OWNER, positions: ["-31,-27"], base_position: "-31,-27", contact_name: null, contact_email: null,
    content_rating: null, disabled: false, disabled_at: null, created_at: null, updated_at: null, favorites: 0,
    likes: 0, dislikes: 0, categories: [], highlighted: false, highlighted_image: null, ranking: null, sdk: null,
    creator_address: null, deployed_at: null, world: false, world_name: null, is_private: false, user_favorite: false,
    user_like: false, user_dislike: false, user_count: null, user_visits: 0, like_rate: null, like_score: null,
    ...over,
  };
}

afterEach(() => vi.resetAllMocks());

describe("fetchProfilePlaces", () => {
  describe("when the wallet owns a place and a world", () => {
    let result: Awaited<ReturnType<typeof fetchProfilePlaces>>;

    beforeEach(() => {
      vi.mocked(getJSON).mockResolvedValueOnce({
        ok: true,
        total: 2,
        data: [row(), row({ id: "abcdcl.dcl.eth", kind: "world", world: true, world_name: "abcdcl.dcl.eth", base_position: "0,0" })],
      });
    });

    describe("and a limit is given", () => {
      beforeEach(async () => {
        await fetchProfilePlaces(OWNER, { limit: 100 });
      });

      it("should ask the destinations catalogue for every place and world the lowercased wallet owns", () => {
        expect(getJSON).toHaveBeenCalledWith(
          "/places/api/destinations",
          expect.objectContaining({ query: expect.objectContaining({ owner: OWNER.toLowerCase(), limit: 100 }) }),
        );
      });

      it("should not sign the read", () => {
        expect(signedGetJSON).not.toHaveBeenCalled();
      });
    });

    describe("and the places are mapped to cards", () => {
      beforeEach(async () => {
        result = await fetchProfilePlaces(OWNER);
      });

      it("should answer the catalogue's total", () => {
        expect(result.total).toBe(2);
      });

      it("should map scenes and worlds to what a card shows", () => {
        expect(result.places).toEqual([
          { id: "place-1", title: "ABC Clubhouse", image: "https://example.com/a.png", owner: OWNER.toLowerCase(), world: false, basePosition: "-31,-27", worldName: undefined },
          expect.objectContaining({ id: "abcdcl.dcl.eth", world: true, worldName: "abcdcl.dcl.eth" }),
        ]);
      });

      it("should locate a scene by its position and a world by its name", () => {
        expect(result.places.map(placeLocation)).toEqual(["-31,-27", "abcdcl.dcl.eth"]);
      });

      it("should not locate a world without a name", () => {
        expect(placeLocation({ ...result.places[1]!, worldName: undefined })).toBeUndefined();
      });
    });
  });

  describe("when images are not http(s) and owners are missing", () => {
    beforeEach(() => {
      vi.mocked(getJSON).mockResolvedValueOnce({
        ok: true,
        total: 2,
        data: [row({ image: "javascript:alert(1)", owner: null }), row({ id: "b", image: null, owner: "  " })],
      });
    });

    it("should drop the images and the owners", async () => {
      const { places } = await fetchProfilePlaces(OWNER);
      expect(places.map((p) => [p.image, p.owner])).toEqual([[undefined, undefined], [undefined, undefined]]);
    });
  });

  describe("when the wallet owns nothing", () => {
    beforeEach(() => {
      vi.mocked(getJSON).mockResolvedValueOnce({ ok: true, total: 0, data: [] });
    });

    it("should return an empty list", async () => {
      await expect(fetchProfilePlaces(OWNER)).resolves.toEqual({ places: [], total: 0 });
    });
  });

  describe("when the catalogue errors", () => {
    beforeEach(() => {
      vi.mocked(getJSON).mockRejectedValueOnce(new CatalystError("down", "/places/api/destinations", 503));
    });

    it("should fail instead of answering an empty list", async () => {
      await expect(fetchProfilePlaces(OWNER)).rejects.toThrow("down");
    });
  });

  describe("when the body is not a destinations envelope", () => {
    beforeEach(() => {
      vi.mocked(getJSON).mockResolvedValueOnce({ data: "nope" });
    });

    it("should fail", async () => {
      await expect(fetchProfilePlaces(OWNER)).rejects.toThrow(/data/);
    });
  });
});

describe("fetchFavoritePlaces", () => {
  let identity: AuthIdentity;

  beforeEach(() => {
    identity = {
      signer: OWNER.toLowerCase(),
      ephemeral: { address: "0x2222222222222222222222222222222222222222", privateKey: "0x01" },
      expiration: "2099-01-01T00:00:00Z",
      authChain: [],
    } as AuthIdentity;
  });

  describe("when the signed caller has saved places", () => {
    let result: Awaited<ReturnType<typeof fetchFavoritePlaces>>;

    beforeEach(async () => {
      vi.mocked(signedGetJSON).mockResolvedValueOnce({ ok: true, total: 1, data: [row({ owner: "0x1111111111111111111111111111111111111111" })] });
      result = await fetchFavoritePlaces(identity);
    });

    it("should read only the caller's saved places, signed with their identity", () => {
      expect(signedGetJSON).toHaveBeenCalledWith(
        "/places/api/destinations",
        expect.objectContaining({ identity, query: expect.objectContaining({ only_favorites: true, limit: 100 }) }),
      );
    });

    it("should not make an unsigned read", () => {
      expect(getJSON).not.toHaveBeenCalled();
    });

    it("should keep each place's owner", () => {
      expect(result.places[0]?.owner).toBe("0x1111111111111111111111111111111111111111");
    });
  });

  describe("when the signed read is refused", () => {
    beforeEach(() => {
      vi.mocked(signedGetJSON).mockRejectedValueOnce(new CatalystError("unauthorized", "/places/api/destinations", 401));
    });

    it("should fail with the refusal", async () => {
      await expect(fetchFavoritePlaces(identity)).rejects.toThrow("unauthorized");
    });
  });
});
