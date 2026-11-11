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

const identity = {
  signer: OWNER.toLowerCase(),
  ephemeral: { address: "0x2222222222222222222222222222222222222222", privateKey: "0x01" },
  expiration: "2099-01-01T00:00:00Z",
  authChain: [],
} as AuthIdentity;

afterEach(() => vi.resetAllMocks());

describe("fetchProfilePlaces", () => {
  beforeEach(() => {
    vi.mocked(getJSON).mockResolvedValue({
      ok: true,
      total: 2,
      data: [row(), row({ id: "abcdcl.dcl.eth", kind: "world", world: true, world_name: "abcdcl.dcl.eth", base_position: "0,0" })],
    });
  });

  it("asks the destinations catalogue for every place and world the lowercased wallet owns", async () => {
    await fetchProfilePlaces(OWNER, { limit: 100 });

    expect(getJSON).toHaveBeenCalledWith(
      "/places/api/destinations",
      expect.objectContaining({ query: expect.objectContaining({ owner: OWNER.toLowerCase(), limit: 100 }) }),
    );
    expect(signedGetJSON).not.toHaveBeenCalled();
  });

  it("maps scenes and worlds to what a card shows", async () => {
    const places = await fetchProfilePlaces(OWNER);

    expect(places).toEqual([
      { id: "place-1", title: "ABC Clubhouse", image: "https://example.com/a.png", owner: OWNER.toLowerCase(), world: false, basePosition: "-31,-27", worldName: undefined },
      expect.objectContaining({ id: "abcdcl.dcl.eth", world: true, worldName: "abcdcl.dcl.eth" }),
    ]);
    expect(places.map(placeLocation)).toEqual(["-31,-27", "abcdcl.dcl.eth"]);
  });

  it("keeps only http(s) images and drops a missing owner", async () => {
    vi.mocked(getJSON).mockResolvedValue({
      ok: true,
      total: 2,
      data: [row({ image: "javascript:alert(1)", owner: null }), row({ id: "b", image: null, owner: "  " })],
    });

    const places = await fetchProfilePlaces(OWNER);

    expect(places.map((p) => [p.image, p.owner])).toEqual([[undefined, undefined], [undefined, undefined]]);
  });

  it("returns an empty list when the wallet owns nothing", async () => {
    vi.mocked(getJSON).mockResolvedValue({ ok: true, total: 0, data: [] });

    await expect(fetchProfilePlaces(OWNER)).resolves.toEqual([]);
  });

  it("fails instead of answering an empty list when the catalogue errors", async () => {
    vi.mocked(getJSON).mockRejectedValue(new CatalystError("down", "/places/api/destinations", 503));

    await expect(fetchProfilePlaces(OWNER)).rejects.toThrow("down");
  });

  it("fails on a body that is not a destinations envelope", async () => {
    vi.mocked(getJSON).mockResolvedValue({ data: "nope" });

    await expect(fetchProfilePlaces(OWNER)).rejects.toThrow();
  });
});

describe("fetchFavoritePlaces", () => {
  it("reads the signed caller's saved places only, signed with their identity", async () => {
    vi.mocked(signedGetJSON).mockResolvedValue({ ok: true, total: 1, data: [row({ owner: "0x1111111111111111111111111111111111111111" })] });

    const places = await fetchFavoritePlaces(identity);

    expect(signedGetJSON).toHaveBeenCalledWith(
      "/places/api/destinations",
      expect.objectContaining({ identity, query: expect.objectContaining({ only_favorites: true, limit: 100 }) }),
    );
    expect(getJSON).not.toHaveBeenCalled();
    expect(places[0]?.owner).toBe("0x1111111111111111111111111111111111111111");
  });

  it("fails when the signed read is refused", async () => {
    vi.mocked(signedGetJSON).mockRejectedValue(new CatalystError("unauthorized", "/places/api/destinations", 401));

    await expect(fetchFavoritePlaces(identity)).rejects.toThrow("unauthorized");
  });
});
