import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
vi.mock("../client", async (original) => ({ ...await original<typeof import("../client")>(), postJSON: vi.fn(), signedGetJSON: vi.fn() }));
import { postJSON, signedGetJSON } from "../client";
import { loadEventProfile, loadManagedEvents, moderateEvent, saveEventPermissions } from "./event-management";
import type { AuthIdentity } from "../../auth/types";
import type { ManagedEvent } from "./event-management";

describe("when managing authenticated hangouts", () => {
  let identity: AuthIdentity;

  beforeEach(() => {
    identity = { signer: "0x1111111111111111111111111111111111111111", ephemeral: { address: "0x2222222222222222222222222222222222222222", privateKey: "0x01" }, expiration: "2099-01-01T00:00:00Z", authChain: [] };
    vi.mocked(signedGetJSON).mockResolvedValue({ ok: true, data: [] });
    vi.mocked(postJSON).mockResolvedValue({ ok: true, data: { user: identity.signer, permissions: ["approve_any_event"], approved: true, rejected: false } });
  });

  afterEach(() => vi.resetAllMocks());

  it("should scope a page of My Hangouts to the signed owner", async () => {
    await loadManagedEvents(identity, { own: true, limit: 25, offset: 25, search: "music" });
    expect(signedGetJSON).toHaveBeenCalledWith("/events/api/events", expect.objectContaining({ identity, query: expect.objectContaining({ owner: true, list: "all", offset: 25, limit: 26, search: "music" }) }));
  });

  it("should request only pending events for moderation", async () => {
    await loadManagedEvents(identity, { own: false, limit: 25, offset: 0 });
    expect(signedGetJSON).toHaveBeenCalledWith("/events/api/events", expect.objectContaining({ identity, query: expect.objectContaining({ list: "active", approved: false, rejected: false, deleted: false }) }));
  });

  it("should send a signed approval to the selected event", async () => {
    await moderateEvent(identity, "event/one", "approve");
    expect(postJSON).toHaveBeenCalledWith("/events/api/events/event%2Fone", { approved: true }, { identity, method: "PATCH" });
  });

  it("should send the rejection reason to the selected event", async () => {
    vi.mocked(postJSON).mockResolvedValueOnce({ ok: true, data: { approved: false, rejected: true } });
    await moderateEvent(identity, "event-one", "reject", "  Image needs revision  ");
    expect(postJSON).toHaveBeenCalledWith("/events/api/events/event-one", { rejected: true, rejection_reason: "Image needs revision" }, { identity, method: "PATCH" });
  });

  it("should save profile permissions through the signed compatibility endpoint", async () => {
    await saveEventPermissions(identity, identity.signer, ["approve_any_event"]);
    expect(postJSON).toHaveBeenCalledWith(`/events/api/profiles/${identity.signer}/settings`, { permissions: ["approve_any_event"] }, { identity, method: "PATCH" });
  });

  it("should reject settings from a different account", async () => {
    vi.mocked(signedGetJSON).mockResolvedValueOnce({ ok: true, data: { user: "0x3333333333333333333333333333333333333333", permissions: [] } });
    await expect(loadEventProfile(identity)).rejects.toThrow("another account");
  });

  it("should reject an invalid permission target without sending a write", async () => {
    await expect(saveEventPermissions(identity, "invalid", ["approve_any_event"])).rejects.toThrow("valid wallet address");
    expect(postJSON).not.toHaveBeenCalled();
  });
  describe("and moderation was not applied", () => {
    beforeEach(() => { vi.mocked(postJSON).mockResolvedValue({ ok: true, data: { approved: false, rejected: true } }); });
    it("should reject an approval superseded by rejection", async () => {
      await expect(moderateEvent(identity, "event-one", "approve")).rejects.toThrow("Another reviewer may be required");
    });
  });

  describe("and approval requires another reviewer", () => {
    beforeEach(() => { vi.mocked(postJSON).mockResolvedValue({ ok: true, data: { approved: false, rejected: false } }); });
    it("should explain the reviewer requirement without asserting the hangout changed", async () => {
      await expect(moderateEvent(identity, "event-one", "approve")).rejects.toThrow("Another reviewer may be required");
    });
  });

  describe.each([
    { shape: "bare array", data: [], total: 50 },
    { shape: "events object", data: { events: [], total: 50 }, total: undefined },
  ])("and the list uses $shape", ({ data, total }) => {
    beforeEach(() => { vi.mocked(signedGetJSON).mockResolvedValue({ ok: true, data, total }); });
    it("should honor the total beyond this page", async () => {
      await expect(loadManagedEvents(identity, { own: true, offset: 25, limit: 25 })).resolves.toEqual({ events: [], hasMore: true });
    });
    it("should stop at the total", async () => {
      await expect(loadManagedEvents(identity, { own: true, offset: 50, limit: 25 })).resolves.toEqual({ events: [], hasMore: false });
    });
  });

  describe.each([25, 26])("and a bare page contains %i rows without a total", (count) => {
    let rows: ManagedEvent[];
    beforeEach(() => {
      rows = Array.from({ length: count }, (_, index) => ({
        id: String(index), name: "Fixture", image: null, image_vertical: null, description: null,
        start_at: null, finish_at: null, next_start_at: null, next_finish_at: null, duration: null,
        all_day: false, x: 0, y: 0, server: null, url: null, user: null, user_name: null,
        estate_id: null, estate_name: null, scene_name: null, approved: false, rejected: false,
        highlighted: false, trending: false, world: false, recurrent: false, recurrent_frequency: null,
        recurrent_weekday_mask: 0, recurrent_month_mask: 0, recurrent_interval: 1, recurrent_setpos: null,
        recurrent_monthday: null, recurrent_count: null, recurrent_until: null, recurrent_dates: [],
        categories: [], schedules: [], total_attendees: 0, latest_attendees: [], coordinates: [0, 0],
        position: [0, 0], live: false, attending: false, place_id: null, community_id: null,
        featured_item: null, created_at: null, updated_at: null, approved_by: null, rejected_by: null,
        rejection_reason: null, deleted_by_user: false, deleted_by_admin: false, deleted_by: null,
        deleted_at: null, deleted_reason: null, previous_place_id: null,
      }));
      vi.mocked(signedGetJSON).mockResolvedValue({ ok: true, data: rows });
    });
    it("should show exactly one page and only enable Next for a lookahead row", async () => {
      await expect(loadManagedEvents(identity, { own: false, offset: 25, limit: 25 })).resolves.toEqual({ events: rows.slice(0, 25), hasMore: count > 25 });
    });
  });

});
