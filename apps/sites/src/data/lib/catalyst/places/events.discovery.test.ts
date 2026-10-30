import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fetchEvents } from "./events";

describe("when discovery reads an event page", () => {
  let fetchImpl: typeof fetch;
  let body: unknown;
  let requested: URL | undefined;
  beforeEach(() => {
    body = { ok: true, data: [] };
    requested = undefined;
    fetchImpl = vi.fn(async (input: RequestInfo | URL) => {
      requested = new URL(String(input));
      return new Response(JSON.stringify(body), { headers: { "content-type": "application/json" } });
    }) as typeof fetch;
  });
  afterEach(() => vi.restoreAllMocks());

  it("should send pagination, recurrence and presence filters to the service", async () => {
    await fetchEvents({ list: "active", offset: 120, limit: 25, recurrent: false, with_connected_users: true, from: "2099-06-01T00:00:00Z", to: "2099-06-02T00:00:00Z" }, { base: "https://node.example", fetchImpl });
    expect(Object.fromEntries(requested!.searchParams)).toMatchObject({ offset: "120", limit: "25", recurrent: "false", with_connected_users: "true", from: "2099-06-01T00:00:00Z", to: "2099-06-02T00:00:00Z" });
  });

  describe("and the response is malformed", () => {
    beforeEach(() => { body = { unexpected: true }; });
    it("should report an unavailable catalogue rather than an empty successful page", async () => {
      await expect(fetchEvents({}, { base: "https://node.example", fetchImpl })).rejects.toThrow("events envelope failed validation");
    });
  });

  describe("and a row is malformed", () => {
    beforeEach(() => { body = { ok: true, data: [{ id: "missing-fields" }] }; });
    it("should fail visibly instead of shortening the page and hiding later results", async () => {
      await expect(fetchEvents({}, { base: "https://node.example", fetchImpl })).rejects.toThrow("event payload failed validation");
    });
  });
});
