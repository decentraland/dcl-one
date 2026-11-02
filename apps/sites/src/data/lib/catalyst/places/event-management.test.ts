import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
vi.mock("../client", async (original) => ({ ...await original<typeof import("../client")>(), postJSON: vi.fn(), signedGetJSON: vi.fn() }));
import { postJSON, signedGetJSON } from "../client";
import { loadEventProfile, loadManagedEvents, moderateEvent, saveEventPermissions } from "./event-management";
import type { AuthIdentity } from "../../auth/types";

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
    expect(signedGetJSON).toHaveBeenCalledWith("/events/api/events", expect.objectContaining({ identity, query: expect.objectContaining({ owner: true, list: "all", offset: 25, limit: 25, search: "music" }) }));
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
      await expect(moderateEvent(identity, "event-one", "approve")).rejects.toThrow("changed before this action");
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

});
