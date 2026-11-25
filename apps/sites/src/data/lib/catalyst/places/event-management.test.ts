import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
vi.mock("../client", async (original) => ({ ...await original<typeof import("../client")>(), postJSON: vi.fn(), signedGetJSON: vi.fn() }));
import { CatalystError, postJSON, signedGetJSON } from "../client";
import { eventAdminKeys, loadAllOwnedEvents, loadEventAccess, loadEventGrants, loadEventProfile, loadEventProfileSettings, loadModerationEvents, loadOwnedEvents, moderateEvent, saveEventPermissions } from "./event-management";
import type { AuthIdentity } from "../../auth/types";
import type { ManagedEvent } from "./event-management";

const makeEvent = (id: string): ManagedEvent => ({
  id, name: "Fixture", image: null, image_vertical: null, description: null,
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
});

describe("when managing authenticated hangouts", () => {
  let identity: AuthIdentity;

  beforeEach(() => {
    identity = { signer: "0x1111111111111111111111111111111111111111", ephemeral: { address: "0x2222222222222222222222222222222222222222", privateKey: "0x01" }, expiration: "2099-01-01T00:00:00Z", authChain: [] };
    vi.mocked(signedGetJSON).mockResolvedValue({ ok: true, data: [] });
    vi.mocked(postJSON).mockResolvedValue({ ok: true, data: { user: identity.signer, permissions: ["approve_any_event"], approved: true, rejected: false } });
  });

  afterEach(() => vi.resetAllMocks());

  it("should scope a page of My Hangouts to the signed owner", async () => {
    await loadOwnedEvents(identity, { limit: 25, offset: 25, search: "music" });
    expect(signedGetJSON).toHaveBeenCalledWith("/events/api/events", expect.objectContaining({ identity, query: expect.objectContaining({ owner: true, list: "all", offset: 25, limit: 26, search: "music" }) }));
  });

  describe("and the owner's events span more than one page", () => {
    // Newest first: the soonest upcoming event was created first, so it is on the last page.
    const pageOne = Array.from({ length: 101 }, (_, index) => makeEvent(`newer-${index}`));
    const soonest = { ...makeEvent("soonest"), start_at: "2099-01-01T10:00:00Z" };

    it("should read every page so the soonest event on page two is kept", async () => {
      vi.mocked(signedGetJSON)
        .mockResolvedValueOnce({ ok: true, data: pageOne })
        .mockResolvedValueOnce({ ok: true, data: [soonest] });
      const events = await loadAllOwnedEvents(identity);
      expect(events).toHaveLength(101);
      expect(events.map((event) => event.id)).toContain("soonest");
      expect(vi.mocked(signedGetJSON).mock.calls.map(([, init]) => (init as unknown as { query: { offset: number } }).query.offset)).toEqual([0, 100]);
    });

    it("should stop with an error when a page repeats instead of advancing", async () => {
      vi.mocked(signedGetJSON).mockResolvedValue({ ok: true, data: pageOne });
      await expect(loadAllOwnedEvents(identity)).rejects.toThrow("did not advance");
      expect(signedGetJSON).toHaveBeenCalledTimes(2);
    });
  });

  it("should read one page when the owner has no events", async () => {
    await expect(loadAllOwnedEvents(identity)).resolves.toEqual([]);
    expect(signedGetJSON).toHaveBeenCalledTimes(1);
  });

  describe("and the signed moderation catalogue contains mixed states", () => {
    beforeEach(() => {
      const now = Date.now();
      const future = new Date(now + 60_000).toISOString();
      const insideWindow = new Date(now - 23 * 3_600_000).toISOString();
      const outsideWindow = new Date(now - 25 * 3_600_000).toISOString();
      const firstPage = Array.from({ length: 100 }, (_, index) => makeEvent(String(index)));
      const pending = { ...makeEvent("pending"), finish_at: future };
      vi.mocked(signedGetJSON)
        .mockResolvedValueOnce({ ok: true, data: [...firstPage, pending] })
        .mockResolvedValueOnce({ ok: true, data: [
          pending,
          { ...makeEvent("recurring"), recurrent: true, finish_at: insideWindow, next_start_at: future, next_finish_at: future },
          { ...makeEvent("approved"), approved: true, updated_at: insideWindow },
          { ...makeEvent("older"), approved: true, updated_at: outsideWindow },
          { ...makeEvent("missing-date"), approved: true },
          { ...makeEvent("rejected"), finish_at: future, rejected: true },
          { ...makeEvent("deleted"), approved: true, updated_at: insideWindow, deleted_by_admin: true },
        ] });
    });

    it("should request all active states and separate pending events from approvals in the last 24 hours", async () => {
      const result = await loadModerationEvents(identity);
      expect(result.events.map((row) => row.id)).toEqual(["pending", "recurring"]);
      expect(result.recentlyApproved.map((row) => row.id)).toEqual(["approved"]);
      expect(signedGetJSON).toHaveBeenCalledWith("/events/api/events", expect.objectContaining({ identity, query: { list: "active", order: "asc", offset: 0, limit: 101 } }));
      expect(signedGetJSON).toHaveBeenCalledWith("/events/api/events", expect.objectContaining({ identity, query: { list: "active", order: "asc", offset: 100, limit: 101 } }));
    });
  });

  describe("and the signed catalogue repeats a full page", () => {
    beforeEach(() => {
      vi.mocked(signedGetJSON).mockResolvedValue({ ok: true, data: Array.from({ length: 101 }, (_, index) => makeEvent(String(index))) });
    });

    it("should report the paging failure after two requests", async () => {
      await expect(loadModerationEvents(identity)).rejects.toThrow("did not advance");
      expect(signedGetJSON).toHaveBeenCalledTimes(2);
    });
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

  describe("and reading one wallet's stored grants", () => {
    const wallet = "0x4444444444444444444444444444444444444444";

    it("should read the wallet's settings, signed, by its lowercased address", async () => {
      vi.mocked(signedGetJSON).mockResolvedValue({ ok: true, data: { user: wallet, permissions: ["edit_any_schedule"], email: null } });
      await expect(loadEventProfileSettings(identity, wallet.toUpperCase().replace("0X", "0x"))).resolves.toEqual({ user: wallet, permissions: ["edit_any_schedule"] });
      expect(signedGetJSON).toHaveBeenCalledWith(`/events/api/profiles/${wallet}/settings`, expect.objectContaining({ identity }));
    });

    it("should answer null for a wallet that never saved settings", async () => {
      vi.mocked(signedGetJSON).mockResolvedValue({ ok: true, data: null });
      await expect(loadEventProfileSettings(identity, wallet)).resolves.toBeNull();
    });

    it("should fail on a refused read", async () => {
      vi.mocked(signedGetJSON).mockRejectedValue(new CatalystError("Forbidden", "settings", 403));
      await expect(loadEventProfileSettings(identity, wallet)).rejects.toThrow("Forbidden");
    });

    it("should reject an invalid address without reading", async () => {
      await expect(loadEventProfileSettings(identity, "invalid")).rejects.toThrow("valid wallet address");
      expect(signedGetJSON).not.toHaveBeenCalled();
    });
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
      await expect(loadOwnedEvents(identity, { offset: 25, limit: 25 })).resolves.toEqual({ events: [], hasMore: true });
    });
    it("should stop at the total", async () => {
      await expect(loadOwnedEvents(identity, { offset: 50, limit: 25 })).resolves.toEqual({ events: [], hasMore: false });
    });
  });

  describe.each([25, 26])("and a bare page contains %i rows without a total", (count) => {
    let rows: ManagedEvent[];
    beforeEach(() => {
      rows = Array.from({ length: count }, (_, index) => makeEvent(String(index)));
      vi.mocked(signedGetJSON).mockResolvedValue({ ok: true, data: rows });
    });
    it("should show exactly one page and only enable Next for a lookahead row", async () => {
      await expect(loadOwnedEvents(identity, { offset: 25, limit: 25 })).resolves.toEqual({ events: rows.slice(0, 25), hasMore: count > 25 });
    });
  });

  describe("and reading the account's admin access", () => {
    const answer = (permissions: string[], list: () => unknown) => {
      vi.mocked(signedGetJSON).mockImplementation(async (path: string) =>
        path === "/events/api/profiles/me/settings" ? { ok: true, data: { user: identity.signer, permissions } } : list());
    };

    it("should read only the account's own grants, even with the profile permission", async () => {
      answer(["edit_any_profile"], () => { throw new CatalystError("Unavailable", "list", 503); });
      await expect(loadEventAccess(identity)).resolves.toEqual({ profile: { user: identity.signer, permissions: ["edit_any_profile"] } });
      expect(signedGetJSON).toHaveBeenCalledTimes(1);
    });

    it("should read every account's grants for a moderator", async () => {
      answer(["edit_any_profile"], () => ({ ok: true, data: [{ user: identity.signer, permissions: ["edit_any_profile"] }] }));
      await expect(loadEventGrants(identity)).resolves.toMatchObject([{ user: identity.signer }]);
    });

    it("should treat a refused list as no user administration", async () => {
      answer(["edit_any_profile"], () => { throw new CatalystError("Forbidden", "list", 403); });
      await expect(loadEventGrants(identity)).resolves.toBeNull();
    });

    it("should fail on any other list failure", async () => {
      answer(["edit_any_profile"], () => { throw new CatalystError("Unavailable", "list", 503); });
      await expect(loadEventGrants(identity)).rejects.toThrow("Unavailable");
    });

    it("should keep each account's admin reads under its own private key", () => {
      expect(eventAdminKeys.access("0xABC")).toEqual(["private", "0xabc", "event-admin", "access"]);
      expect(eventAdminKeys.queue("0xABC")).toEqual(["private", "0xabc", "event-admin", "queue"]);
      expect(eventAdminKeys.grants("0xABC")).toEqual(["private", "0xabc", "event-admin", "grants"]);
    });
  });
});
