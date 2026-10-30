import { beforeEach, expect, it, vi } from "vitest";
import { loadAssetCatalog } from "@/data/lib/catalyst/creator-hub/asset-catalog.server";
import { loader } from "./api.creator-hub.asset-catalog";

vi.mock("@/data/lib/catalyst/creator-hub/asset-catalog.server", () => ({ loadAssetCatalog: vi.fn() }));
beforeEach(() => vi.clearAllMocks());
const request = () => new Request("https://sites.test/api/creator-hub/asset-catalog");

it("serves a cacheable catalog retaining the files and composite needed to place assets", async () => {
  const items = [{ id: "tree", name: "Tree", category: "Nature", pack: "Trees",
    thumbnailUrl: "/builder-api/contents/thumb", glbFile: "tree.gltf", glbUrl: "/builder-api/contents/model",
    contents: { "tree.gltf": "model", "tree.bin": "buffer" }, isMultiFile: true,
    composite: { components: [{ id: 1 }] }, smart: true }];
  vi.mocked(loadAssetCatalog).mockResolvedValue(items);
  const response = await loader({ request: request() });
  expect(response.status).toBe(200);
  expect(response.headers.get("Cache-Control")).toContain("public, max-age=300");
  expect(await response.json()).toEqual(items);
});

it.each([null, new Error("upstream unavailable")])("does not cache an unavailable catalog (%s)", async result => {
  if (result instanceof Error) vi.mocked(loadAssetCatalog).mockRejectedValue(result);
  else vi.mocked(loadAssetCatalog).mockResolvedValue(result);
  const response = await loader({ request: request() });
  expect(response.status).toBe(503);
  expect(response.headers.get("Cache-Control")).toBe("no-store");
});

it("propagates navigation cancellation", async () => {
  const controller = new AbortController();
  vi.mocked(loadAssetCatalog).mockImplementation(async ({ signal } = {}) => {
    controller.abort();
    signal?.throwIfAborted();
    return null;
  });
  await expect(loader({ request: new Request(request(), { signal: controller.signal }) })).rejects.toThrow();
});
