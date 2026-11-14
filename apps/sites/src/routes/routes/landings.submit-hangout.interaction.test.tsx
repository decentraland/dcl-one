// @vitest-environment jsdom
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AuthIdentity } from "@/data/lib/auth/types";
import { useAuth } from "@/data/lib/auth/context";
import { getIdentity } from "@/data/lib/auth/session";
import * as authoring from "@/data/lib/catalyst/landings/submit-hangout";
import { loadHangoutOptions, type HangoutOptions } from "@/data/lib/catalyst/landings/hangout-options";
import SubmitHangoutRoute from "./landings.submit-hangout";

vi.hoisted(() => { Object.assign(window, { __vite_plugin_react_preamble_installed__: true }); });

vi.mock("@/data/lib/auth/context", () => ({ useAuth: vi.fn() }));
vi.mock("@/data/lib/auth/session", () => ({ getIdentity: vi.fn() }));

vi.mock("@/data/lib/catalyst/landings/hangout-options", async (original) => ({ ...await original<typeof import("@/data/lib/catalyst/landings/hangout-options")>(), loadHangoutOptions: vi.fn() }));

describe("when editing a hangout through its route", () => {
  let options: HangoutOptions;
  let identity: AuthIdentity;
  let event: authoring.EditableHangout;
  let router: ReturnType<typeof createMemoryRouter>;
  let resolveLoad: (event: authoring.EditableHangout) => void;
  let rejectLoad: (error: Error) => void;
  let resolveOptions: (options: HangoutOptions) => void;
  let user: ReturnType<typeof userEvent.setup>;

  beforeEach(() => {
    identity = { signer: "0x1111111111111111111111111111111111111111", ephemeral: { address: "0x2222222222222222222222222222222222222222", privateKey: "0x01" }, expiration: "2099-01-01T00:00:00Z", authChain: [] };
    event = {
        id: "existing-event", name: "Monthly series", image: "https://example.com/cover.png", image_vertical: null,
        description: "Series", start_at: "2099-06-01T18:00:00Z", finish_at: "2099-06-01T20:00:00Z", next_start_at: null, next_finish_at: null,
        duration: 7_200_000, all_day: false, x: 0, y: 0, server: "music.dcl.eth", url: null, user: identity.signer, user_name: null,
        estate_id: null, estate_name: null, scene_name: null, approved: true, rejected: false, highlighted: false, trending: false, world: true,
        recurrent: true, recurrent_frequency: "MONTHLY", recurrent_weekday_mask: 2, recurrent_month_mask: 4, recurrent_interval: 2,
        recurrent_setpos: -1, recurrent_monthday: null, recurrent_count: 8, recurrent_until: null, recurrent_dates: [], categories: ["music", "social"], schedules: [],
        total_attendees: 0, latest_attendees: [], coordinates: [0, 0], position: [0, 0], live: false, attending: false, place_id: null, community_id: "community",
        featured_item: null, created_at: null, updated_at: null, approved_by: null, rejected_by: null, rejection_reason: null,
        deleted_by_user: false, deleted_by_admin: false, deleted_by: null, deleted_at: null, deleted_reason: null, previous_place_id: null,
      };
    options = { profile: null, profileError: null, communities: [], communitiesError: null, schedules: [], schedulesError: null, worlds: [], worldsError: null };
    vi.mocked(loadHangoutOptions).mockImplementationOnce(() => new Promise((resolve) => { resolveOptions = resolve; }));
    user = userEvent.setup();
    vi.mocked(useAuth).mockReturnValue({ identity, address: identity.signer, isConnected: true } as ReturnType<typeof useAuth>);
    vi.mocked(getIdentity).mockReturnValue(identity);
    vi.spyOn(authoring, "loadEditableHangout").mockImplementationOnce(() => new Promise((resolve, reject) => { resolveLoad = resolve; rejectLoad = reject; }));
    vi.spyOn(authoring, "deleteHangout").mockResolvedValue(undefined);
    vi.spyOn(window, "confirm").mockReturnValue(true);
    const loaderData = { sid: "test", mode: "edit", eventId: "existing-event", communityId: "", categories: [], categoriesError: false, assignment: { variant: "wizard" } };
    router = createMemoryRouter([
      { id: "hangout", path: "/landings/submit-hangout", Component: SubmitHangoutRoute as never, loader: () => loaderData },
      { path: "/events", element: <h1>What's On</h1> },
    ], { initialEntries: ["/landings/submit-hangout?mode=edit&id=existing-event&step=details"], hydrationData: { loaderData: { hangout: loaderData } } });
    render(<RouterProvider router={router} />);
  });
  afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.resetAllMocks(); router.dispose(); });

  it("should keep loading inside the page before the footer", () => {
    expect(screen.getByRole("status").textContent).toContain("Loading your hangout");
    expect(screen.getByRole("status").compareDocumentPosition(screen.getByRole("contentinfo")) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });
  it("should keep a load failure within the page chrome", async () => {
    await act(async () => rejectLoad(new Error("You cannot edit this event")));
    expect(screen.getByRole("alert").textContent).toContain("You cannot edit this event");
    expect(screen.getByRole("alert").compareDocumentPosition(screen.getByRole("contentinfo")) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });
  describe("and the event read fails transiently", () => {
    beforeEach(async () => {
      await act(async () => rejectLoad(new Error("Event service unavailable")));
      vi.mocked(authoring.loadEditableHangout).mockResolvedValueOnce(event);
      vi.mocked(loadHangoutOptions).mockResolvedValueOnce(options);
    });
    it("should retry both the event read and access lookup", async () => {
      await user.click(screen.getByRole("button", { name: "Check access again" }));
      await screen.findByRole("heading", { name: "Edit Hangout" });
      expect(authoring.loadEditableHangout).toHaveBeenCalledTimes(2);
      expect(loadHangoutOptions).toHaveBeenCalledTimes(2);
      expect(screen.queryByText("Event service unavailable")).toBeNull();
    });
  });
  describe("and another creator's event loads before permissions", () => {
    beforeEach(async () => {
      event.user = "0x3333333333333333333333333333333333333333";
      event.contact = "owner@example.test";
      event.details = "Owner private notes";
      await act(async () => resolveLoad(event));
    });
    it("should wait for access without exposing an editor or delete action", () => {
      expect(screen.getByRole("status").textContent).toBe("Loading your hangout…");
      expect(screen.queryByRole("button", { name: "Delete hangout" })).toBeNull();
      expect(screen.queryByRole("heading", { name: "Edit Hangout" })).toBeNull();
    });
    describe("and the account lacks edit permission", () => {
      beforeEach(async () => { await act(async () => resolveOptions(options)); });
      it("should deny access and keep delete hidden", () => {
        expect(screen.getByRole("alert").textContent).toBe("You don't have permission to edit this hangout.");
        expect(screen.queryByRole("button", { name: "Delete hangout" })).toBeNull();
      });
    });
    describe("and the account may edit any event", () => {
      beforeEach(async () => {
        options.profile = { user: identity.signer, permissions: ["edit_any_event"] };
        await act(async () => resolveOptions(options));
      });
      it("should show delegated controls while keeping owner fields private", () => {
        expect(screen.getByRole("textbox", { name: "Custom join link" })).toBeTruthy();
        expect(screen.getByRole("button", { name: "Delete hangout" })).toBeTruthy();
        expect(screen.queryByRole("textbox", { name: "Contact email" })).toBeNull();
        expect(screen.queryByRole("textbox", { name: "Details for moderators" })).toBeNull();
        expect((screen.getByRole("combobox", { name: "Community" }) as HTMLSelectElement).disabled).toBe(true);
      });
    });
  });
  describe("and the event has loaded", () => {
    beforeEach(async () => { await act(async () => { resolveOptions(options); resolveLoad(event); }); });
    it("should allow the owner to clear a community absent from the management list", async () => {
      await user.selectOptions(screen.getByRole("combobox", { name: "Community" }), "");
      expect((screen.getByRole("combobox", { name: "Community" }) as HTMLSelectElement).value).toBe("");
    });
    it("should show exactly one footer after the delete control", () => {
      expect(screen.getAllByRole("contentinfo")).toHaveLength(1);
      expect(screen.getByRole("button", { name: "Delete hangout" }).compareDocumentPosition(screen.getByRole("contentinfo")) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    });
    it("should delete the event through the authenticated data helper and navigate back", async () => {
      await user.click(screen.getByRole("button", { name: "Delete hangout" }));
      expect(authoring.deleteHangout).toHaveBeenCalledWith(identity, event.id);
      await waitFor(() => expect(router.state.location.pathname).toBe("/events"));
    });
    it("should keep the editor open and show a failed delete", async () => {
      vi.mocked(authoring.deleteHangout).mockRejectedValueOnce(new Error("Delete denied"));
      await user.click(screen.getByRole("button", { name: "Delete hangout" }));
      expect(screen.getByRole("alert").textContent).toBe("Delete denied");
      expect(router.state.location.pathname).toBe("/landings/submit-hangout");
    });
  });
});
