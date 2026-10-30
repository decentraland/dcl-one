import { afterEach, describe, expect, it, vi } from "vitest";
import { loader } from "./create.activity";
import { loader as worldLoader } from "./create.activity_.$world";

const ADDRESS = "0x1111111111111111111111111111111111111111";
afterEach(() => vi.restoreAllMocks());

describe("retired Activity routes", () => {
  it.each([
    ["/create/activity", loader],
    ["/create/activity/example.dcl.eth", worldLoader],
  ] as const)("redirects %s to Metrics without fetching upstream activity", async (path, load) => {
    const fetchSpy = vi.spyOn(globalThis, "fetch").mockRejectedValue(new Error("Unexpected upstream request"));
    const response = await load({ request: new Request("https://sites.test" + path), params: {}, context: {} } as never);
    expect(response.status).toBe(308);
    expect(response.headers.get("Location")).toBe("/create/metrics");
    expect(fetchSpy).not.toHaveBeenCalled();
  });

  it("keeps the address scope and drops obsolete Activity filters", async () => {
    const response = await loader({
      request: new Request("https://sites.test/create/activity?address=" + ADDRESS + "&pointer=0,0&from=activity"),
      params: {}, context: {},
    } as never);
    expect(response.headers.get("Location")).toBe("/create/metrics?address=" + ADDRESS);
  });
});
