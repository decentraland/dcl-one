import { beforeEach, describe, expect, it, vi } from "vitest";

import { CatalystError } from "@/data/lib/catalyst/client";
import { loadPlace } from "@/data/lib/catalyst/places/index.server";
import { loader } from "./places_.$id";

vi.mock("@/data/lib/catalyst/places/index.server", () => ({ loadPlace: vi.fn() }));
vi.mock("@/core/lib/telemetry/track", () => ({ track: vi.fn() }));
vi.mock("@/core/lib/telemetry/page-loader", () => ({
  pageLoaderWith: vi.fn(
    async (
      _request: Request,
      _defaults: unknown,
      load: (base: { sid: string }) => Promise<unknown>,
    ) => ({
      sid: "sid-1",
      data: await load({ sid: "sid-1" }),
      wrap: (payload: unknown, init?: { status?: number }) => ({ payload, status: init?.status ?? 200 }),
    }),
  ),
}));

const CANONICAL = "66ef7bd9-3060-4e1a-a6d7-edc8d251e0cb";
// uuid5(NAMESPACE_URL, "34,17"): the id the content-derived catalog minted for that parcel.
const PARCEL_ALIAS = "3ae8c888-1c33-5c77-b82f-ab14bd047f13";

function place(id: string) {
  return { id, title: "New Scene 12", base_position: "34,17", positions: ["34,17"], world: false };
}

async function run(path: string, id: string) {
  return loader({ request: new Request(`https://sites.test${path}`), params: { id }, context: {} } as never);
}

beforeEach(() => vi.clearAllMocks());

describe("/places/:id", () => {
  it("serves a place under its own id", async () => {
    vi.mocked(loadPlace).mockResolvedValueOnce(place(CANONICAL) as never);
    const result = (await run(`/places/${CANONICAL}`, CANONICAL)) as unknown as { status: number; payload: { id: string } };
    expect(result.status).toBe(200);
    expect(result.payload.id).toBe(CANONICAL);
  });

  it("redirects a derive-era parcel id to the canonical place URL, keeping the query", async () => {
    vi.mocked(loadPlace).mockResolvedValueOnce(place(CANONICAL) as never);
    const result = await run(`/places/${PARCEL_ALIAS}?from=open-screen`, PARCEL_ALIAS);
    expect(result).toBeInstanceOf(Response);
    const response = result as Response;
    expect(response.status).toBe(301);
    expect(response.headers.get("Location")).toBe(`/places/${CANONICAL}?from=open-screen`);
    expect(loadPlace).toHaveBeenCalledWith(PARCEL_ALIAS, expect.anything());
  });

  it("answers 404 for an id nothing resolves", async () => {
    vi.mocked(loadPlace).mockRejectedValueOnce(new CatalystError("not found", "/api/places/x", 404));
    const result = (await run("/places/x", "x")) as unknown as { status: number; payload: { place: unknown } };
    expect(result.status).toBe(404);
    expect(result.payload.place).toBeNull();
  });

  it("answers 503 when the places service is unreachable", async () => {
    vi.mocked(loadPlace).mockRejectedValueOnce(new Error("connect ECONNREFUSED"));
    const result = (await run("/places/x", "x")) as unknown as { status: number };
    expect(result.status).toBe(503);
  });
});
