import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fetchEvents, fetchEventPage } from "./events";

describe("when discovery reads an event page", () => {
  let fetchImpl: typeof fetch;
  let body: unknown;
  let record: Record<string, unknown>;
  let requested: URL | undefined;
  beforeEach(() => {
    record = {
      id: "valid", name: "Valid event", image: null, image_vertical: null, description: null,
      start_at: "2099-06-01T18:00:00Z", finish_at: "2099-06-01T20:00:00Z", next_start_at: null, next_finish_at: null,
      duration: 7_200_000, all_day: false, x: 0, y: 0, server: null, url: null, user: null, user_name: null,
      estate_id: null, estate_name: null, scene_name: null, approved: true, rejected: false, highlighted: false, trending: false, world: false,
      recurrent: false, recurrent_frequency: null, recurrent_weekday_mask: 0, recurrent_month_mask: 0, recurrent_interval: 1,
      recurrent_setpos: null, recurrent_monthday: null, recurrent_count: null, recurrent_until: null, recurrent_dates: [], categories: [], schedules: [],
      total_attendees: 0, latest_attendees: [], coordinates: [0, 0], position: [0, 0], live: false, attending: false, place_id: null, community_id: null,
      featured_item: null, created_at: null, updated_at: null, approved_by: null, rejected_by: null, rejection_reason: null,
      deleted_by_user: false, deleted_by_admin: false, deleted_by: null, deleted_at: null, deleted_reason: null, previous_place_id: null,
    };
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
    beforeEach(() => {
      body = { ok: true, data: [{ ...record, name: 42 }, record, { ...record, id: "lookahead" }] };
      vi.spyOn(console, "warn").mockImplementation(() => {});
    });
    it("should preserve valid rows and report the malformed record", async () => {
      await expect(fetchEvents({}, { base: "https://node.example", fetchImpl })).resolves.toMatchObject({ data: [{ id: "valid" }, { id: "lookahead" }], received: 3, invalidCount: 1 });
    });
    it("should preserve raw page boundaries and lookahead while skipping invalid rows", async () => {
      await expect(fetchEventPage({ limit: 2 }, { base: "https://node.example", fetchImpl })).resolves.toMatchObject({ data: [{ id: "valid" }], hasMore: true, invalidCount: 1 });
      expect(requested!.searchParams.get("limit")).toBe("3");
    });
  });
});
