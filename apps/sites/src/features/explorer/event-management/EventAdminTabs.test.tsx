// @vitest-environment jsdom
import { cleanup, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { AuthIdentity } from "@/data/lib/auth/types";
import { signedGetJSON } from "@/data/lib/catalyst/client";
import { fakeEventsApi, makeIdentity, renderAdminPage, type FakeEventsApi } from "./admin.test-utils";
import { EventAdminTabs } from "./EventAdminTabs";

const auth = vi.hoisted(() => ({ identity: null as AuthIdentity | null, isConnected: false }));
vi.hoisted(() => { Object.assign(window, { __vite_plugin_react_preamble_installed__: true }); });
vi.mock("@/data/lib/auth/context", () => ({ useAuth: () => auth }));
vi.mock("@/data/lib/catalyst/client", async (original) => ({
  ...await original<typeof import("@/data/lib/catalyst/client")>(),
  postJSON: vi.fn(), signedGetJSON: vi.fn(),
}));

describe("when the events page renders the admin bar", () => {
  let api: FakeEventsApi;

  beforeEach(() => {
    auth.identity = makeIdentity();
    auth.isConnected = true;
    api = { profile: { user: auth.identity.signer, permissions: [] }, profiles: "forbidden", events: [], failNext: {} };
    fakeEventsApi(api);
  });

  afterEach(() => {
    cleanup();
    vi.resetAllMocks();
  });

  describe("and the visitor is signed out", () => {
    beforeEach(() => {
      auth.identity = null;
      auth.isConnected = false;
    });

    it("should render nothing and read no permissions", () => {
      renderAdminPage(<EventAdminTabs active="events" />, "/x");
      expect(screen.queryByRole("navigation")).toBeNull();
      expect(signedGetJSON).not.toHaveBeenCalled();
    });
  });

  it("should render nothing for an account without event permissions", async () => {
    renderAdminPage(<EventAdminTabs active="events" />, "/x");
    await vi.waitFor(() => expect(signedGetJSON).toHaveBeenCalled());
    expect(screen.queryByRole("navigation")).toBeNull();
  });

  describe("and the account may only approve its own events", () => {
    beforeEach(() => { api.profile.permissions = ["approve_own_event"]; });

    it("should show Events and Pending Events but not Users", async () => {
      renderAdminPage(<EventAdminTabs active="events" />, "/x");
      const bar = await screen.findByRole("navigation", { name: "Event administration" });
      expect(within(bar).getAllByRole("link").map((link) => link.textContent)).toEqual(["Events", "Pending Events", "+ Create Event"]);
      expect(within(bar).getByRole("link", { name: "Events" }).getAttribute("aria-current")).toBe("page");
    });
  });

  describe("and the account is a moderator who manages users", () => {
    beforeEach(() => {
      api.profile.permissions = ["approve_any_event", "edit_any_profile"];
      api.profiles = [api.profile];
    });

    it("should show all three tabs", async () => {
      renderAdminPage(<EventAdminTabs active="events" />, "/x");
      const bar = await screen.findByRole("navigation", { name: "Event administration" });
      expect(await within(bar).findByRole("link", { name: "Users" })).toBeTruthy();
      expect(within(bar).getByRole("link", { name: "Pending Events" }).getAttribute("href")).toBe("/events/admin/pending-events");
    });
  });
});
