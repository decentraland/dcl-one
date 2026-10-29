import { describe, expect, it, vi } from "vitest";

import { loadLandList } from "./land-list";

const WALLET = "0x1234567890123456789012345678901234567890";

describe("LAND list", () => {
  it("reads owned LAND and direct grants from the selected realm", async () => {
    const fetchMock = vi.fn(async (url: string | URL | Request) => {
      const path = String(url);
      const body = path.includes("lands-permissions")
        ? { elements: [{ id: "7", x: "1", y: "2", owner: WALLET }], totalAmount: 1 }
        : { elements: [{ tokenId: "3", x: "3", y: "4", viaEstate: false }], totalAmount: 1 };
      return new Response(JSON.stringify(body), { status: 200 });
    });
    const fetchImpl = fetchMock as unknown as typeof fetch;
    const result = await loadLandList(WALLET, {
      base: "https://interconnected.online",
      fetchImpl,
    });
    expect(result.owned[0]).toMatchObject({ x: "3", y: "4" });
    expect(result.delegated[0]).toMatchObject({ x: "1", y: "2" });
    expect(fetchMock.mock.calls.every(([url]) => String(url).startsWith("https://interconnected.online/lambdas/users/"))).toBe(true);
  });

  it("does not turn a broken response into an empty LAND list", async () => {
    const fetchImpl = vi.fn(async () => new Response("{}", { status: 200 })) as unknown as typeof fetch;
    await expect(loadLandList(WALLET, { fetchImpl })).rejects.toThrow();
  });

  it("still returns owned parcels when the grants source fails", async () => {
    const fetchImpl = vi.fn(async (url: string | URL | Request) =>
      String(url).includes("lands-permissions")
        ? new Response("unavailable", { status: 503 })
        : new Response(JSON.stringify({
          elements: [{ tokenId: "3", x: "3", y: "4", viaEstate: false }],
          totalAmount: 1,
        })),
    ) as unknown as typeof fetch;
    const result = await loadLandList(WALLET, { fetchImpl });
    expect(result.owned).toHaveLength(1);
    expect(result.delegated).toEqual([]);
    expect(result.delegatedError).toBeTruthy();
  });
});
