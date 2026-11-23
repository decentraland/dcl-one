// @vitest-environment jsdom
import { cleanup, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { AuthIdentity } from "@/data/lib/auth/types";
import { CatalystError, postJSON, signedGetJSON } from "@/data/lib/catalyst/client";
import { eventsPageKeys } from "@/data/lib/catalyst/places/events-page";
import { openSignIn } from "../../components/auth/signin-store";
import { CREATOR, currentPath, fakeEventsApi, makeEvent, makeIdentity, MODERATOR, readsOf, renderAdminPage, type FakeEventsApi } from "./admin.test-utils";
import PendingEventsPage from "./PendingEventsPage";

const auth = vi.hoisted(() => ({ identity: null as AuthIdentity | null, isConnected: false }));
vi.hoisted(() => { Object.assign(window, { __vite_plugin_react_preamble_installed__: true }); });
vi.mock("@/data/lib/auth/context", () => ({ useAuth: () => auth }));
vi.mock("@/data/lib/auth/session", () => ({ getIdentity: () => auth.identity }));
vi.mock("../../components/auth/signin-store", () => ({ openSignIn: vi.fn() }));
vi.mock("../../components/chrome/SitesLayout", () => ({ default: ({ children }: { children: React.ReactNode }) => <>{children}</> }));
vi.mock("@/data/lib/catalyst/client", async (original) => ({
  ...await original<typeof import("@/data/lib/catalyst/client")>(),
  postJSON: vi.fn(), signedGetJSON: vi.fn(),
}));

const PATH = "/events/admin/pending-events";
const SETTINGS = "/events/api/profiles/me/settings";
const EVENTS = "/events/api/events";
const moderation = (id: string, body: unknown) => ["/events/api/events/" + id, body, expect.objectContaining({ method: "PATCH" })];

describe("when opening the Pending Events page", () => {
  let identity: AuthIdentity;
  let api: FakeEventsApi;
  let user: ReturnType<typeof userEvent.setup>;

  beforeEach(() => {
    identity = makeIdentity();
    auth.identity = identity;
    auth.isConnected = true;
    api = {
      // A reviewer: another creator's event takes both grants to approve or reject.
      profile: { user: identity.signer, permissions: ["approve_any_event", "edit_any_event"] },
      profiles: "forbidden",
      events: [makeEvent(), makeEvent({ id: "approved-one", name: "Sun party", approved: true })],
      failNext: {},
    };
    fakeEventsApi(api);
    user = userEvent.setup();
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

    it("should send the visitor to sign in without reading the queue", async () => {
      renderAdminPage(<PendingEventsPage />, PATH);
      expect(await screen.findByRole("button", { name: "Sign in" })).toBeTruthy();
      await waitFor(() => expect(openSignIn).toHaveBeenCalledWith({ redirectTo: PATH }));
      expect(signedGetJSON).not.toHaveBeenCalled();
    });
  });

  describe("and a pending event has a cover uploaded to the page's own http origin", () => {
    const poster = "/events-assets-099ac00/poster/abc.png";

    beforeEach(() => {
      expect(window.location.protocol).toBe("http:");
      api.events = [makeEvent({ image: `${window.location.origin}${poster}` })];
    });

    it("should show the cover on its card, and pass the review the same path", async () => {
      renderAdminPage(<PendingEventsPage />, PATH);
      const pending = await screen.findByRole("region", { name: "Pending Events" });
      const card = (await within(pending).findByRole("link", { name: "Moon concert" })).closest("li")!;
      expect(card.querySelector("img")?.getAttribute("src")).toBe(poster);
      await user.click(within(pending).getByRole("link", { name: "Moon concert" }));
      const review = await screen.findByRole("dialog", { name: "Moon concert" });
      expect(review.querySelector("img")?.getAttribute("src")).toBe(poster);
    });
  });

  describe("and the account has no event permission", () => {
    beforeEach(() => { api.profile.permissions = []; });

    it("should redirect to /events without reading the queue", async () => {
      renderAdminPage(<PendingEventsPage />, PATH);
      await waitFor(() => expect(currentPath()).toBe("/events"));
      expect(readsOf(EVENTS)).toBe(0);
    });
  });

  describe("and the account may only approve its own events", () => {
    beforeEach(() => {
      api.profile.permissions = ["approve_own_event"];
      api.events = [
        makeEvent({ id: "own-one", name: "My own show", user: MODERATOR }),
        makeEvent({ id: "other-one", name: "Someone else's show", user: CREATOR }),
      ];
    });

    it("should list only its own pending event", async () => {
      renderAdminPage(<PendingEventsPage />, PATH);
      const pending = await screen.findByRole("region", { name: "Pending Events" });
      expect(await within(pending).findByRole("link", { name: "My own show" })).toBeTruthy();
      expect(within(pending).queryByRole("link", { name: "Someone else's show" })).toBeNull();
    });

    it("should approve its own event and offer no rejection", async () => {
      renderAdminPage(<PendingEventsPage />, `${PATH}?id=own-one`);
      const review = await screen.findByRole("dialog", { name: "My own show" });
      expect(within(review).queryByRole("button", { name: "Reject" })).toBeNull();
      await user.click(within(review).getByRole("button", { name: "Approve" }));
      expect(await screen.findByText("Event/Experience approved successfully")).toBeTruthy();
      expect(postJSON).toHaveBeenCalledWith(...moderation("own-one", { approved: true }));
    });

    it("should list only its own recently approved event", async () => {
      api.events = [
        makeEvent({ id: "own-approved", name: "My approved show", user: MODERATOR, approved: true }),
        makeEvent({ id: "other-approved", name: "Someone else's approved show", user: CREATOR, approved: true }),
      ];
      renderAdminPage(<PendingEventsPage />, PATH);
      const recent = await screen.findByRole("region", { name: "Recently Approved (Last 24hs)" });
      expect(await within(recent).findByRole("link", { name: "My approved show" })).toBeTruthy();
      expect(within(recent).queryByRole("link", { name: "Someone else's approved show" })).toBeNull();
    });

    it("should open no review for someone else's approved event named by a link", async () => {
      api.events[1] = { ...api.events[1]!, approved: true };
      renderAdminPage(<PendingEventsPage />, `${PATH}?id=other-one`);
      await screen.findByRole("link", { name: "My own show" });
      expect(screen.queryByRole("dialog")).toBeNull();
    });
  });

  describe("and the account may approve any event but not edit any", () => {
    beforeEach(() => {
      api.profile.permissions = ["approve_any_event"];
      api.events = [
        makeEvent({ id: "own-one", name: "My own show", user: MODERATOR }),
        makeEvent({ id: "other-one", name: "Someone else's show", user: CREATOR }),
      ];
    });

    it("should offer no moderation or edit for someone else's event, which the API refuses", async () => {
      renderAdminPage(<PendingEventsPage />, `${PATH}?id=other-one`);
      const review = await screen.findByRole("dialog", { name: "Someone else's show" });
      expect(within(review).queryByRole("button", { name: "Approve" })).toBeNull();
      expect(within(review).queryByRole("button", { name: "Reject" })).toBeNull();
      expect(within(review).queryByRole("button", { name: "Edit Event" })).toBeNull();
    });

    it("should approve its own event", async () => {
      renderAdminPage(<PendingEventsPage />, `${PATH}?id=own-one`);
      const review = await screen.findByRole("dialog", { name: "My own show" });
      await user.click(within(review).getByRole("button", { name: "Approve" }));
      expect(await screen.findByText("Event/Experience approved successfully")).toBeTruthy();
      expect(postJSON).toHaveBeenCalledWith(...moderation("own-one", { approved: true }));
    });

    it("should reject its own event with a reason", async () => {
      renderAdminPage(<PendingEventsPage />, `${PATH}?id=own-one`);
      await user.click(within(await screen.findByRole("dialog", { name: "My own show" })).getByRole("button", { name: "Reject" }));
      const reject = screen.getByRole("dialog", { name: "Reject Reason*" });
      await user.click(within(reject).getByRole("checkbox", { name: /Invalid image/ }));
      await user.click(within(reject).getByRole("button", { name: "Submit" }));
      expect(await screen.findByText("Event/Experience rejected successfully")).toBeTruthy();
      expect(postJSON).toHaveBeenCalledWith(...moderation("own-one", { rejected: true, rejection_reason: "Invalid image" }));
    });
  });

  describe("and the permission read is refused", () => {
    beforeEach(() => { api.failNext.profile = new CatalystError("Forbidden", SETTINGS, 403); });

    it("should redirect to /events", async () => {
      renderAdminPage(<PendingEventsPage />, PATH);
      await waitFor(() => expect(currentPath()).toBe("/events"));
    });
  });

  describe("and the permission read fails", () => {
    beforeEach(() => { api.failNext.profile = new Error("Settings unavailable"); });

    it("should offer a retry instead of redirecting, and load the page once it succeeds", async () => {
      renderAdminPage(<PendingEventsPage />, PATH);
      const alert = await screen.findByRole("alert");
      expect(alert.textContent).toBe("Settings unavailable");
      expect(alert.dataset.tone).toBe("error");
      expect(currentPath()).toBe(PATH);
      await user.click(screen.getByRole("button", { name: "Retry" }));
      expect(await screen.findByRole("link", { name: "Moon concert" })).toBeTruthy();
    });
  });

  it("should show the admin bar with Pending Events current and no Users tab for a non-moderator list", async () => {
    renderAdminPage(<PendingEventsPage />, PATH);
    const bar = await screen.findByRole("navigation", { name: "Event administration" });
    expect(within(bar).getByRole("link", { name: "Pending Events" }).getAttribute("aria-current")).toBe("page");
    expect(within(bar).getByRole("link", { name: "Events" }).getAttribute("href")).toBe("/events");
    expect(within(bar).queryByRole("link", { name: "Users" })).toBeNull();
    expect(within(bar).getByRole("link", { name: "+ Create Event" }).getAttribute("href")).toBe("/events/new-event");
  });

  it("should show a spinner while the page loads, then the queue in its place", async () => {
    renderAdminPage(<PendingEventsPage />, PATH);
    expect(screen.getByRole("progressbar", { name: "Loading…" }).closest("[role=status]")).toBeTruthy();
    expect(await screen.findByRole("link", { name: "Moon concert" })).toBeTruthy();
    expect(screen.queryByRole("progressbar")).toBeNull();
  });

  it("should list the pending events and the recently approved ones with their status", async () => {
    renderAdminPage(<PendingEventsPage />, PATH);
    const pending = await screen.findByRole("region", { name: "Pending Events" });
    expect(await within(pending).findByRole("link", { name: "Moon concert" })).toBeTruthy();
    expect(within(pending).getByText("Pending")).toBeTruthy();
    expect(within(pending).getByText("Luna")).toBeTruthy();
    const recent = screen.getByRole("region", { name: "Recently Approved (Last 24hs)" });
    expect(within(recent).getByRole("link", { name: "Sun party" })).toBeTruthy();
    expect(within(recent).getByText("Approved")).toBeTruthy();
  });

  describe("and nothing waits for approval", () => {
    beforeEach(() => { api.events = []; });

    it("should say so", async () => {
      renderAdminPage(<PendingEventsPage />, PATH);
      expect(await screen.findByText("No events waiting for approval")).toBeTruthy();
    });
  });

  describe("and the queue fails to load", () => {
    beforeEach(() => { api.failNext.events = new Error("Queue unavailable"); });

    it("should show the failure and reload the queue on retry", async () => {
      renderAdminPage(<PendingEventsPage />, PATH);
      const alert = await screen.findByRole("alert");
      expect(alert.textContent).toBe("Queue unavailable");
      expect(alert.dataset.tone).toBe("error");
      await user.click(screen.getByRole("button", { name: "Retry" }));
      expect(await screen.findByRole("link", { name: "Moon concert" })).toBeTruthy();
    });
  });

  it("should open the review from a card and approve the event", async () => {
    const { client } = renderAdminPage(<PendingEventsPage />, PATH);
    client.setQueryData(eventsPageKeys.detail("pending-one"), makeEvent());
    await user.click(await screen.findByRole("link", { name: "Moon concert" }));
    expect(currentPath()).toBe(`${PATH}?id=pending-one`);
    const review = screen.getByRole("dialog", { name: "Moon concert" });
    expect(within(review).getByText("A concert")).toBeTruthy();
    await user.click(within(review).getByRole("button", { name: "Approve" }));
    expect(await screen.findByText("Event/Experience approved successfully")).toBeTruthy();
    expect(postJSON).toHaveBeenCalledWith(...moderation("pending-one", { approved: true }));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(client.getQueryState(eventsPageKeys.detail("pending-one"))?.isInvalidated).toBe(true);
    await waitFor(() => expect(readsOf(EVENTS)).toBe(2));
  });

  it("should open the review a deep link names", async () => {
    renderAdminPage(<PendingEventsPage />, `${PATH}?id=pending-one`);
    expect(await screen.findByRole("dialog", { name: "Moon concert" })).toBeTruthy();
  });

  it("should offer no moderation for an event already approved", async () => {
    renderAdminPage(<PendingEventsPage />, PATH);
    await user.click(await screen.findByRole("link", { name: "Sun party" }));
    const review = screen.getByRole("dialog", { name: "Sun party" });
    expect(within(review).queryByRole("button", { name: "Approve" })).toBeNull();
    expect(within(review).queryByRole("button", { name: "Reject" })).toBeNull();
  });

  describe("and the account may edit any event", () => {
    beforeEach(() => { api.profile.permissions = ["edit_any_event"]; });

    it("should edit someone else's event from the review but not approve or reject it", async () => {
      renderAdminPage(<PendingEventsPage />, PATH);
      await user.click(await screen.findByRole("link", { name: "Moon concert" }));
      const review = screen.getByRole("dialog", { name: "Moon concert" });
      expect(within(review).queryByRole("button", { name: "Approve" })).toBeNull();
      expect(within(review).queryByRole("button", { name: "Reject" })).toBeNull();
      await user.click(within(review).getByRole("button", { name: "Edit Event" }));
      expect(currentPath()).toBe("/events/edit-event/pending-one");
    });
  });

  describe("and the approval fails", () => {
    beforeEach(() => { api.failNext.moderate = new Error("The hangout changed. Refresh."); });

    it("should keep the review and the card, report the failure and allow another try", async () => {
      renderAdminPage(<PendingEventsPage />, PATH);
      await user.click(await screen.findByRole("link", { name: "Moon concert" }));
      const review = screen.getByRole("dialog", { name: "Moon concert" });
      await user.click(within(review).getByRole("button", { name: "Approve" }));
      expect((await within(review).findByRole("alert")).textContent).toBe("The hangout changed. Refresh.");
      expect(screen.queryByText("Event/Experience approved successfully")).toBeNull();
      expect(readsOf(EVENTS)).toBe(1);
      expect(within(screen.getByRole("region", { name: "Pending Events" })).getByRole("link", { name: "Moon concert" })).toBeTruthy();
      await user.click(within(review).getByRole("button", { name: "Approve" }));
      expect(await screen.findByText("Event/Experience approved successfully")).toBeTruthy();
    });
  });

  it("should require a reason before rejecting", async () => {
    renderAdminPage(<PendingEventsPage />, PATH);
    await user.click(await screen.findByRole("link", { name: "Moon concert" }));
    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Reject" }));
    const reject = screen.getByRole("dialog", { name: "Reject Reason*" });
    await user.click(within(reject).getByRole("button", { name: "Submit" }));
    expect(within(reject).getByRole("alert").textContent).toBe("Select at least one reason");
    expect(within(reject).getByRole("alert").dataset.tone).toBe("error");
    expect(postJSON).not.toHaveBeenCalled();
  });

  it("should reject with the chosen reasons and the note", async () => {
    renderAdminPage(<PendingEventsPage />, PATH);
    await user.click(await screen.findByRole("link", { name: "Moon concert" }));
    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Reject" }));
    const reject = screen.getByRole("dialog", { name: "Reject Reason*" });
    await user.click(within(reject).getByRole("checkbox", { name: /Invalid image/ }));
    await user.click(within(reject).getByRole("checkbox", { name: /Invalid location/ }));
    await user.type(within(reject).getByRole("textbox", { name: "Other (optional)" }), "Wrong parcel");
    await user.click(within(reject).getByRole("button", { name: "Submit" }));
    expect(await screen.findByText("Event/Experience rejected successfully")).toBeTruthy();
    expect(postJSON).toHaveBeenCalledWith(...moderation("pending-one", { rejected: true, rejection_reason: "Invalid image, Invalid location. Wrong parcel" }));
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("should return to the review when the rejection is cancelled", async () => {
    renderAdminPage(<PendingEventsPage />, PATH);
    await user.click(await screen.findByRole("link", { name: "Moon concert" }));
    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Reject" }));
    await user.click(within(screen.getByRole("dialog", { name: "Reject Reason*" })).getByRole("button", { name: "Cancel" }));
    expect(screen.getByRole("dialog", { name: "Moon concert" })).toBeTruthy();
    expect(postJSON).not.toHaveBeenCalled();
  });

  describe("and the rejection fails", () => {
    beforeEach(() => { api.failNext.moderate = new Error("Rejection not applied"); });

    it("should keep the reason dialog open with the failure", async () => {
      renderAdminPage(<PendingEventsPage />, PATH);
      await user.click(await screen.findByRole("link", { name: "Moon concert" }));
      await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Reject" }));
      const reject = screen.getByRole("dialog", { name: "Reject Reason*" });
      await user.click(within(reject).getByRole("checkbox", { name: /Invalid image/ }));
      await user.click(within(reject).getByRole("button", { name: "Submit" }));
      expect((await within(reject).findByRole("alert")).textContent).toBe("Rejection not applied");
      expect(within(reject).getByRole<HTMLInputElement>("checkbox", { name: /Invalid image/ }).checked).toBe(true);
      expect(screen.queryByText("Event/Experience rejected successfully")).toBeNull();
    });
  });

  describe("and the signed-in wallet changed in another tab", () => {
    it("should ask to sign in instead of signing with the other wallet", async () => {
      renderAdminPage(<PendingEventsPage />, PATH);
      await user.click(await screen.findByRole("link", { name: "Moon concert" }));
      auth.identity = makeIdentity("0x3333333333333333333333333333333333333333");
      await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Approve" }));
      expect(openSignIn).toHaveBeenCalled();
      expect(postJSON).not.toHaveBeenCalled();
    });
  });

  // The Sites client renders in StrictMode, which runs each effect's cleanup and setup once more after
  // mounting; an action's result must still close the dialog, report and clear the busy state.
  describe("in StrictMode, as the Sites client renders", () => {
    it("should close the review and report an approval", async () => {
      renderAdminPage(<PendingEventsPage />, PATH, { strict: true });
      await user.click(await screen.findByRole("link", { name: "Moon concert" }));
      await user.click(within(screen.getByRole("dialog", { name: "Moon concert" })).getByRole("button", { name: "Approve" }));
      expect(await screen.findByText("Event/Experience approved successfully")).toBeTruthy();
      expect(screen.queryByRole("dialog")).toBeNull();
      expect(currentPath()).toBe(PATH);
    });

    it("should close the reason dialog and report a rejection", async () => {
      renderAdminPage(<PendingEventsPage />, PATH, { strict: true });
      await user.click(await screen.findByRole("link", { name: "Moon concert" }));
      await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Reject" }));
      const reject = screen.getByRole("dialog", { name: "Reject Reason*" });
      await user.click(within(reject).getByRole("checkbox", { name: /Invalid image/ }));
      await user.click(within(reject).getByRole("button", { name: "Submit" }));
      expect(await screen.findByText("Event/Experience rejected successfully")).toBeTruthy();
      expect(screen.queryByRole("dialog")).toBeNull();
    });

    it("should report a failed approval and let it be tried again", async () => {
      api.failNext.moderate = new Error("The hangout changed. Refresh.");
      renderAdminPage(<PendingEventsPage />, PATH, { strict: true });
      await user.click(await screen.findByRole("link", { name: "Moon concert" }));
      const review = screen.getByRole("dialog", { name: "Moon concert" });
      await user.click(within(review).getByRole("button", { name: "Approve" }));
      expect((await within(review).findByRole("alert")).textContent).toBe("The hangout changed. Refresh.");
      const approve = within(review).getByRole<HTMLButtonElement>("button", { name: "Approve" });
      expect(approve.disabled).toBe(false);
      await user.click(approve);
      expect(await screen.findByText("Event/Experience approved successfully")).toBeTruthy();
    });
  });
});
