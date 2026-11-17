import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { data } from "react-router";
import { loadWorldPlace } from "@/data/lib/catalyst/places/destinations";
import { loadPlaceTravel, loadWorldScenes } from "@/data/lib/catalyst/places/world-scenes";
import { loadLivePlayers } from "@/data/lib/catalyst/places/live-players.server";
import { loader } from "./places_.world.$name";

vi.mock("@/data/lib/catalyst/places/destinations", () => ({ loadWorldPlace: vi.fn() }));
vi.mock("@/data/lib/catalyst/places/world-scenes", async (original) => ({
  ...await original<typeof import("@/data/lib/catalyst/places/world-scenes")>(), loadPlaceTravel: vi.fn(), loadWorldScenes: vi.fn(),
}));
vi.mock("@/data/lib/catalyst/places/live-players.server", () => ({ loadLivePlayers: vi.fn(), isMobileRequest: () => false }));
vi.mock("@/core/lib/telemetry/page-loader", () => ({
  pageLoaderWith: async (_request: Request, _context: unknown, _defaults: unknown, load: () => Promise<unknown>) => ({ sid: "world-test", data: await load(), wrap: data }),
}));

const WORLD = { id: "sample.dcl.eth", title: "Sample", world: true, world_name: "sample.dcl.eth", base_position: "0,0", positions: ["0,0"] };
const open = (name: string, query = "") => loader({ request: new Request(`https://site.example/places/world/${name}${query}`), params: { name } } as never);

describe("when opening a world's page by its name", () => {
  beforeEach(() => {
    vi.mocked(loadWorldPlace).mockResolvedValue(WORLD as never);
    vi.mocked(loadPlaceTravel).mockResolvedValue({ playBase: "/play/", jumpHref: "/play/?realm=sample.dcl.eth&position=0%2C0" });
    vi.mocked(loadWorldScenes).mockResolvedValue([{ entityId: "bafy1", title: "Plaza", base: "0,0" }]);
    vi.mocked(loadLivePlayers).mockResolvedValue(2);
  });
  afterEach(() => vi.resetAllMocks());

  it("should show the world at its own address, with its scenes and players, without a redirect", async () => {
    const result = await open("Sample.dcl.eth");
    expect(loadWorldPlace).toHaveBeenCalledWith("sample.dcl.eth", expect.anything());
    expect(loadWorldScenes).toHaveBeenCalledWith("sample.dcl.eth", expect.anything());
    expect(result).toMatchObject({ data: { place: { id: "sample.dcl.eth" }, livePlayers: 2, scenes: [{ entityId: "bafy1" }] }, init: { status: 200 } });
  });

  it("should answer not found for a name no world has", async () => {
    vi.mocked(loadWorldPlace).mockResolvedValueOnce(null);
    const result = await open("missing.dcl.eth");
    expect(result).toMatchObject({ data: { place: null, scenes: [] }, init: { status: 404 } });
  });

  it("should send a travel retry back to the world's own address", async () => {
    const result = await open("sample.dcl.eth", "?retry_travel=1");
    expect((result as Response).status).toBe(303);
    expect((result as Response).headers.get("Location")).toBe("/places/world/sample.dcl.eth");
  });
});
