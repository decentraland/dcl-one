import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { z } from "zod";
vi.mock("../client", async (original) => ({ ...await original<typeof import("../client")>(), getJSON: vi.fn(), signedGetJSON: vi.fn() }));
import { getJSON, signedGetJSON } from "../client";
import { DestinationSchema } from "../generated-schemas/places";
import { fetchDestinations, resolveLegacyPlace } from "./destinations";
import { mapPlace } from "./index";
import type { AuthIdentity } from "../../auth/types";

describe("when browsing the unified destinations feed", () => {
  let row: z.infer<typeof DestinationSchema>;
  let identity: AuthIdentity;
  beforeEach(() => {
    row = { id: "world-id", kind: "world", title: "A world", description: "World discovery", image: "https://example.com/world.png", owner: "0x1111111111111111111111111111111111111111", positions: [], base_position: "", contact_name: "Creator", contact_email: null, content_rating: null, disabled: false, disabled_at: null, created_at: null, updated_at: null, favorites: 4, likes: 8, dislikes: 2, categories: ["music"], highlighted: false, highlighted_image: null, ranking: null, sdk: null, creator_address: null, deployed_at: null, world: true, world_name: "sample.dcl.eth", is_private: false, user_favorite: false, user_like: false, user_dislike: false, user_count: null, user_visits: 9, like_rate: .8, like_score: null };
    identity = { signer: row.owner ?? "", ephemeral: { address: "0x2222222222222222222222222222222222222222", privateKey: "0x01" }, expiration: "2099-01-01T00:00:00Z", authChain: [] };
    vi.mocked(getJSON).mockResolvedValue({ ok: true, data: [row], total: 1 });
    vi.mocked(signedGetJSON).mockResolvedValue({ ok: true, data: [row], total: 1 });
  });
  afterEach(() => vi.resetAllMocks());

  it("should order by activity and request live decorations from the destinations endpoint", async () => {
    await fetchDestinations({ limit: 40 });
    expect(getJSON).toHaveBeenCalledWith("/places/api/destinations", expect.objectContaining({ query: expect.objectContaining({ order_by: "most_active", limit: 40, with_realms_detail: true, with_live_events: true }) }));
  });

  it("should preserve world locations and unavailable presence in rendered card data", async () => {
    await expect(fetchDestinations().then((result) => mapPlace(result.data[0]!))).resolves.toMatchObject({ coords: "sample.dcl.eth", players: null, rating: 80 });
  });

  it("should read personal favourites with the current signed identity", async () => {
    await fetchDestinations({ only_favorites: true, offset: 60 }, { identity });
    expect(signedGetJSON).toHaveBeenCalledWith("/places/api/destinations", expect.objectContaining({ identity, query: expect.objectContaining({ only_favorites: true, offset: 60 }) }));
    expect(getJSON).not.toHaveBeenCalled();
  });

  it("should not cache signed personal results", async () => {
    await fetchDestinations({ only_favorites: true }, { identity });
    await fetchDestinations({ only_favorites: true }, { identity });
    expect(signedGetJSON).toHaveBeenCalledTimes(2);
  });

  it("should reject a signed-out favourites request", async () => {
    await expect(fetchDestinations({ only_favorites: true })).rejects.toThrow("Sign in");
    expect(getJSON).not.toHaveBeenCalled();
  });

  it("should resolve a legacy world name using an exact world filter", async () => {
    await expect(resolveLegacyPlace("world", "SAMPLE.dcl.eth")).resolves.toBe("world-id");
    expect(getJSON).toHaveBeenCalledWith("/places/api/destinations", expect.objectContaining({ query: expect.objectContaining({ world_names: "sample.dcl.eth" }) }));
  });

  it("should resolve a legacy parcel using the position lookup", async () => {
    await expect(resolveLegacyPlace("place", "1,-2")).resolves.toBe("world-id");
    expect(getJSON).toHaveBeenCalledWith("/places/api/places", expect.objectContaining({ query: { positions: "1,-2", limit: 1 } }));
  });

  it("should reject malformed coordinates without querying the catalogue", async () => {
    await expect(resolveLegacyPlace("place", "1.5,2")).resolves.toBeNull();
    expect(getJSON).not.toHaveBeenCalled();
  });
});
