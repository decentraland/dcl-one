// @vitest-environment jsdom
import { cleanup, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Profiler, Suspense, use, type ReactNode } from "react";
import { useLocation } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { AuthIdentity } from "@/data/lib/auth/types";
import { CatalystError, postJSON, signedGetJSON } from "@/data/lib/catalyst/client";
import { eventsPageKeys } from "@/data/lib/catalyst/places/events-page";
import { eventAdminKeys } from "@/data/lib/catalyst/places/event-management";
import { QueryClient } from "@tanstack/react-query";
import { openSignIn } from "@/features/components/auth/signin-store";
import { CREATOR, currentPath, fakeEventsApi, makeEvent, makeIdentity, MODERATOR, readsOf, renderAdminPage, type FakeEventsApi } from "@/features/explorer/event-management/admin.test-utils";
import PendingEventsPage from "./PendingEventsPage";

const auth = vi.hoisted(() => ({ identity: null as AuthIdentity | null, isConnected: false }));
vi.hoisted(() => { Object.assign(window, { __vite_plugin_react_preamble_installed__: true }); });
vi.mock("@/data/lib/auth/context", () => ({ useAuth: () => auth }));
vi.mock("@/data/lib/auth/session", () => ({ getIdentity: () => auth.identity }));
vi.mock("@/features/components/auth/signin-store", () => ({ openSignIn: vi.fn() }));
vi.mock("@/features/components/chrome/SitesLayout", () => ({ default: ({ children }: { children: React.ReactNode }) => <>{children}</> }));
vi.mock("@/data/lib/catalyst/client", async (original) => ({
  ...await original<typeof import("@/data/lib/catalyst/client")>(),
  postJSON: vi.fn(), signedGetJSON: vi.fn(),
}));

const PATH = "/events/admin/pending-events";
const SETTINGS = "/events/api/profiles/me/settings";
const EVENTS = "/events/api/events";
const moderation = (id: string, body: unknown) => ["/events/api/events/" + id, body, expect.objectContaining({ method: "PATCH" })];

/**
 * The page as each commit leaves it: the names of the open dialogs and the toast. The router commits a
 * URL change in a transition of its own, after the page's state, so a frame between the two only shows
 * up when every commit is read, not when a test polls the settled DOM.
 */
function watchCommits(hold?: Promise<void>) {
  const frames: { dialogs: string[]; toast: string }[] = [];
  const record = () => frames.push({
    // The reason dialog is a native <dialog> (the shared Modal), the review the app's role="dialog" panel.
    dialogs: [...document.querySelectorAll("[role=dialog], dialog")].map((dialog) =>
      document.getElementById(dialog.getAttribute("aria-labelledby") ?? "")?.textContent ?? ""),
    toast: document.querySelector("[data-tone=success]")?.textContent ?? "",
  });
  const page = hold ? <Suspense fallback={null}><HoldBareUrl until={hold}><PendingEventsPage /></HoldBareUrl></Suspense> : <PendingEventsPage />;
  return { frames, page: <Profiler id="pending-events" onRender={record}>{page}</Profiler> };
}

/**
 * Holds the router's transition to the bare page URL open until `until` settles. It suspends above the
 * page, so the held transition never renders the page and leaves no trace in its state.
 */
function HoldBareUrl({ until, children }: { until: Promise<void>; children: ReactNode }) {
  if (!useLocation().search) use(until);
  return children;
}

// Records each dialog named `name` that React adds to the page from now on, including one that a
// later commit removes again: a synchronous query after the action sees only the final page.
function watchDialogMounts(name: string) {
  const mounted: string[] = [];
  const record = (records: MutationRecord[]) => {
    for (const { addedNodes } of records) {
      for (const node of addedNodes) {
        if (!(node instanceof Element)) continue;
        const dialogs = [node, ...node.querySelectorAll("*")].filter((el) => el.getAttribute("role") === "dialog" || el.tagName === "DIALOG");
        if (dialogs.some((dialog) => dialog.textContent?.includes(name))) mounted.push(name);
      }
    }
  };
  const observer = new MutationObserver(record);
  observer.observe(document.body, { childList: true, subtree: true });
  return () => {
    record(observer.takeRecords());
    observer.disconnect();
    return mounted;
  };
}

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

  it("should fade only the pending event's cover, keeping its text and status at full contrast", async () => {
    renderAdminPage(<PendingEventsPage />, PATH);
    const pending = await screen.findByRole("region", { name: "Pending Events" });
    const card = (await within(pending).findByRole("link", { name: "Moon concert" })).closest("li")!;
    const faded = card.querySelectorAll("[data-dimmed]");
    expect(faded).toHaveLength(1);
    expect(faded[0]!.contains(within(card).getByRole("link", { name: "Moon concert" }))).toBe(false);
    expect(faded[0]!.contains(within(card).getByText("Pending"))).toBe(false);
    const recent = screen.getByRole("region", { name: "Recently Approved (Last 24hs)" });
    expect(recent.querySelector("[data-dimmed]")).toBeNull();
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

  it("should open the same review again after a moderation closed it", async () => {
    renderAdminPage(<PendingEventsPage />, PATH);
    await user.click(await screen.findByRole("link", { name: "Moon concert" }));
    await user.click(within(screen.getByRole("dialog", { name: "Moon concert" })).getByRole("button", { name: "Approve" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    await waitFor(() => expect(currentPath()).toBe(PATH));
    // The fake queue still lists the event, as the API does until its next read settles.
    await user.click(screen.getByRole("link", { name: "Moon concert" }));
    expect(currentPath()).toBe(`${PATH}?id=pending-one`);
    expect(screen.getByRole("dialog", { name: "Moon concert" })).toBeTruthy();
  });

  it("should not call a just-moderated event gone while its id leaves the URL", async () => {
    renderAdminPage(<PendingEventsPage />, PATH);
    await user.click(await screen.findByRole("link", { name: "Moon concert" }));
    await user.click(within(screen.getByRole("dialog", { name: "Moon concert" })).getByRole("button", { name: "Approve" }));
    expect(await screen.findByText("Event/Experience approved successfully")).toBeTruthy();
    await waitFor(() => expect(currentPath()).toBe(PATH));
    expect(screen.queryByText("That event is no longer waiting for review.")).toBeNull();
  });

  it("should open the review a deep link names", async () => {
    renderAdminPage(<PendingEventsPage />, `${PATH}?id=pending-one`);
    expect(await screen.findByRole("dialog", { name: "Moon concert" })).toBeTruthy();
  });

  describe("and a deep link names an event the queue no longer holds", () => {
    beforeEach(() => { renderAdminPage(<PendingEventsPage />, `${PATH}?id=handled-elsewhere&from=email`); });

    it("should say so and drop the id from the URL, keeping the other parameters", async () => {
      expect(await screen.findByText("That event is no longer waiting for review.")).toBeTruthy();
      await waitFor(() => expect(currentPath()).toBe(`${PATH}?from=email`));
      expect(screen.queryByRole("dialog")).toBeNull();
    });
  });

  describe("and a deep link names an event newer than the cached queue", () => {
    let client: QueryClient;
    beforeEach(() => {
      client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
      // A queue read moments ago, before "Moon concert" was submitted, and still fresh.
      client.setQueryData(eventAdminKeys.queue(MODERATOR), { events: [], recentlyApproved: [], hasMore: false });
      renderAdminPage(<PendingEventsPage />, `${PATH}?id=pending-one`, { client });
    });

    it("should read the queue again and open the event instead of calling it gone", async () => {
      expect(await screen.findByRole("dialog", { name: "Moon concert" })).toBeTruthy();
      expect(currentPath()).toBe(`${PATH}?id=pending-one`);
      expect(screen.queryByText("That event is no longer waiting for review.")).toBeNull();
      expect(readsOf(EVENTS)).toBe(1);
    });
  });

  describe("and the read made for a deep link is cancelled before it settles", () => {
    beforeEach(async () => {
      const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
      client.setQueryData(eventAdminKeys.queue(MODERATOR), { events: [], recentlyApproved: [], hasMore: false });
      // The first queue read never answers, so it is still in flight when it is cancelled.
      const answer = vi.mocked(signedGetJSON).getMockImplementation()!;
      let held = false;
      vi.mocked(signedGetJSON).mockImplementation((path, ...rest) => {
        if (path === EVENTS && !held) { held = true; return new Promise(() => {}); }
        return answer(path, ...rest);
      });
      renderAdminPage(<PendingEventsPage />, `${PATH}?id=pending-one`, { client });
      await waitFor(() => expect(readsOf(EVENTS)).toBe(1));
      await client.cancelQueries({ queryKey: eventAdminKeys.queue(MODERATOR) });
    });

    it("should read the queue again and open the event", async () => {
      expect(await screen.findByRole("dialog", { name: "Moon concert" })).toBeTruthy();
      expect(currentPath()).toBe(`${PATH}?id=pending-one`);
      expect(readsOf(EVENTS)).toBe(2);
    });
  });

  describe("and a deep link names an event neither the cached nor the fresh queue holds", () => {
    beforeEach(() => {
      const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
      client.setQueryData(eventAdminKeys.queue(MODERATOR), { events: [], recentlyApproved: [], hasMore: false });
      renderAdminPage(<PendingEventsPage />, `${PATH}?id=handled-elsewhere`, { client });
    });

    it("should say so after the fresh read and drop the id", async () => {
      expect(await screen.findByText("That event is no longer waiting for review.")).toBeTruthy();
      await waitFor(() => expect(currentPath()).toBe(PATH));
      expect(readsOf(EVENTS)).toBe(1);
    });
  });

  describe("and a deep link names an event while the queue fails to load", () => {
    beforeEach(() => {
      api.failNext.events = new CatalystError("Queue unavailable", EVENTS, 503);
      renderAdminPage(<PendingEventsPage />, `${PATH}?id=pending-one`);
    });

    it("should keep the id for the retry instead of calling the event gone", async () => {
      expect(await screen.findByText("Queue unavailable")).toBeTruthy();
      expect(currentPath()).toBe(`${PATH}?id=pending-one`);
      expect(screen.queryByText("That event is no longer waiting for review.")).toBeNull();
    });
  });

  describe("and the account also manages users but the list of grants fails", () => {
    beforeEach(() => {
      api.profile.permissions = [...api.profile.permissions, "edit_any_profile"];
      api.profiles = [api.profile];
      api.failNext.profiles = new CatalystError("Grants unavailable", "/events/api/profiles/settings", 503);
    });

    it("should still list the pending events, without reading the grants", async () => {
      renderAdminPage(<PendingEventsPage />, PATH);
      expect(await screen.findByRole("link", { name: "Moon concert" })).toBeTruthy();
      expect(screen.queryByText("Grants unavailable")).toBeNull();
      expect(readsOf("/events/api/profiles/settings")).toBe(0);
    });
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
    const reviewMounts = watchDialogMounts("Moon concert");
    await user.click(within(reject).getByRole("button", { name: "Submit" }));
    expect(await screen.findByText("Event/Experience rejected successfully")).toBeTruthy();
    expect(postJSON).toHaveBeenCalledWith(...moderation("pending-one", { rejected: true, rejection_reason: "Invalid image, Invalid location. Wrong parcel" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    // The review the rejection replaced does not come back while the closed review leaves the URL.
    expect(reviewMounts()).toEqual([]);
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
      await waitFor(() => expect(currentPath()).toBe(PATH));
    });

    it("should close the reason dialog and report a rejection", async () => {
      renderAdminPage(<PendingEventsPage />, PATH, { strict: true });
      await user.click(await screen.findByRole("link", { name: "Moon concert" }));
      await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Reject" }));
      const reject = screen.getByRole("dialog", { name: "Reject Reason*" });
      await user.click(within(reject).getByRole("checkbox", { name: /Invalid image/ }));
      const reviewMounts = watchDialogMounts("Moon concert");
      await user.click(within(reject).getByRole("button", { name: "Submit" }));
      expect(await screen.findByText("Event/Experience rejected successfully")).toBeTruthy();
      expect(screen.queryByRole("dialog")).toBeNull();
      expect(reviewMounts()).toEqual([]);
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

  // The review's `?id` leaves the URL in the router's own transition, committed after the page's state.
  describe.each([
    { mode: "", strict: false },
    { mode: " in StrictMode", strict: true },
  ])("and a moderation closes the review$mode", ({ strict }) => {
    it("should never show a dialog beside the rejection's toast", async () => {
      const { frames, page } = watchCommits();
      renderAdminPage(page, PATH, { strict });
      await user.click(await screen.findByRole("link", { name: "Moon concert" }));
      await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Reject" }));
      const reject = screen.getByRole("dialog", { name: "Reject Reason*" });
      await user.click(within(reject).getByRole("checkbox", { name: /Invalid image/ }));
      const submitted = frames.length;
      await user.click(within(reject).getByRole("button", { name: "Submit" }));
      await waitFor(() => expect(currentPath()).toBe(PATH));
      expect(screen.getByText("Event/Experience rejected successfully")).toBeTruthy();
      const after = frames.slice(submitted);
      expect(after.some((frame) => frame.toast === "Event/Experience rejected successfully")).toBe(true);
      expect(after.filter((frame) => frame.toast && frame.dialogs.length)).toEqual([]);
      expect(screen.queryByRole("dialog")).toBeNull();
    });

    it("should never show the review beside the approval's toast", async () => {
      const { frames, page } = watchCommits();
      renderAdminPage(page, PATH, { strict });
      await user.click(await screen.findByRole("link", { name: "Moon concert" }));
      const approving = frames.length;
      await user.click(within(screen.getByRole("dialog", { name: "Moon concert" })).getByRole("button", { name: "Approve" }));
      await waitFor(() => expect(currentPath()).toBe(PATH));
      expect(screen.getByText("Event/Experience approved successfully")).toBeTruthy();
      const after = frames.slice(approving);
      expect(after.some((frame) => frame.toast === "Event/Experience approved successfully")).toBe(true);
      expect(after.filter((frame) => frame.toast && frame.dialogs.length)).toEqual([]);
      expect(screen.queryByRole("dialog")).toBeNull();
    });

    it("should keep the reason dialog through a failed rejection, with no toast", async () => {
      api.failNext.moderate = new Error("Rejection not applied");
      const { frames, page } = watchCommits();
      renderAdminPage(page, PATH, { strict });
      await user.click(await screen.findByRole("link", { name: "Moon concert" }));
      await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Reject" }));
      const reject = screen.getByRole("dialog", { name: "Reject Reason*" });
      await user.click(within(reject).getByRole("checkbox", { name: /Invalid image/ }));
      const submitted = frames.length;
      await user.click(within(reject).getByRole("button", { name: "Submit" }));
      expect((await within(reject).findByRole("alert")).textContent).toBe("Rejection not applied");
      expect(frames.length).toBeGreaterThan(submitted);
      expect(frames.slice(submitted).every((frame) => frame.dialogs.join() === "Reject Reason*" && !frame.toast)).toBe(true);
      expect(currentPath()).toBe(`${PATH}?id=pending-one`);
    });

    it("should keep the review through a failed approval, with no toast", async () => {
      api.failNext.moderate = new Error("The hangout changed. Refresh.");
      const { frames, page } = watchCommits();
      renderAdminPage(page, PATH, { strict });
      await user.click(await screen.findByRole("link", { name: "Moon concert" }));
      const review = screen.getByRole("dialog", { name: "Moon concert" });
      const approving = frames.length;
      await user.click(within(review).getByRole("button", { name: "Approve" }));
      expect((await within(review).findByRole("alert")).textContent).toBe("The hangout changed. Refresh.");
      expect(frames.length).toBeGreaterThan(approving);
      expect(frames.slice(approving).every((frame) => frame.dialogs.join() === "Moon concert" && !frame.toast)).toBe(true);
      expect(currentPath()).toBe(`${PATH}?id=pending-one`);
    });

    it("should keep the closed review closed when another card is opened before its URL change commits", async () => {
      api.events = [makeEvent(), makeEvent({ id: "pending-two", name: "Sun festival" })];
      // The closed review's `?id` stays in the URL while the next card is opened.
      let release!: () => void;
      const { frames, page } = watchCommits(new Promise<void>((resolve) => { release = resolve; }));
      renderAdminPage(page, `${PATH}?id=pending-one`, { strict });
      const review = await screen.findByRole("dialog", { name: "Moon concert" });
      await user.click(within(review).getByRole("button", { name: "Approve" }));
      expect(await screen.findByText("Event/Experience approved successfully")).toBeTruthy();
      expect(currentPath()).toBe(`${PATH}?id=pending-one`);
      expect(screen.queryByRole("dialog")).toBeNull();
      const opening = frames.length;
      await user.click(screen.getByRole("link", { name: "Sun festival" }));
      expect(await screen.findByRole("dialog", { name: "Sun festival" })).toBeTruthy();
      expect(currentPath()).toBe(`${PATH}?id=pending-two`);
      const after = frames.slice(opening);
      expect(after.length).toBeGreaterThan(0);
      expect(after.filter((frame) => frame.dialogs.includes("Moon concert"))).toEqual([]);
      release();
    });

    it("should open the review again from its card after it was closed", async () => {
      renderAdminPage(<PendingEventsPage />, PATH, { strict });
      await user.click(await screen.findByRole("link", { name: "Moon concert" }));
      expect(screen.getByRole("dialog", { name: "Moon concert" })).toBeTruthy();
      await user.keyboard("{Escape}");
      expect(screen.queryByRole("dialog")).toBeNull();
      await user.click(screen.getByRole("link", { name: "Moon concert" }));
      expect(await screen.findByRole("dialog", { name: "Moon concert" })).toBeTruthy();
      expect(currentPath()).toBe(`${PATH}?id=pending-one`);
    });
  });
});
