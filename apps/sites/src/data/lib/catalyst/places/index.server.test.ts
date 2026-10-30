import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { apiSource } from "../api";
import { CatalystError } from "../client";
import { dbSource } from "../db.server";
import { loadPlace } from "./index.server";

vi.mock("../db.server", () => ({
  dbSource: { fetchPlace: vi.fn(), fetchPlaces: vi.fn(), fetchCategories: vi.fn() },
}));
vi.mock("../api", () => ({
  apiSource: { fetchPlace: vi.fn(), fetchPlaces: vi.fn(), fetchCategories: vi.fn() },
}));

const CANONICAL = { id: "66ef7bd9-3060-4e1a-a6d7-edc8d251e0cb", base_position: "34,17" };
const PARCEL_ALIAS = "3ae8c888-1c33-5c77-b82f-ab14bd047f13";

beforeEach(() => {
  vi.clearAllMocks();
  vi.stubEnv("CATALYST_DATABASE_URL", "postgresql:///places");
});
afterEach(() => vi.unstubAllEnvs());

describe("loadPlace", () => {
  it("lets the API resolve an id the place table does not carry, without a fallback warning", async () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    vi.mocked(dbSource.fetchPlace).mockRejectedValueOnce(
      new CatalystError(`place ${PARCEL_ALIAS} not found`, "db://place", 404),
    );
    vi.mocked(apiSource.fetchPlace).mockResolvedValueOnce(CANONICAL as never);

    await expect(loadPlace(PARCEL_ALIAS)).resolves.toEqual(CANONICAL);
    expect(apiSource.fetchPlace).toHaveBeenCalledWith(PARCEL_ALIAS, {});
    expect(warn).not.toHaveBeenCalled();
    warn.mockRestore();
  });

  it("serves a direct table hit without touching the API", async () => {
    vi.mocked(dbSource.fetchPlace).mockResolvedValueOnce(CANONICAL as never);
    await expect(loadPlace(CANONICAL.id)).resolves.toEqual(CANONICAL);
    expect(apiSource.fetchPlace).not.toHaveBeenCalled();
  });

  it("surfaces the API's own 404 when neither source knows the id", async () => {
    vi.mocked(dbSource.fetchPlace).mockRejectedValueOnce(new CatalystError("place x not found", "db://place", 404));
    vi.mocked(apiSource.fetchPlace).mockRejectedValueOnce(new CatalystError("Not found place", "/api/places/x", 404));
    await expect(loadPlace("x")).rejects.toMatchObject({ status: 404 });
  });
});
