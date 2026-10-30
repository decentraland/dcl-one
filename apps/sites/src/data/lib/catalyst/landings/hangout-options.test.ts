import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
vi.mock("../client", async (original) => ({ ...await original<typeof import("../client")>(), getJSON: vi.fn(), signedGetJSON: vi.fn() }));
import { getJSON, signedGetJSON } from "../client";
import { loadManageableCommunities, searchFeaturedAssets } from "./hangout-options";
import type { AuthIdentity } from "../../auth/types";

describe("when loading community options for a hangout", () => {
  let identity: AuthIdentity;
  beforeEach(() => {
    identity = { signer: "0x1111111111111111111111111111111111111111", ephemeral: { address: "0x2222222222222222222222222222222222222222", privateKey: "0x01" }, expiration: "2099-01-01T00:00:00Z", authChain: [] };
    vi.mocked(signedGetJSON).mockResolvedValue({ data: { results: [{ id: "owned", name: "Owned community", role: "owner" }, { id: "moderated", name: "Moderated community", role: "moderator" }, { id: "member", name: "Membership only", role: "member" }], total: 3 } });
  });
  afterEach(() => vi.resetAllMocks());
  it("should request management roles using the authenticated native community API", async () => {
    await loadManageableCommunities(identity);
    expect(signedGetJSON).toHaveBeenCalledWith(expect.stringContaining("roles=owner&roles=moderator"), expect.objectContaining({ identity }));
  });
  it("should offer only communities the account owns or moderates", async () => {
    await expect(loadManageableCommunities(identity)).resolves.toEqual([{ id: "owned", name: "Owned community" }, { id: "moderated", name: "Moderated community" }]);
  });
});

describe("when searching native featured assets", () => {
  let item: string;
  let collection: string;
  beforeEach(() => {
    item = "urn:decentraland:matic:collections-v2:0x1111111111111111111111111111111111111111:2";
    collection = "urn:decentraland:ethereum:collections-v2:0x3333333333333333333333333333333333333333";
    vi.mocked(getJSON).mockImplementation(async (path) => ({ data: path.endsWith("items") ? [{ urn: item, name: "Native wearable", thumbnail: null }, { urn: "invalid", name: "Unsupported" }] : [{ urn: collection, name: "Native collection", thumbnail: null }] }));
  });
  afterEach(() => vi.resetAllMocks());
  it("should combine actual items and collections while excluding unsupported asset URNs", async () => {
    await expect(searchFeaturedAssets("native")).resolves.toEqual([{ urn: item, name: "Native wearable", thumbnail: null, kind: "item" }, { urn: collection, name: "Native collection", thumbnail: null, kind: "collection" }]);
  });
  it("should resolve an exact collection URN without issuing an unrelated item search", async () => {
    await searchFeaturedAssets(collection);
    expect(getJSON).toHaveBeenCalledTimes(1);
    expect(getJSON).toHaveBeenCalledWith("/marketplace-api/v1/collections", expect.objectContaining({ query: { urn: collection, first: 6 } }));
  });
  it("should surface catalogue errors instead of accepting an unresolved asset as a successful lookup", async () => {
    vi.mocked(getJSON).mockRejectedValueOnce(new Error("Catalogue unavailable"));
    await expect(searchFeaturedAssets(collection)).rejects.toThrow("Catalogue unavailable");
  });
});
