// @vitest-environment jsdom
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AuthIdentity } from "@/data/lib/auth/types";
import { CatalystError } from "@/data/lib/catalyst/client";
import { loadEventProfile, loadEventProfiles, loadManagedEvents, moderateEvent, saveEventPermissions, type EventProfile, type ManagedEvent } from "@/data/lib/catalyst/places/event-management";
import EventManagement, { EventDiscoveryNavigation } from "./EventManagement";

const auth = vi.hoisted(() => ({ identity: null as AuthIdentity | null, isConnected: false }));
vi.hoisted(() => { Object.assign(window, { __vite_plugin_react_preamble_installed__: true }); });
vi.mock("@/data/lib/auth/context", () => ({ useAuth: () => auth }));
vi.mock("@/data/lib/auth/session", () => ({ getIdentity: () => auth.identity }));
vi.mock("@ui/web/frames/SitesChrome", () => ({ default: ({ children }: { children: React.ReactNode }) => <>{children}</> }));
vi.mock("@/data/lib/catalyst/places/event-management", async (original) => ({
  ...await original<typeof import("@/data/lib/catalyst/places/event-management")>(),
  loadEventProfile: vi.fn(), loadEventProfiles: vi.fn(), loadManagedEvents: vi.fn(), moderateEvent: vi.fn(), saveEventPermissions: vi.fn(),
}));

describe("when managing hangouts", () => {
  let identity: AuthIdentity;
  let profile: EventProfile;
  let event: ManagedEvent;
  let user: ReturnType<typeof userEvent.setup>;
  let renderPage: (mode?: "mine" | "pending" | "users") => ReturnType<typeof render>;
  let view: ReturnType<typeof render>;
  beforeEach(() => {
    identity = { signer: "0x1111111111111111111111111111111111111111", ephemeral: { address: "0x2222222222222222222222222222222222222222", privateKey: "0x01" }, expiration: "2099-01-01T00:00:00Z", authChain: [] };
    auth.identity = identity; auth.isConnected = true;
    profile = { user: identity.signer, permissions: ["approve_any_event"] };
    // Adapter mock: only the rendered fields are needed by this UI fixture.
    event = { id: "pending-one", name: "Moon concert", user: identity.signer, description: "A concert", image: null, start_at: "2099-01-01T00:00:00Z", finish_at: "2099-01-02T00:00:00Z", approved: false, rejected: false, deleted_by_user: false, deleted_by_admin: false, world: true, server: null } as ManagedEvent;
    vi.mocked(loadEventProfile).mockResolvedValue(profile);
    vi.mocked(loadEventProfiles).mockResolvedValue([profile]);
    vi.mocked(loadManagedEvents).mockResolvedValue({ events: [event], hasMore: true });
    vi.mocked(moderateEvent).mockResolvedValue();
    vi.mocked(saveEventPermissions).mockResolvedValue(profile);
    user = userEvent.setup();
    renderPage = (mode = "pending") => render(<MemoryRouter><EventManagement mode={mode} /></MemoryRouter>);
  });
  afterEach(() => { cleanup(); vi.resetAllMocks(); });

  it("should page pending events and display an unpublished world location", async () => {
    renderPage();
    await screen.findByRole("button", { name: "Approve Moon concert" });
    expect(screen.getByText(/World location not published/)).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Next page" }));
    await waitFor(() => expect(loadManagedEvents).toHaveBeenLastCalledWith(identity, expect.objectContaining({ own: false, offset: 25 }), expect.any(AbortSignal)));
  });

  it("should approve the selected event and report confirmed success", async () => {
    renderPage();
    await user.click(await screen.findByRole("button", { name: "Approve Moon concert" }));
    await screen.findByText("Hangout approved.");
    expect(moderateEvent).toHaveBeenCalledWith(identity, "pending-one", "approve", undefined);
  });

  describe("and moderation was superseded", () => {
    beforeEach(() => { vi.mocked(moderateEvent).mockRejectedValueOnce(new Error("The hangout changed. Refresh.")); });
    it("should show the failure without claiming success", async () => {
      renderPage();
      await user.click(await screen.findByRole("button", { name: "Approve Moon concert" }));
      await screen.findByText("The hangout changed. Refresh.");
      expect(screen.queryByText("Hangout approved.")).toBeNull();
    });
  });

  it("should reject the named event with a reason", async () => {
    renderPage();
    await user.click(await screen.findByRole("button", { name: "Reject Moon concert" }));
    expect(screen.getByRole("region", { name: "Reject Moon concert" })).toBeTruthy();
    await user.type(screen.getByRole("textbox", { name: "Reason" }), "Wrong image");
    await user.click(screen.getByRole("button", { name: "Confirm rejection" }));
    await screen.findByText("Hangout rejected.");
    expect(moderateEvent).toHaveBeenCalledWith(identity, "pending-one", "reject", "Wrong image");
  });

  it("should clear rejection when paging", async () => {
    renderPage();
    await user.click(await screen.findByRole("button", { name: "Reject Moon concert" }));
    await user.click(screen.getByRole("button", { name: "Next page" }));
    expect(screen.queryByRole("button", { name: "Confirm rejection" })).toBeNull();
  });

  it("should clear rejection and debounce event search", async () => {
    renderPage();
    await user.click(await screen.findByRole("button", { name: "Reject Moon concert" }));
    await user.type(screen.getByRole("searchbox"), "music");
    expect(screen.queryByRole("button", { name: "Confirm rejection" })).toBeNull();
    expect(loadManagedEvents).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(loadManagedEvents).toHaveBeenCalledTimes(2));
    expect(loadManagedEvents).toHaveBeenLastCalledWith(identity, expect.objectContaining({ search: "music" }), expect.any(AbortSignal));
  });

  it("should preserve a draft after a same-wallet storage update", async () => {
    view = renderPage();
    await user.click(await screen.findByRole("button", { name: "Reject Moon concert" }));
    await user.type(screen.getByRole("textbox", { name: "Reason" }), "Draft reason");
    auth.identity = { ...identity };
    view.rerender(<MemoryRouter><EventManagement mode="pending" /></MemoryRouter>);
    expect((screen.getByRole("textbox", { name: "Reason" }) as HTMLTextAreaElement).value).toBe("Draft reason");
    expect(loadEventProfile).toHaveBeenCalledTimes(1);
  });

  it("should reset the page and draft before fetching for another wallet", async () => {
    view = renderPage("mine");
    await screen.findByRole("button", { name: "Approve Moon concert" });
    await user.click(screen.getByRole("button", { name: "Next page" }));
    await screen.findByText("Page 2");
    await user.click(await screen.findByRole("button", { name: "Reject Moon concert" }));
    auth.identity = { ...identity, signer: "0x3333333333333333333333333333333333333333" };
    view.rerender(<MemoryRouter><EventManagement mode="mine" /></MemoryRouter>);
    await screen.findByText("Page 1");
    expect(screen.queryByRole("button", { name: "Confirm rejection" })).toBeNull();
    await waitFor(() => expect(loadManagedEvents).toHaveBeenLastCalledWith(auth.identity, expect.objectContaining({ offset: 0 }), expect.any(AbortSignal)));
    expect(vi.mocked(loadManagedEvents).mock.calls.filter(([account]) => account.signer === auth.identity?.signer)).toHaveLength(1);
  });

  describe("and the account may only approve its own events", () => {
    beforeEach(() => { profile.permissions = ["approve_own_event"]; });
    it("should deny the all-owner pending view without loading a misleading catalogue", async () => {
      renderPage();
      await screen.findByRole("alert");
      expect(loadManagedEvents).not.toHaveBeenCalled();
      expect(screen.queryByRole("link", { name: "Moderation" })).toBeNull();
    });
    it("should provide own-event approval from My Hangouts", async () => {
      renderPage("mine");
      await user.click(await screen.findByRole("button", { name: "Approve Moon concert" }));
      await screen.findByText("Hangout approved.");
    });
  });

  describe("and an edit-profile holder is not a moderator", () => {
    beforeEach(() => {
      profile.permissions = ["edit_any_profile"];
      vi.mocked(loadEventProfiles).mockRejectedValue(new CatalystError("Forbidden", "test", 403));
    });
    it("should deny administration without exposing the editor or navigation", async () => {
      renderPage("users");
      await screen.findByRole("alert");
      expect(screen.queryByRole("link", { name: "User permissions" })).toBeNull();
      expect(screen.queryByRole("button", { name: "Add permissions for a wallet" })).toBeNull();
    });
  });

  describe("and the account is a moderator", () => {
    beforeEach(() => {
      profile.permissions = ["edit_any_profile", "approve_any_event"];
      vi.mocked(loadEventProfiles).mockResolvedValue(Array.from({ length: 26 }, (_, i) => ({ user: `0x${String(i).padStart(40, "0")}`, permissions: [] })));
    });
    it("should page and filter profiles locally without refetching", async () => {
      renderPage("users");
      await screen.findByRole("button", { name: "Add permissions for a wallet" });
      await user.click(screen.getByRole("button", { name: "Next page" }));
      await screen.findByText("Page 2");
      await user.type(screen.getByRole("searchbox"), "000");
      await screen.findByText("Page 1");
      expect(loadEventProfiles).toHaveBeenCalledTimes(1);
    });
    it("should save permissions and refresh the authorized list", async () => {
      renderPage("users");
      await user.click(await screen.findByRole("button", { name: "Add permissions for a wallet" }));
      await user.type(screen.getByRole("textbox", { name: "Wallet address" }), identity.signer);
      await user.click(screen.getByRole("checkbox", { name: "Approve any hangout" }));
      await user.click(screen.getByRole("button", { name: "Save permissions" }));
      await screen.findByText("Permissions saved.");
      expect(saveEventPermissions).toHaveBeenCalledWith(identity, identity.signer, ["approve_any_event"]);
    });
  });

  it("should render public navigation without private settings requests", () => {
    render(<MemoryRouter><EventDiscoveryNavigation /></MemoryRouter>);
    expect(loadEventProfile).not.toHaveBeenCalled();
    expect(loadEventProfiles).not.toHaveBeenCalled();
  });
});
