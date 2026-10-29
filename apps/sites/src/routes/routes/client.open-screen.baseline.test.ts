import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/features/explorer/open-screen/OpenScreen", () => ({
  default: () => null,
}));
vi.mock("@/data/lib/catalyst/places/index.server", () => ({
  loadPlaces: vi.fn(),
}));
vi.mock("@/core/lib/telemetry/page-loader", () => ({
  pageLoader: vi.fn(async () => ({
    sid: "sid-test",
    wrap: (payload: unknown) => payload,
  })),
}));

import { loadPlaces } from "@/data/lib/catalyst/places/index.server";
import { loader } from "./client.open-screen";

const places = [{ id: "place-1", title: "Genesis Plaza" }];

function args(): Parameters<typeof loader>[0] {
  return {
    request: new Request("https://sites.test/client/open-screen"),
    params: {},
    context: {} as never,
  } as unknown as Parameters<typeof loader>[0];
}

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(loadPlaces).mockResolvedValue({ data: places } as never);
});

describe("GET /client/open-screen", () => {
  it("loads the default Places list without flag resolution", async () => {
    await expect(loader(args())).resolves.toEqual({ sid: "sid-test", places });
    expect(loadPlaces).toHaveBeenCalledWith({ limit: 24 });
  });
});
