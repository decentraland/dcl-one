// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AuthIdentity } from "@/data/lib/auth/types";
import { CatalystError } from "@/data/lib/catalyst/client";
import { eventsPageKeys } from "@/data/lib/catalyst/places/events-page";
import { loadEventProfile, loadEventProfiles, loadModerationEvents, loadOwnedEvents, moderateEvent, saveEventPermissions, type EventProfile, type ManagedEvent } from "@/data/lib/catalyst/places/event-management";
import EventManagement from "./EventManagement";

const auth = vi.hoisted(() => ({ identity: null as AuthIdentity | null, isConnected: false }));
vi.hoisted(() => { Object.assign(window, { __vite_plugin_react_preamble_installed__: true }); });
vi.mock("@/data/lib/auth/context", () => ({ useAuth: () => auth }));
vi.mock("@/data/lib/auth/session", () => ({ getIdentity: () => auth.identity }));
vi.mock("@ui/web/frames/SitesChrome", () => ({ default: ({ children }: { children: React.ReactNode }) => <>{children}</> }));
vi.mock("@/data/lib/catalyst/places/event-management", async (original) => ({
  ...await original<typeof import("@/data/lib/catalyst/places/event-management")>(),
  loadEventProfile: vi.fn(), loadEventProfiles: vi.fn(), loadModerationEvents: vi.fn(), loadOwnedEvents: vi.fn(), moderateEvent: vi.fn(), saveEventPermissions: vi.fn(),
}));

describe("when managing hangouts", () => {
  let identity: AuthIdentity;
  let profile: EventProfile;
  let event: ManagedEvent;
  let user: ReturnType<typeof userEvent.setup>;
  let client: QueryClient;
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
    vi.mocked(loadModerationEvents).mockResolvedValue({ events: [event], recentlyApproved: [], hasMore: false });
    vi.mocked(loadOwnedEvents).mockResolvedValue({ events: [event], hasMore: true });
    vi.mocked(moderateEvent).mockResolvedValue();
    vi.mocked(saveEventPermissions).mockImplementation(async (_identity, address, permissions) => ({ user: address, permissions }));
    user = userEvent.setup();
    client = new QueryClient();
    client.setQueryData(eventsPageKeys.detail(event.id), event);
    renderPage = (mode = "pending") => render(<MemoryRouter><EventManagement mode={mode} /></MemoryRouter>, {
      wrapper: ({ children }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>,
    });
  });
  afterEach(() => { cleanup(); client.clear(); vi.resetAllMocks(); });

  it("should page owned events and display an unpublished world location", async () => {
    renderPage("mine");
    await screen.findByRole("button", { name: "Approve Moon concert" });
    expect(screen.getByText(/World location not published/)).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Next page" }));
    await waitFor(() => expect(loadOwnedEvents).toHaveBeenLastCalledWith(identity, expect.objectContaining({ offset: 25 }), expect.any(AbortSignal)));
  });

  it("should approve the selected event and report confirmed success", async () => {
    renderPage();
    await user.click(await screen.findByRole("button", { name: "Moon concert" }));
    const review = screen.getByRole("dialog", { name: "Moon concert" });
    expect(within(review).getByText("A concert")).toBeTruthy();
    expect(moderateEvent).not.toHaveBeenCalled();
    await user.click(within(review).getByRole("button", { name: "Approve event" }));
    await screen.findByText("Hangout approved.");
    expect(moderateEvent).toHaveBeenCalledWith(identity, "pending-one", "approve", undefined);
    expect(loadEventProfile).toHaveBeenCalledTimes(1);
    expect(client.getQueryState(eventsPageKeys.detail(event.id))?.isInvalidated).toBe(true);
  });

  describe("and approval moves an event to recent history", () => {
    beforeEach(() => {
      vi.mocked(loadModerationEvents)
        .mockResolvedValueOnce({ events: [event], recentlyApproved: [], hasMore: false })
        .mockResolvedValueOnce({ events: [], recentlyApproved: [{ ...event, approved: true }], hasMore: false });
    });

    it("should show only the moderation queue and remove actions from an approved event", async () => {
      renderPage();
      const pending = await screen.findByRole("region", { name: "Pending Events" });
      expect(screen.getByRole("region", { name: "Recently Approved (Last 24hs)" })).toBeTruthy();
      expect(screen.queryByRole("searchbox")).toBeNull();
      expect(screen.queryByRole("navigation", { name: "Hangout navigation" })).toBeNull();
      expect(screen.queryByRole("button", { name: "Next page" })).toBeNull();
      await user.click(within(pending).getByRole("button", { name: "Moon concert" }));
      await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Approve event" }));
      const history = await screen.findByRole("region", { name: "Recently Approved (Last 24hs)" });
      await user.click(within(history).getByRole("button", { name: "Moon concert" }));
      expect(within(screen.getByRole("region", { name: "Pending Events" })).queryByRole("button", { name: "Moon concert" })).toBeNull();
      expect(within(screen.getByRole("dialog")).queryByRole("button", { name: "Approve event" })).toBeNull();
      expect(within(screen.getByRole("dialog")).queryByRole("button", { name: "Reject" })).toBeNull();
    });
  });

  it("should leave the queue unchanged if review is cancelled", async () => {
    renderPage();
    await user.click(await screen.findByRole("button", { name: "Moon concert" }));
    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Close" }));
    expect(moderateEvent).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  describe("and moderation was superseded", () => {
    beforeEach(() => { vi.mocked(moderateEvent).mockRejectedValueOnce(new Error("The hangout changed. Refresh.")); });
    it("should show the failure without claiming success", async () => {
      renderPage();
      await user.click(await screen.findByRole("button", { name: "Moon concert" }));
      const review = screen.getByRole("dialog", { name: "Moon concert" });
      await user.click(within(review).getByRole("button", { name: "Approve event" }));
      expect((await within(review).findByRole("alert")).textContent).toBe("The hangout changed. Refresh.");
      expect(screen.queryByText("Hangout approved.")).toBeNull();
      expect(client.getQueryState(eventsPageKeys.detail(event.id))?.isInvalidated).toBe(false);
      await user.click(within(review).getByRole("button", { name: "Approve event" }));
      await screen.findByText("Hangout approved.");
      expect(moderateEvent).toHaveBeenCalledTimes(2);
    });
  });

  it("should reject the named event with a reason", async () => {
    renderPage();
    await user.click(await screen.findByRole("button", { name: "Moon concert" }));
    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Reject" }));
    expect(screen.getByRole("dialog", { name: "Reject Moon concert" })).toBeTruthy();
    expect(screen.getByRole<HTMLButtonElement>("button", { name: "Confirm rejection" }).disabled).toBe(true);
    await user.type(screen.getByRole("textbox", { name: "Reason" }), "Wrong image");
    await user.click(screen.getByRole("button", { name: "Confirm rejection" }));
    await screen.findByText("Hangout rejected.");
    expect(moderateEvent).toHaveBeenCalledWith(identity, "pending-one", "reject", "Wrong image");
  });

  it("should clear rejection when paging", async () => {
    renderPage("mine");
    await user.click(await screen.findByRole("button", { name: "Reject Moon concert" }));
    await user.click(screen.getByRole("button", { name: "Next page" }));
    expect(screen.queryByRole("button", { name: "Confirm rejection" })).toBeNull();
  });

  it("should dismiss the inline rejection when switching to approval review", async () => {
    renderPage("mine");
    await user.click(await screen.findByRole("button", { name: "Reject Moon concert" }));
    await user.type(screen.getByRole("textbox", { name: "Reason" }), "Draft");
    await user.click(screen.getByRole("button", { name: "Approve Moon concert" }));
    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("textbox", { name: "Reason" })).toBeNull();
    expect(moderateEvent).not.toHaveBeenCalled();
  });

  it("should clear rejection and debounce event search", async () => {
    renderPage("mine");
    await user.click(await screen.findByRole("button", { name: "Reject Moon concert" }));
    await user.type(screen.getByRole("searchbox"), "music");
    expect(screen.queryByRole("button", { name: "Confirm rejection" })).toBeNull();
    expect(loadOwnedEvents).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(loadOwnedEvents).toHaveBeenCalledTimes(2));
    expect(loadOwnedEvents).toHaveBeenLastCalledWith(identity, expect.objectContaining({ search: "music" }), expect.any(AbortSignal));
  });

  it("should combine the debounced search and page reset into one request", async () => {
    renderPage("mine");
    await screen.findByRole("button", { name: "Approve Moon concert" });
    await user.click(screen.getByRole("button", { name: "Next page" }));
    await waitFor(() => expect(loadOwnedEvents).toHaveBeenCalledTimes(2));
    await user.type(screen.getByRole("searchbox"), "music");
    expect(loadOwnedEvents).toHaveBeenCalledTimes(2);
    await waitFor(() => expect(loadOwnedEvents).toHaveBeenCalledTimes(3));
    expect(loadOwnedEvents).toHaveBeenLastCalledWith(identity, expect.objectContaining({ search: "music", offset: 0 }), expect.any(AbortSignal));
  });

  describe("and an event-list request fails", () => {
    beforeEach(() => { vi.mocked(loadModerationEvents).mockRejectedValueOnce(new Error("List unavailable")); });
    it("should retry the list without checking access again", async () => {
      renderPage();
      await user.click(await screen.findByRole("button", { name: "Retry" }));
      await screen.findByRole("button", { name: "Moon concert" });
      expect(loadEventProfile).toHaveBeenCalledTimes(1);
    });
  });

  it("should preserve a draft after a same-wallet storage update", async () => {
    view = renderPage();
    await user.click(await screen.findByRole("button", { name: "Moon concert" }));
    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Reject" }));
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
    await waitFor(() => expect(loadOwnedEvents).toHaveBeenLastCalledWith(auth.identity, expect.objectContaining({ offset: 0 }), expect.any(AbortSignal)));
    expect(vi.mocked(loadOwnedEvents).mock.calls.filter(([account]) => account.signer === auth.identity?.signer)).toHaveLength(1);
  });

  describe("and the account may only approve its own events", () => {
    beforeEach(() => { profile.permissions = ["approve_own_event"]; });
    it("should deny the all-owner pending view without loading a misleading catalogue", async () => {
      renderPage();
      await screen.findByRole("alert");
      expect(loadModerationEvents).not.toHaveBeenCalled();
      expect(screen.queryByRole("link", { name: "Moderation" })).toBeNull();
    });
    it("should provide own-event approval from My Hangouts", async () => {
      renderPage("mine");
      await user.click(await screen.findByRole("button", { name: "Approve Moon concert" }));
      await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Approve event" }));
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
      await screen.findByText("Delegated permissions saved. Moderator accounts always retain every permission.");
      expect(saveEventPermissions).toHaveBeenCalledWith(identity, identity.signer, ["approve_any_event"]);
    });

    describe("and the access refresh is slow", () => {
      beforeEach(() => {
        vi.mocked(loadEventProfile).mockResolvedValueOnce(profile).mockImplementation(() => new Promise(() => {}));
      });
      it("should retain the users view and show feedback immediately after saving", async () => {
        renderPage("users");
        await user.click(await screen.findByRole("button", { name: "Add permissions for a wallet" }));
        await user.type(screen.getByRole("textbox", { name: "Wallet address" }), identity.signer);
        await user.click(screen.getByRole("button", { name: "Save permissions" }));
        await screen.findByText("Delegated permissions saved. Moderator accounts always retain every permission.");
        expect(screen.getByRole("searchbox")).toBeTruthy();
        expect(screen.queryByText("Checking access…")).toBeNull();
      });
    });

    describe("and a moderator's effective permissions cannot be removed", () => {
      beforeEach(() => { vi.mocked(saveEventPermissions).mockResolvedValue(profile); });
      it("should explain fixed moderator authority instead of claiming the requested change took effect", async () => {
        renderPage("users");
        await user.click(await screen.findByRole("button", { name: "Add permissions for a wallet" }));
        await user.type(screen.getByRole("textbox", { name: "Wallet address" }), identity.signer);
        expect(screen.getByText(/This form updates delegated grants/)).toBeTruthy();
        await user.click(screen.getByRole("button", { name: "Save permissions" }));
        await screen.findByText("The server kept the account's effective permissions. Moderator accounts always retain every permission.");
      });
    });
  });
});
