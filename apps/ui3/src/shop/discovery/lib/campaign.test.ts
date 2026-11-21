import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { fetchCampaignContracts } from "./campaign";

describe("fetchCampaignContracts", () => {
  let fetchMock: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    fetchMock = vi.fn(async () => new Response(JSON.stringify({ ok: true, data: ["0x" + "A".repeat(40)] })));
    vi.stubGlobal("fetch", fetchMock);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  describe("when a campaign names tags", () => {
    it("should read the Shop's campaign collections for every tag", async () => {
      const contracts = await fetchCampaignContracts(["summer", " event "]);
      const url = new URL(String(fetchMock.mock.calls[0]?.[0]), "http://node.example");
      expect(url.pathname).toMatch(/\/shop-api\/v1\/campaign-collections$/);
      expect(url.searchParams.getAll("tag")).toEqual(["summer", "event"]);
      expect(contracts).toEqual(["0x" + "a".repeat(40)]);
    });
  });
});
