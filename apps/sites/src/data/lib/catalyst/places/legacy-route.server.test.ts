import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
vi.mock("./destinations", () => ({ resolveLegacyPlace: vi.fn() }));
import { resolveLegacyPlace } from "./destinations";
import { legacyPlaceRedirect } from "./legacy-route.server";

describe("when opening a legacy place page", () => {
  let request: Request;
  beforeEach(() => { request = new Request("https://example.test/places/place/1,2?source=bookmark"); vi.mocked(resolveLegacyPlace).mockResolvedValue("canonical-place"); });
  afterEach(() => vi.resetAllMocks());

  it("should redirect to the current canonical place and retain the query", async () => {
    await expect(legacyPlaceRedirect(request, "1,2").then((response) => response.headers.get("Location"))).resolves.toBe("/places/canonical-place?source=bookmark");
  });

  it("should answer not found for an unknown destination", async () => {
    vi.mocked(resolveLegacyPlace).mockResolvedValueOnce(null);
    await expect(legacyPlaceRedirect(request, "9,9")).rejects.toMatchObject({ status: 404 });
  });

  it("should distinguish a service failure from an unknown destination", async () => {
    vi.mocked(resolveLegacyPlace).mockRejectedValueOnce(new Error("offline"));
    await expect(legacyPlaceRedirect(request, "1,2")).rejects.toMatchObject({ status: 503 });
  });
});
