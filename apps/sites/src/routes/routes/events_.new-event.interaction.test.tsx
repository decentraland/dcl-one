// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AuthIdentity } from "@/data/lib/auth/types";
import { useAuth } from "@/data/lib/auth/context";
import { getIdentity } from "@/data/lib/auth/session";
import * as authoring from "@/data/lib/catalyst/landings/submit-hangout";
import { CatalystError } from "@/data/lib/catalyst/client";
import { eventsPageKeys } from "@/data/lib/catalyst/places/events-page";
import { loadHangoutOptions, type HangoutOptions } from "@/data/lib/catalyst/landings/hangout-options";
import { openSignIn } from "@/features/components/auth/signin-store";
import SubmitHangoutRoute from "./events_.new-event";

vi.hoisted(() => { Object.assign(window, { __vite_plugin_react_preamble_installed__: true }); });

vi.mock("@/data/lib/auth/context", () => ({ useAuth: vi.fn() }));
vi.mock("@/data/lib/auth/session", () => ({ getIdentity: vi.fn() }));
vi.mock("@/features/components/auth/signin-store", () => ({ openSignIn: vi.fn() }));

vi.mock("@/data/lib/catalyst/landings/hangout-options", async (original) => ({ ...await original<typeof import("@/data/lib/catalyst/landings/hangout-options")>(), loadHangoutOptions: vi.fn() }));

const EVENT_ID = "5f0c6a52-6b1d-4c1e-9a77-0e8f5b2c4d10";
const EDIT_PATH = `/events/edit-event/${EVENT_ID}`;
const CREATE_PATH = "/events/new-event";

describe("when editing a hangout through its route", () => {
  let options: HangoutOptions;
  let identity: AuthIdentity;
  let event: authoring.EditableHangout;
  let router: ReturnType<typeof createMemoryRouter>;
  let client: QueryClient;
  let resolveLoad: (event: authoring.EditableHangout) => void;
  let rejectLoad: (error: Error) => void;
  let resolveOptions: (options: HangoutOptions) => void;
  let user: ReturnType<typeof userEvent.setup>;

  let scrollTo: ReturnType<typeof vi.spyOn>;

  function mount(path = `/events/edit-event/${EVENT_ID}`, eventId: string | null = EVENT_ID) {
    const mode = path === CREATE_PATH ? "create" : "edit";
    const loaderData = { sid: "test", mode, eventId: mode === "create" ? null : eventId, communityId: "", categories: [], categoriesError: false, assignment: { variant: "wizard" } };
    router = createMemoryRouter([
      { id: "hangout", path: mode === "create" ? CREATE_PATH : "/events/edit-event/:id", Component: SubmitHangoutRoute as never, loader: () => loaderData },
      { path: "/events", element: <h1>What's On</h1> },
    ], { initialEntries: [path], hydrationData: { loaderData: { hangout: loaderData } } });
    render(<QueryClientProvider client={client}><RouterProvider router={router} /></QueryClientProvider>);
  }

  beforeEach(() => {
    identity = { signer: "0x1111111111111111111111111111111111111111", ephemeral: { address: "0x2222222222222222222222222222222222222222", privateKey: "0x01" }, expiration: "2099-01-01T00:00:00Z", authChain: [] };
    event = {
        id: EVENT_ID, name: "Monthly series", image: "https://example.com/cover.png", image_vertical: null,
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
    scrollTo = vi.spyOn(window, "scrollTo").mockImplementation(() => {});
    client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    client.setQueryData(eventsPageKeys.detail(event.id), event);
  });
  afterEach(() => { cleanup(); client.clear(); vi.restoreAllMocks(); vi.resetAllMocks(); router.dispose(); });

  describe("and the event is still loading", () => {
    beforeEach(() => mount());

    it("should show the editor header and a loading status before the footer", () => {
      expect(screen.getByRole("heading", { name: "Edit Event" })).toBeTruthy();
      expect(screen.getByRole("status").textContent).toBe("Loading your hangout…");
      expect(screen.getByRole("status").dataset.tone).toBe("info");
      expect(screen.getByRole("status").compareDocumentPosition(screen.getByRole("contentinfo")) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    });

    it("should not offer a delete action", () => {
      expect(screen.queryByRole("button", { name: "Delete" })).toBeNull();
    });
  });

  describe.each([400, 401, 403, 404])("and the event read answers %i", (status) => {
    beforeEach(async () => {
      mount();
      await act(async () => rejectLoad(new CatalystError("Event not found", `/events/api/events/${EVENT_ID}`, status)));
    });

    it("should replace the edit page with the Events page without showing the error", async () => {
      await waitFor(() => expect(router.state.location.pathname).toBe("/events"));
      expect(router.state.historyAction).toBe("REPLACE");
      expect(screen.queryByText("Event not found")).toBeNull();
    });
  });

  describe("and the event read fails with a server error", () => {
    beforeEach(async () => {
      mount();
      await act(async () => rejectLoad(new CatalystError("Event service unavailable", `/events/api/events/${EVENT_ID}`, 500)));
    });

    it("should stay on the edit page and show the error with a retry under the header", () => {
      expect(router.state.location.pathname).toBe(EDIT_PATH);
      expect(screen.getByRole("alert").textContent).toBe("Event service unavailable");
      expect(screen.getByRole("alert").dataset.tone).toBe("error");
      expect(screen.getByRole("heading", { name: "Edit Event" }).compareDocumentPosition(screen.getByRole("alert")) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
      expect(screen.getByRole("button", { name: "Retry" })).toBeTruthy();
    });

    describe("and the retry succeeds", () => {
      beforeEach(async () => {
        vi.mocked(authoring.loadEditableHangout).mockResolvedValueOnce(event);
        vi.mocked(loadHangoutOptions).mockResolvedValueOnce(options);
        await user.click(screen.getByRole("button", { name: "Retry" }));
      });

      it("should read the event and permissions again and open the editor", async () => {
        expect(await screen.findByRole("textbox", { name: "Event Name" })).toHaveProperty("value", "Monthly series");
        expect(authoring.loadEditableHangout).toHaveBeenCalledTimes(2);
        expect(loadHangoutOptions).toHaveBeenCalledTimes(2);
        expect(screen.queryByRole("alert")).toBeNull();
      });
    });
  });

  describe("and the event read times out without a status", () => {
    beforeEach(async () => {
      mount();
      await act(async () => rejectLoad(new Error("The request timed out")));
    });

    it("should offer a retry instead of leaving the page", () => {
      expect(router.state.location.pathname).toBe(EDIT_PATH);
      expect(screen.getByRole("button", { name: "Retry" })).toBeTruthy();
    });
  });

  describe("and the visitor is signed out", () => {
    beforeEach(() => {
      vi.mocked(useAuth).mockReturnValue({ identity: null, address: null, isConnected: false } as unknown as ReturnType<typeof useAuth>);
      vi.mocked(getIdentity).mockReturnValue(null);
      mount();
    });

    it("should ask for sign-in and return to this same editor afterwards", async () => {
      await waitFor(() => expect(openSignIn).toHaveBeenCalledWith({ redirectTo: EDIT_PATH }));
    });

    it("should stay on the edit page showing only the loading state, without reading the event", () => {
      expect(router.state.location.pathname).toBe(EDIT_PATH);
      expect(screen.getByRole("status").textContent).toBe("Loading your hangout…");
      expect(screen.queryByRole("button", { name: /sign in/i })).toBeNull();
      expect(authoring.loadEditableHangout).not.toHaveBeenCalled();
    });
  });

  describe("and a signed-out visitor opens the create page", () => {
    beforeEach(() => {
      vi.mocked(useAuth).mockReturnValue({ identity: null, address: null, isConnected: false } as unknown as ReturnType<typeof useAuth>);
      vi.mocked(getIdentity).mockReturnValue(null);
      mount(CREATE_PATH);
    });

    it("should send the visitor to sign-in and back to the create page", async () => {
      await waitFor(() => expect(openSignIn).toHaveBeenCalledWith({ redirectTo: CREATE_PATH }));
      expect(openSignIn).toHaveBeenCalledTimes(1);
    });

    it("should show only the loading state, not the form or a sign-in button", () => {
      expect(screen.getByRole("status").textContent).toBe("Loading your hangout…");
      expect(screen.queryByRole("button", { name: /sign in/i })).toBeNull();
      expect(screen.queryByRole("textbox", { name: "Event Name" })).toBeNull();
    });
  });

  describe("and a signed-in visitor opens the create page", () => {
    beforeEach(() => mount(CREATE_PATH));

    it("should not ask for sign-in", async () => {
      expect(await screen.findByRole("heading", { level: 1 })).toBeTruthy();
      expect(openSignIn).not.toHaveBeenCalled();
    });
  });

  describe("and the edit link names a malformed event id, which the loader reads as no event", () => {
    beforeEach(() => mount("/events/edit-event/not-an-id", null));

    it("should not read the event and replace the edit page with the Events page", async () => {
      await waitFor(() => expect(router.state.location.pathname).toBe("/events"));
      expect(router.state.historyAction).toBe("REPLACE");
      expect(authoring.loadEditableHangout).not.toHaveBeenCalled();
    });
  });

  describe("and another creator's event loads before permissions", () => {
    beforeEach(async () => {
      mount();
      event.user = "0x3333333333333333333333333333333333333333";
      event.contact = "owner@example.test";
      event.details = "Owner private notes";
      await act(async () => resolveLoad(event));
    });

    it("should wait for access without exposing an editor or delete action", () => {
      expect(screen.getByRole("status").textContent).toBe("Loading your hangout…");
      expect(screen.queryByRole("button", { name: "Delete" })).toBeNull();
      expect(screen.queryByRole("textbox", { name: "Event Name" })).toBeNull();
    });

    describe("and the account lacks edit permission", () => {
      beforeEach(async () => { await act(async () => resolveOptions(options)); });

      it("should replace the edit page with the Events page", async () => {
        await waitFor(() => expect(router.state.location.pathname).toBe("/events"));
        expect(router.state.historyAction).toBe("REPLACE");
      });
    });

    describe("and the account's permissions cannot be read", () => {
      beforeEach(async () => { await act(async () => resolveOptions({ ...options, profileError: "Profile service unavailable" })); });

      it("should stay on the edit page with a retry instead of denying access", () => {
        expect(router.state.location.pathname).toBe(EDIT_PATH);
        expect(screen.getByRole("alert").textContent).toBe("Profile service unavailable");
        expect(screen.queryByRole("textbox", { name: "Event Name" })).toBeNull();
      });

      describe("and the retry shows the account may edit any event", () => {
        beforeEach(async () => {
          vi.mocked(authoring.loadEditableHangout).mockResolvedValueOnce(event);
          vi.mocked(loadHangoutOptions).mockResolvedValueOnce({ ...options, profile: { user: identity.signer, permissions: ["edit_any_event"] } });
          await user.click(screen.getByRole("button", { name: "Retry" }));
        });

        it("should open the delegated editor", async () => {
          expect(await screen.findByRole("button", { name: "Delete" })).toBeTruthy();
          expect(router.state.location.pathname).toBe(EDIT_PATH);
        });
      });
    });

    describe("and the account may edit any event", () => {
      beforeEach(async () => {
        options.profile = { user: identity.signer, permissions: ["edit_any_event"] };
        await act(async () => resolveOptions(options));
      });

      it("should show delegated controls while keeping owner fields private", () => {
        expect(screen.getByRole("button", { name: "Delete" })).toBeTruthy();
        expect(screen.queryByRole("textbox", { name: "Custom join link" })).toBeNull();
        expect(screen.queryByRole("textbox", { name: "Contact email" })).toBeNull();
        expect(screen.queryByRole("textbox", { name: "Details for moderators" })).toBeNull();
        expect((screen.getByRole("combobox", { name: "Community" }) as HTMLSelectElement).disabled).toBe(true);
      });
    });
  });

  describe("and the owner's event has loaded", () => {
    beforeEach(async () => {
      mount();
      await act(async () => { resolveOptions(options); resolveLoad(event); });
    });

    it("should keep the prefilled cover, details, location and schedule in one editor", () => {
      expect(screen.getByRole("textbox", { name: "Event Name" })).toHaveProperty("value", "Monthly series");
      expect(screen.getByRole("img", { name: "Hangout cover" }).getAttribute("src")).toBe(event.image);
      expect(screen.getByLabelText("World")).toHaveProperty("value", "music.dcl.eth");
      expect(screen.getByLabelText("Date")).toHaveProperty("value", "2099-06-01");
    });

    it("should open the editor at the top of the page", () => {
      expect(scrollTo).toHaveBeenLastCalledWith(0, 0);
    });

    it("should retain the saved recurrence interval in edit preview", async () => {
      await user.click(screen.getByRole("button", { name: "Preview" }));
      expect(within(screen.getByRole("dialog")).getByText("Every 2 months")).toBeTruthy();
    });

    it("should allow the owner to clear a community absent from the management list", async () => {
      await user.selectOptions(screen.getByRole("combobox", { name: "Community" }), "");
      expect((screen.getByRole("combobox", { name: "Community" }) as HTMLSelectElement).value).toBe("");
    });

    it("should place Delete with the form actions, before Cancel and Save changes", () => {
      const del = screen.getByRole("button", { name: "Delete" });
      expect(del.parentElement).toBe(screen.getByRole("button", { name: "Save changes" }).parentElement);
      expect(del.compareDocumentPosition(screen.getByRole("link", { name: "Cancel" })) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
      expect(screen.getAllByRole("contentinfo")).toHaveLength(1);
    });

    describe("and the owner asks to delete it", () => {
      beforeEach(async () => { await user.click(screen.getByRole("button", { name: "Delete" })); });

      it("should ask for confirmation before deleting", () => {
        expect(screen.getByRole("dialog", { name: "Are you sure you want to delete your event?" })).toBeTruthy();
        expect(authoring.deleteHangout).not.toHaveBeenCalled();
      });

      it("should keep the event and the editor when the owner cancels", async () => {
        await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Cancel" }));
        expect(screen.queryByRole("dialog")).toBeNull();
        expect(authoring.deleteHangout).not.toHaveBeenCalled();
        expect(router.state.location.pathname).toBe(EDIT_PATH);
      });

      it("should delete through the authenticated helper and open My Events", async () => {
        await user.click(screen.getByRole("button", { name: "Delete event" }));
        expect(authoring.deleteHangout).toHaveBeenCalledWith(identity, event.id);
        await waitFor(() => expect(router.state.location.pathname).toBe("/events"));
        expect(router.state.location.search).toBe("?tab=my");
        expect(client.getQueryState(eventsPageKeys.detail(event.id))?.isInvalidated).toBe(true);
      });

      it("should keep the dialog busy and undismissable while the delete is in flight", async () => {
        let finish: () => void = () => {};
        vi.mocked(authoring.deleteHangout).mockImplementationOnce(() => new Promise<void>((resolve) => { finish = resolve; }));
        await user.click(screen.getByRole("button", { name: "Delete event" }));
        const dialog = within(screen.getByRole("dialog"));
        expect(dialog.getByRole<HTMLButtonElement>("button", { name: "Deleting…" }).disabled).toBe(true);
        expect(dialog.getByRole<HTMLButtonElement>("button", { name: "Cancel" }).disabled).toBe(true);
        await user.keyboard("{Escape}");
        expect(screen.getByRole("dialog")).toBeTruthy();
        await act(async () => finish());
      });

      it("should keep the editor open and report a failed delete in the dialog", async () => {
        vi.mocked(authoring.deleteHangout).mockRejectedValueOnce(new Error("Delete denied"));
        await user.click(screen.getByRole("button", { name: "Delete event" }));
        expect(within(screen.getByRole("dialog")).getByRole("alert").textContent).toBe("We couldn't delete your event. Please try again.");
        expect(within(screen.getByRole("dialog")).getByRole("alert").dataset.tone).toBe("error");
        expect(within(screen.getByRole("dialog")).getByRole<HTMLButtonElement>("button", { name: "Delete event" }).disabled).toBe(false);
        expect(router.state.location.pathname).toBe(EDIT_PATH);
        expect(client.getQueryState(eventsPageKeys.detail(event.id))?.isInvalidated).toBe(false);
      });
    });
  });

  describe("and the edit link names an anchor", () => {
    beforeEach(async () => {
      mount(`${EDIT_PATH}#schedule`);
      await act(async () => { resolveOptions(options); resolveLoad(event); });
    });

    it("should not reset the scroll position", () => {
      expect(screen.getByRole("textbox", { name: "Event Name" })).toBeTruthy();
      expect(scrollTo).not.toHaveBeenCalledWith(0, 0);
    });
  });
});
