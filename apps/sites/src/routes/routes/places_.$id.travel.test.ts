import { data } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { loadPlace } from "@/data/lib/catalyst/places/index.server";
import { loadPlaceTravel } from "@/data/lib/catalyst/places/world-scenes";
import { loader } from "./places_.$id";

vi.mock("@/data/lib/catalyst/places/index.server", () => ({ loadPlace: vi.fn() }));
vi.mock("@/data/lib/catalyst/places/world-scenes", async (original) => ({ ...await original<typeof import("@/data/lib/catalyst/places/world-scenes")>(), loadPlaceTravel: vi.fn() }));
vi.mock("@/core/lib/telemetry/page-loader", () => ({
  pageLoaderWith: async (_request: Request, _context: unknown, _defaults: unknown, load: () => Promise<unknown>) => ({ sid: "travel-test", data: await load(), wrap: data }),
}));

describe("when retrying travel through a single-fetch data request", () => {
  let request: Request;
  let result: Awaited<ReturnType<typeof loader>>;
  beforeEach(() => {
    request = new Request("https://site.example/places/genesis.data?retry_travel=1&_routes=routes%2Fplaces_.%24id&tab=details");
    vi.mocked(loadPlace).mockResolvedValue({ id: "genesis", world: false, world_name: null, base_position: "1,2" } as Awaited<ReturnType<typeof loadPlace>>);
    vi.mocked(loadPlaceTravel).mockResolvedValue({ playBase: "/play/", jumpHref: "/play/?realm=node&position=1%2C2" });
  });
  afterEach(() => vi.resetAllMocks());

  it("should refresh and redirect to the document URL without internal request parameters", async () => {
    result = await loader({ request, params: { id: "genesis" } } as never);
    expect(loadPlaceTravel).toHaveBeenCalledWith(expect.anything(), { signal: request.signal, refreshRealm: true });
    expect(result).toBeInstanceOf(Response);
    expect((result as Response).status).toBe(303);
    expect((result as Response).headers.get("Location")).toBe("/places/genesis?tab=details");
  });

  describe("and a normal read has no resolved realm", () => {
    beforeEach(() => {
      request = new Request("https://site.example/places/genesis");
      vi.mocked(loadPlaceTravel).mockResolvedValueOnce(null);
    });
    it("should keep the place available and present a travel failure", async () => {
      result = await loader({ request, params: { id: "genesis" } } as never);
      expect(result).toMatchObject({ data: { place: { id: "genesis" }, travel: { launchError: "Travel is temporarily unavailable. Please retry." } }, init: { status: 200 } });
    });
  });
});
