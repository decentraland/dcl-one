// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from "vitest";

vi.hoisted(() =>
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshReg$: () => {},
    $RefreshSig$: () => (type: unknown) => type,
  }),
);

const auth = vi.hoisted(() => ({ identity: null as unknown }));
vi.mock("@/data/lib/auth/context", () => ({
  useAuth: () => ({ identity: auth.identity, isConnected: Boolean(auth.identity) }),
}));
const signIn = vi.hoisted(() => ({ openSignIn: vi.fn(), real: undefined as undefined | ((...args: never[]) => unknown) }));
vi.mock("../components/auth/signin-store", async (original) => {
  const real = await original<typeof import("../components/auth/signin-store")>();
  signIn.real = real.openSignIn as never;
  return { ...real, openSignIn: signIn.openSignIn };
});
vi.mock("@/core/lib/telemetry/track", () => ({ track: vi.fn() }));
const http = vi.hoisted(() => ({ signedGetJSON: vi.fn(), postJSON: vi.fn() }));
vi.mock("@/data/lib/catalyst/client", async (actual) => ({
  ...(await actual<typeof import("@/data/lib/catalyst/client")>()),
  signedGetJSON: http.signedGetJSON,
  postJSON: http.postJSON,
}));
vi.mock("@/data/lib/catalyst/places/events", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/places/events")>()),
  fetchEvents: vi.fn(),
  fetchEvent: vi.fn(),
}));
vi.mock("@/data/lib/catalyst/places/index", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/places/index")>()),
  fetchMostActivePlaces: vi.fn(),
}));
vi.mock("@/data/lib/catalyst/jump/place-lookup", () => ({ loadJumpPlace: vi.fn() }));
vi.mock("@/data/lib/catalyst/places/event-management", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/places/event-management")>()),
  loadAllOwnedEvents: vi.fn(),
}));

import type { AuthIdentity } from "@/data/lib/auth/types";
import { CatalystError } from "@/data/lib/catalyst/client";
import { track } from "@/core/lib/telemetry/track";
import { loadJumpPlace } from "@/data/lib/catalyst/jump/place-lookup";
import { fetchEvent, fetchEvents, type Event } from "@/data/lib/catalyst/places/events";
import { fetchMostActivePlaces } from "@/data/lib/catalyst/places/index";
import { loadAllOwnedEvents, type ManagedEvent } from "@/data/lib/catalyst/places/event-management";
import type { DetectedPlatform } from "../download/options";
import { columnsFor, EventsView } from "./EventsView";
import { eventRecord, placeRecord } from "./fixtures";

const DESKTOP: DetectedPlatform = { kind: "desktop", os: "macos", arch: "arm64" } as DetectedPlatform;
const HOUR = 3_600_000;

function soon(id: string, name: string, hours = 2): Event {
  const start = new Date(Date.now() + hours * HOUR).toISOString();
  const finish = new Date(Date.now() + (hours + 1) * HOUR).toISOString();
  return eventRecord({ id, name, start_at: start, next_start_at: start, finish_at: finish, next_finish_at: finish });
}

let calendar: Event[];
let upcoming: Event[];
let live: { events: Event[]; places: ReturnType<typeof placeRecord>[] };

function setup(path: string, platform: DetectedPlatform = DESKTOP, pageOrigin = "https://example.org") {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const router = createMemoryRouter(
    [
      {
        path: "/events",
        element: <EventsView sid="sid" platform={platform} origin="https://example.org" pageOrigin={pageOrigin} initial={null} />,
      },
    ],
    { initialEntries: [path] },
  );
  render(
    <QueryClientProvider client={client}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
  return { router, user: userEvent.setup(), client };
}

// Local midday keeps soon(-0.5) and soon(3) on today's column in any TZ; only Date is faked so RTL polling runs.
const NOW = new Date(2026, 10, 19, 12);

beforeEach(() => {
  vi.useFakeTimers({ toFake: ["Date"] });
  vi.setSystemTime(NOW);
  auth.identity = null;
  calendar = [soon("cal", "Poker Night")];
  upcoming = [soon("up", "Watch Party", 1)];
  live = { events: [], places: [] };
  vi.mocked(fetchEvents).mockImplementation(async (params) => {
    if (params?.list === "live") return { data: live.events, total: live.events.length };
    if (params?.list === "upcoming") return { data: upcoming, total: upcoming.length };
    return { data: calendar, total: calendar.length };
  });
  vi.mocked(fetchMostActivePlaces).mockImplementation(async () => live.places);
  vi.mocked(fetchEvent).mockImplementation(async (id) => {
    const found = [...calendar, ...upcoming].find((event) => event.id === id);
    if (!found) throw new CatalystError("missing", "/events", 404);
    return found;
  });
  vi.mocked(loadJumpPlace).mockResolvedValue(null);
  http.signedGetJSON
    .mockReset()
    .mockResolvedValue({ ok: true, data: [{ id: "up", name: "Watch Party", attending: false }] });
  http.postJSON.mockReset();
  // afterEach resets every mock, so the spy gets the real sign-in back here.
  signIn.openSignIn.mockImplementation(signIn.real!);
  // Creator faces: no profiles unless a test says otherwise, and never the real network.
  vi.stubGlobal(
    "fetch",
    vi.fn(async () => Response.json([])),
  );
});

afterEach(() => {
  cleanup();
  vi.resetAllMocks();
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

const FACE = "https://profile-images.decentraland.org/entities/bafyface/face.png";
const profilesReply = (address: string) =>
  Response.json([{ avatars: [{ ethAddress: address, name: "OtoLam", avatar: { snapshots: { face256: FACE } } }] }]);

describe("EventsView", () => {
  describe("when nothing is live", () => {
    it("should open Live Now with the Foundation's Genesis Plaza at 0,0", async () => {
      setup("/events");
      const section = await screen.findByRole("region", { name: "Live Now" });
      expect(within(section).getByRole("link", { name: "Genesis Plaza" }).getAttribute("href")).toBe(
        "/events?position=0%2C0",
      );
      expect(
        within(section)
          .getByRole("link", { name: /Jump In/ })
          .getAttribute("href"),
      ).toMatch(/\/play\/\?position=0%2C0$/);
      expect(within(section).getByText("Decentraland Foundation")).toBeTruthy();
      expect(within(section).getByText("0 online")).toBeTruthy();
    });

    it("should mark empty days", async () => {
      setup("/events");
      expect(await screen.findAllByText("No events this day.")).not.toHaveLength(0);
    });

    describe("and the node's Genesis Plaza is quiet", () => {
      beforeEach(() => {
        live.places = [placeRecord({ id: "plaza", title: "Genesis Plaza", positions: ["0,0"], user_count: 0 })];
      });

      it("should count nothing live although Genesis Plaza opens the rail", async () => {
        setup("/events");
        expect(await screen.findByRole("link", { name: "Genesis Plaza" })).toBeTruthy();
        await waitFor(() =>
          expect(vi.mocked(track)).toHaveBeenCalledWith("lp_whatson_viewed", expect.objectContaining({ live_count: 0 }), expect.anything()),
        );
      });
    });

    it("should link to the event hosting article on the site", async () => {
      setup("/events");
      expect((await screen.findByRole("link", { name: "Learn More" })).getAttribute("href")).toBe(
        "/blog/about-decentraland/how-to-make-and-run-an-event-in-decentraland",
      );
    });
  });

  describe("when the live feed is unavailable", () => {
    beforeEach(() => {
      vi.mocked(fetchMostActivePlaces).mockRejectedValue(new CatalystError("down", "/places", 503));
    });

    it("should render no Live Now section", async () => {
      setup("/events");
      await screen.findByRole("heading", { name: "All Events" });
      await waitFor(() => expect(fetchMostActivePlaces).toHaveBeenCalled());
      expect(screen.queryByRole("heading", { name: "Live Now" })).toBeNull();
    });
  });

  describe("when one scene is live", () => {
    beforeEach(() => {
      live.places = [placeRecord({ id: "busy", title: "Busy Club", positions: ["9,9"], user_count: 30 })];
    });

    it("should count it as one live item", async () => {
      setup("/events");
      expect(await screen.findByRole("link", { name: "Busy Club" })).toBeTruthy();
      await waitFor(() =>
        expect(vi.mocked(track)).toHaveBeenCalledWith("lp_whatson_viewed", expect.objectContaining({ live_count: 1 }), expect.anything()),
      );
    });
  });

  describe("when a place is busy", () => {
    describe("and the visitor is on desktop", () => {
      beforeEach(() => {
        live = { events: [], places: [placeRecord({ title: "Meta GamiMall", base_position: "1,95", user_count: 6 })] };
      });

      it("should show the place in Live Now with a Jump In to the web explorer", async () => {
        setup("/events");
        const section = (await screen.findByRole("heading", { name: "Live Now" })).closest("section")!;
        expect(within(section).getByRole("link", { name: "Meta GamiMall" }).getAttribute("href")).toBe(
          "/events?position=1%2C95",
        );
        expect(
          within(section)
            .getByRole("link", { name: /Jump In/ })
            .getAttribute("href"),
        ).toMatch(/\/play\/\?position=1%2C95$/);
      });

      it("should name the presence count, not a registration count, for assistive technology", async () => {
        setup("/events");
        const section = (await screen.findByRole("heading", { name: "Live Now" })).closest("section")!;
        expect(within(section).getByText("6 online")).toBeTruthy();
      });
    });

    describe("and the visitor is on an Android phone", () => {
      beforeEach(() => {
        live = { events: [], places: [placeRecord({ base_position: "1,95", user_count: 6 })] };
      });

      it("should open the app instead of the web explorer", async () => {
        setup("/events", { kind: "mobile", os: "android" });
        const section = (await screen.findByRole("heading", { name: "Live Now" })).closest("section")!;
        expect(
          within(section)
            .getByRole("link", { name: /Jump In/ })
            .getAttribute("href"),
        ).toMatch(/^intent:\/\/open\?position=1%2C95#Intent;/);
      });
    });
  });

  describe("when paging through days", () => {
    describe("and the window starts today", () => {
      it("should keep the day window in the URL and page back to today", async () => {
        const { router, user } = setup("/events");
        const previous = await screen.findByRole("button", { name: "Navigate to previous dates" });
        expect((previous as HTMLButtonElement).disabled).toBe(true);
        await user.click(screen.getByRole("button", { name: "Navigate to next dates" }));
        expect(router.state.location.search).toBe("?day=4");
        expect((screen.getByRole("button", { name: "Navigate to previous dates" }) as HTMLButtonElement).disabled).toBe(
          false,
        );
        await user.click(screen.getByRole("button", { name: "Navigate to previous dates" }));
        expect(router.state.location.search).toBe("");
      });
    });

    describe("and the URL offset is beyond a year ahead", () => {
      it("should stop paging forward a year ahead and clamp the offset in the URL", async () => {
        const { router, user } = setup("/events?day=999");
        const next = await screen.findByRole("button", { name: "Navigate to next dates" });
        expect((next as HTMLButtonElement).disabled).toBe(true);
        await user.click(screen.getByRole("button", { name: "Navigate to previous dates" }));
        expect(router.state.location.search).toBe("?day=361");
        expect((screen.getByRole("button", { name: "Navigate to next dates" }) as HTMLButtonElement).disabled).toBe(false);
        await user.click(screen.getByRole("button", { name: "Navigate to next dates" }));
        expect(router.state.location.search).toBe("?day=365");
      });
    });
  });

  describe("when a member is signed in", () => {
    beforeEach(() => {
      auth.identity = { signer: "0xAB" } as unknown as AuthIdentity;
    });

    it("should offer All Events and My Events as link tabs, the current one selected", async () => {
      const { router, user } = setup("/events");
      const views = await screen.findByRole("tablist", { name: "Events views" });
      expect(within(views).getByRole("tab", { name: "All Events" }).getAttribute("aria-selected")).toBe("true");
      const mine = within(views).getByRole("tab", { name: "My Events" });
      expect(mine.getAttribute("href")).toBe("/events?tab=my");
      await user.click(mine);
      expect(router.state.location.search).toBe("?tab=my");
    });

    describe("and they click Remind me", () => {
      beforeEach(async () => {
        http.postJSON.mockResolvedValue({ ok: true });
        const { user } = setup("/events");
        const button = await screen.findByRole("button", { name: "Remind me: Watch Party" });
        await waitFor(() => expect((button as HTMLButtonElement).disabled).toBe(false));
        await user.click(button);
      });

      it("should save the reminder", async () => {
        await waitFor(() => expect(http.postJSON).toHaveBeenCalled());
      });

      it("should not send them to sign in", () => {
        expect(signIn.openSignIn).not.toHaveBeenCalled();
      });
    });

    describe("and a reminder write fails", () => {
      let fail: (error: Error) => void;

      beforeEach(() => {
        fail = () => {};
        http.postJSON.mockReturnValueOnce(new Promise((_, reject) => (fail = reject)));
      });

      it("should roll the reminder back and say so", async () => {
        const { user } = setup("/events");
        const button = await screen.findByRole("button", { name: "Remind me: Watch Party" });
        await waitFor(() => expect((button as HTMLButtonElement).disabled).toBe(false));
        await user.click(button);
        await waitFor(() => expect(button.getAttribute("aria-pressed")).toBe("true"));
        await act(async () => fail(new Error("401")));
        await waitFor(() => expect(button.getAttribute("aria-pressed")).toBe("false"));
        expect(await screen.findByText("Couldn't update your reminder. Try again.")).toBeTruthy();
      });
    });
  });

  describe("when the visitor is signed out", () => {
    it("should show the section title instead of the views", async () => {
      setup("/events");
      expect(await screen.findByRole("heading", { name: "All Events" })).toBeTruthy();
      expect(screen.queryByRole("tablist", { name: "Events views" })).toBeNull();
    });

    describe("and they click Remind me", () => {
      let assign: ReturnType<typeof vi.fn>;

      beforeEach(async () => {
        const { user } = setup("/events");
        const button = await screen.findByRole("button", { name: "Remind me: Watch Party" });
        assign = vi.fn();
        vi.stubGlobal("location", { origin: "https://example.org", pathname: "/events", search: "?tab=all", hash: "", assign });
        await user.click(button);
        vi.unstubAllGlobals();
      });

      it("should send them to the sign-in page, to come back here", () => {
        expect(assign).toHaveBeenCalledWith("/auth/login?redirectTo=%2Fevents%3Ftab%3Dall");
      });

      it("should read nothing signed", () => {
        expect(http.signedGetJSON).not.toHaveBeenCalled();
      });

      it("should write nothing", () => {
        expect(http.postJSON).not.toHaveBeenCalled();
      });
    });
  });

  describe("when the upcoming read settles after the page renders", () => {
    let release: () => void;

    beforeEach(() => {
      release = () => {};
      const gate = new Promise<void>((resolve) => (release = resolve));
      vi.mocked(fetchEvents).mockImplementation(async (params) => {
        if (params?.list === "upcoming") {
          await gate;
          return { data: upcoming, total: upcoming.length };
        }
        return { data: params?.list === "live" ? [] : calendar, total: 0 };
      });
    });

    it("should report the page view once, after the reads settle, with their counts", async () => {
      setup("/events");
      await screen.findByRole("heading", { name: "All Events" });
      expect(vi.mocked(track).mock.calls.filter(([name]) => name === "lp_whatson_viewed")).toHaveLength(0);
      release();
      await waitFor(() =>
        expect(vi.mocked(track)).toHaveBeenCalledWith(
          "lp_whatson_viewed",
          { live_count: 0, upcoming_count: 1, filter: null, search: null },
          expect.anything(),
        ),
      );
      expect(vi.mocked(track).mock.calls.filter(([name]) => name === "lp_whatson_viewed")).toHaveLength(1);
    });
  });

  describe("when the calendar is unavailable", () => {
    let fail: boolean;

    beforeEach(() => {
      fail = true;
      vi.mocked(fetchEvents).mockImplementation(async (params) => {
        if (params?.list === "active" && fail) throw new CatalystError("down", "/events", 502);
        return { data: params?.list === "active" ? calendar : [], total: 0 };
      });
    });

    it("should say so with a Retry that reads it again", async () => {
      const { user } = setup("/events");
      const alert = await screen.findByRole("alert");
      expect(alert.textContent).toContain("Events are temporarily unavailable.");
      fail = false;
      await user.click(within(alert).getByRole("button", { name: "Retry" }));
      await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
      expect((await screen.findAllByRole("link", { name: "Poker Night" })).length).toBeGreaterThan(0);
    });
  });

  describe("when a card is clicked", () => {
    it("should open its detail in the URL and close it with Escape", async () => {
      const { router, user } = setup("/events");
      const [card] = await screen.findAllByRole("link", { name: "Watch Party" });
      await user.click(card!);
      expect(router.state.location.search).toBe("?id=up");
      const dialog = await screen.findByRole("dialog", { name: "Watch Party" });
      expect(within(dialog).getByText("Cards and chips.")).toBeTruthy();
      await user.keyboard("{Escape}");
      await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
      expect(router.state.location.search).toBe("");
    });
  });

  describe("when the URL deep-links an event", () => {
    describe("and the service knows it", () => {
      it("should open it and keep focus inside the dialog", async () => {
        const { user } = setup("/events?id=cal");
        const dialog = await screen.findByRole("dialog", { name: "Poker Night" });
        await waitFor(() => expect(dialog.contains(document.activeElement)).toBe(true));
        for (let i = 0; i < 8; i++) await user.tab();
        expect(dialog.contains(document.activeElement)).toBe(true);
      });
    });

    describe("and the service does not know it", () => {
      it("should close the deep link and keep the other parameters", async () => {
        const { router } = setup("/events?id=missing&day=4");
        await waitFor(() => expect(router.state.location.search).toBe("?day=4"));
        expect(screen.queryByRole("dialog")).toBeNull();
      });
    });

    describe("and the events service is down", () => {
      beforeEach(() => {
        vi.mocked(fetchEvent).mockRejectedValue(new CatalystError("down", "/events", 503));
      });

      it("should keep the deep link in the URL without opening a dialog", async () => {
        const { router } = setup("/events?id=cal");
        await screen.findByRole("heading", { name: "All Events" });
        await waitFor(() => expect(fetchEvent).toHaveBeenCalled());
        expect(router.state.location.search).toBe("?id=cal");
        expect(screen.queryByRole("dialog")).toBeNull();
      });
    });
  });

  describe("when the URL deep-links a place", () => {
    beforeEach(() => {
      vi.mocked(loadJumpPlace).mockResolvedValue({
        title: "Genesis Plaza",
        description: "Spawn point.",
        image: null,
        base_position: "-3,-2",
        owner: null,
        contact_name: "Decentraland Foundation",
        user_count: 11,
        world: false,
        world_name: null,
      });
    });

    it("should open it at the parcel it names", async () => {
      setup("/events?position=0,0");
      const dialog = await screen.findByRole("dialog", { name: "Genesis Plaza" });
      expect(within(dialog).getByText("(0,0)")).toBeTruthy();
      expect(within(dialog).getByText("11 people here now")).toBeTruthy();
    });
  });

  describe("when sections share a creator with a profile face", () => {
    const wallet = "0x1111111111111111111111111111111111111111";
    let fetchMock: Mock<() => Promise<Response>>;

    beforeEach(() => {
      fetchMock = vi.fn(async () => profilesReply(wallet));
      vi.stubGlobal("fetch", fetchMock);
      calendar = [soon("cal", "Poker Night"), soon("cal2", "Dance Night", 3)];
    });

    it("should show creator faces from one batched profile read per section", async () => {
      setup("/events");
      await waitFor(() =>
        expect(
          document.querySelectorAll('img[src*="/profile-images/entities/bafyface/face.png"]').length,
        ).toBeGreaterThan(1),
      );
      // Upcoming and All Events share the creator: the second section asks for no wallet the first did.
      expect(fetchMock).toHaveBeenCalledOnce();
      expect(JSON.parse(String((fetchMock.mock.calls[0] as unknown as [string, RequestInit])[1].body))).toEqual({
        ids: [wallet],
      });
    });
  });

  describe("when the profile read fails", () => {
    beforeEach(() => {
      vi.stubGlobal(
        "fetch",
        vi.fn(async () => new Response("down", { status: 502 })),
      );
    });

    it("should keep the creator's initial", async () => {
      setup("/events");
      await screen.findAllByRole("link", { name: "Watch Party" });
      await waitFor(() => expect(fetch).toHaveBeenCalled());
      expect(document.querySelector('img[src*="/profile-images/"]')).toBeNull();
      expect(screen.getAllByText("O").length).toBeGreaterThan(0);
    });
  });
});

describe("when calendar occupancy is available or unknown", () => {
  describe.each([
    { connected: undefined, expected: null, state: "missing" },
    { connected: null, expected: null, state: "null" },
    { connected: [], expected: "0 online", state: "known zero" },
    {
      connected: ["0xABC", "0xabc", "0xDEF"],
      expected: "2 online",
      state: "known distinct users",
    },
  ])("and a live event has $state occupancy", ({ connected, expected }) => {
    beforeEach(() => {
      calendar = [
        {
          ...soon("occupancy", "Occupancy check", -0.5),
          live: true,
          total_attendees: 26,
          connected_addresses: connected,
        } as Event,
      ];
    });

    it("should use it without substituting registrations", async () => {
      setup("/events");
      const title = await screen.findByRole("link", {
        name: "Occupancy check",
      });
      const card = title.closest("article")!;
      expect(within(card).getByText("LIVE")).toBeTruthy();
      expect(within(card).queryByText("26 online")).toBeNull();
      if (expected) expect(within(card).getByText(expected)).toBeTruthy();
      else expect(within(card).queryByText(/online$/)).toBeNull();
    });
  });

  describe("and live events are in desktop day columns", () => {
    beforeEach(() => {
      vi.stubGlobal("innerWidth", 1440);
      calendar = [
        { ...soon("parcel", "Live parcel", -0.5), live: true, x: 3, y: 7 },
        { ...soon("world", "Live world", -0.5), live: true, x: 3, y: 7, world: true, server: "gallery.dcl.eth" },
      ];
      setup("/events");
    });

    it("should link the live parcel to its coordinates", async () => {
      const jump = await screen.findByRole("link", { name: "Jump in: Live parcel" });
      expect(jump.getAttribute("href")).toMatch(/\/play\/\?position=3%2C7(&|$)/);
    });

    it("should link the live World to its named realm", async () => {
      const jump = await screen.findByRole("link", { name: "Jump in: Live world" });
      expect(jump.getAttribute("href")).toContain("realm=gallery.dcl.eth");
    });
  });

  describe("and a live event is in the phone list", () => {
    let liveCard: HTMLElement;

    beforeEach(async () => {
      vi.stubGlobal("innerWidth", 390);
      calendar = [{ ...soon("now", "Live on phone", -0.5), live: true, x: 3, y: 7 }];
      setup("/events");
      const list = await screen.findByRole("list", { name: /^Events for / });
      liveCard = within(list).getByRole("link", { name: "Live on phone" }).closest("article")!;
    });

    it("should give it JUMP IN", () => {
      expect(within(liveCard).getByRole("link", { name: "Jump in: Live on phone" }).getAttribute("href")).toMatch(
        /\/play\/\?position=3%2C7(&|$)/,
      );
    });

    it("should not offer Remind me", () => {
      expect(within(liveCard).queryByRole("button", { name: /Remind me/ })).toBeNull();
    });

    it("should offer Copy link", () => {
      expect(within(liveCard).getByRole("button", { name: "Copy link: Live on phone" })).toBeTruthy();
    });
  });

  describe("and a future event has registrations", () => {
    let card: HTMLElement;

    beforeEach(async () => {
      calendar = [soon("future", "Future registration count")];
      calendar[0]!.total_attendees = 26;
      setup("/events");
      const title = await screen.findByRole("link", {
        name: "Future registration count",
      });
      card = title.closest("article")!;
    });

    it("should not present them as current occupancy", () => {
      expect(within(card).queryByText(/online$/)).toBeNull();
    });

    it("should not mark the event as live", () => {
      expect(within(card).queryByText("LIVE")).toBeNull();
    });
  });
});

describe("columnsFor", () => {
  describe.each([
    [1920, 5],
    [1536, 5],
    [1535, 4],
    [1024, 4],
    [1023, 2],
    [768, 2],
    [767, 1],
    [390, 1],
  ])("when the screen is %ipx wide", (width, columns) => {
    it(`should show ${columns} column(s)`, () => {
      expect(columnsFor(width)).toBe(columns);
    });
  });
});

describe("when /events opens My Events", () => {
  const SIGNER = "0x1111111111111111111111111111111111111111";
  const owned = (over: Partial<Event>) => eventRecord({ user: SIGNER, ...over }) as unknown as ManagedEvent;
  const at = (hours: number) => new Date(Date.now() + hours * HOUR).toISOString();
  let scrolled: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    scrolled = vi.fn();
    Element.prototype.scrollIntoView = scrolled as unknown as Element["scrollIntoView"];
  });

  describe("and the member is signed in", () => {
    beforeEach(() => {
      auth.identity = { signer: SIGNER } as unknown as AuthIdentity;
    });

    it("should keep the whole page and list the member's upcoming events in start order, each opening its editor with its review status", async () => {
      vi.mocked(loadAllOwnedEvents).mockResolvedValue([
          owned({ id: "later", name: "Later Party", start_at: at(48), finish_at: at(50), next_start_at: at(48), next_finish_at: at(50) }),
          owned({ id: "draft", name: "Draft Night", approved: false, start_at: at(5), finish_at: at(6), next_start_at: at(5), next_finish_at: at(6) }),
          owned({ id: "no", name: "Turned Down", approved: false, rejected: true, start_at: at(10), finish_at: at(11), next_start_at: at(10), next_finish_at: at(11) }),
          owned({ id: "over", name: "Ended Show", start_at: at(-5), finish_at: at(-4), next_start_at: at(-5), next_finish_at: at(-4) }),
          owned({ id: "gone", name: "Deleted One", deleted_by_user: true, start_at: at(3), finish_at: at(4), next_start_at: at(3), next_finish_at: at(4) }),
          owned({ id: "theirs", name: "Not Mine", user: "0x2222222222222222222222222222222222222222", start_at: at(2), finish_at: at(3), next_start_at: at(2), next_finish_at: at(3) }),
        ]);
      setup("/events?tab=my");
      const panel = await screen.findByRole("tabpanel", { name: "My Events" });
      await within(panel).findByRole("heading", { name: "Hosted by me" });
      const links = within(panel).getAllByRole("link").filter((link) => link.getAttribute("href")?.includes("/edit-event/"));
      expect(links.map((link) => [link.textContent, link.getAttribute("href")])).toEqual([
        ["Draft Night", "/events/edit-event/draft"],
        ["Turned Down", "/events/edit-event/no"],
        ["Later Party", "/events/edit-event/later"],
      ]);
      const items = within(panel).getAllByRole("listitem");
      expect(within(items[0]!).getByText("Pending")).toBeTruthy();
      expect(within(items[1]!).getByText("Rejected")).toBeTruthy();
      expect(within(items[2]!).getByText("Approved")).toBeTruthy();
      expect(screen.getByRole("tab", { name: "My Events" }).getAttribute("aria-selected")).toBe("true");
      expect(screen.queryByRole("navigation", { name: "Event days" })).toBeNull();
      expect(screen.getByRole("heading", { name: "Upcoming" })).toBeTruthy();
      expect(vi.mocked(loadAllOwnedEvents)).toHaveBeenCalledWith(auth.identity, expect.anything());
      await waitFor(() => expect(scrolled).toHaveBeenCalledOnce());
      expect(scrolled.mock.contexts[0]).toBe(document.getElementById("all-events"));
    });

    it("should show an uploaded cover the page's own http origin serves", async () => {
      const poster = "/events-assets-099ac00/poster/abc.png";
      vi.mocked(loadAllOwnedEvents).mockResolvedValue([
          owned({ id: "covered", name: "Covered", image: `http://127.0.0.1:5254${poster}`, start_at: at(5), finish_at: at(6), next_start_at: at(5), next_finish_at: at(6) }),
        ]);
      setup("/events?tab=my", DESKTOP, "http://127.0.0.1:5254");
      const panel = await screen.findByRole("tabpanel", { name: "My Events" });
      const card = (await within(panel).findByRole("link", { name: "Covered" })).closest("li")!;
      expect(card.querySelector("img")?.getAttribute("src")).toBe(poster);
    });

    it("should count a recurring event by its next occurrence", async () => {
      vi.mocked(loadAllOwnedEvents).mockResolvedValue([
          owned({ id: "weekly", name: "Weekly", recurrent: true, start_at: at(-200), finish_at: at(-199), next_start_at: at(30), next_finish_at: at(31) }),
        ]);
      setup("/events?tab=my");
      const panel = await screen.findByRole("tabpanel", { name: "My Events" });
      expect((await within(panel).findByRole("link", { name: "Weekly" })).getAttribute("href")).toBe("/events/edit-event/weekly");
    });

    it("should show the empty card with Create event when the member has no upcoming events", async () => {
      vi.mocked(loadAllOwnedEvents).mockResolvedValue([owned({ id: "over", start_at: at(-5), finish_at: at(-4), next_start_at: at(-5), next_finish_at: at(-4) })]);
      setup("/events?tab=my");
      const panel = await screen.findByRole("tabpanel", { name: "My Events" });
      expect(await within(panel).findByText("You don't have any event created")).toBeTruthy();
      expect(within(panel).getByRole("link", { name: "Create event" }).getAttribute("href")).toBe("/events/new-event");
      await waitFor(() => expect(scrolled).toHaveBeenCalledOnce());
    });

    it("should report a failed read with a Retry that reads again, and no empty card", async () => {
      vi.mocked(loadAllOwnedEvents).mockRejectedValueOnce(new CatalystError("down", "/events/api/events", 503));
      vi.mocked(loadAllOwnedEvents).mockResolvedValueOnce([]);
      const { user } = setup("/events?tab=my");
      const alert = await screen.findByRole("alert");
      expect(alert.textContent).toContain("Couldn't load your events.");
      expect(screen.queryByText("You don't have any event created")).toBeNull();
      await user.click(within(alert).getByRole("button", { name: "Retry" }));
      expect(await screen.findByText("You don't have any event created")).toBeTruthy();
      expect(vi.mocked(loadAllOwnedEvents)).toHaveBeenCalledTimes(2);
    });

    it("should keep the listed events when a later read fails", async () => {
      vi.mocked(loadAllOwnedEvents).mockResolvedValueOnce([
        owned({ id: "kept", name: "Kept Party", start_at: at(5), finish_at: at(6), next_start_at: at(5), next_finish_at: at(6) }),
      ]);
      vi.mocked(loadAllOwnedEvents).mockRejectedValueOnce(new CatalystError("down", "/events/api/events", 503));
      const { client } = setup("/events?tab=my");
      const panel = await screen.findByRole("tabpanel", { name: "My Events" });
      await within(panel).findByRole("link", { name: "Kept Party" });

      // The observer reports on a later task, so the failed read is rendered before the page is read.
      await act(async () => {
        await client.refetchQueries();
        await new Promise((resolve) => setTimeout(resolve, 0));
      });
      expect(client.getQueryState(["private", SIGNER, "events", "mine"])?.status).toBe("error");

      expect(vi.mocked(loadAllOwnedEvents)).toHaveBeenCalledTimes(2);
      expect(within(panel).getByRole("link", { name: "Kept Party" })).toBeTruthy();
      expect(screen.queryByText("Couldn't load your events.")).toBeNull();
    });

    it("should go back to All Events from its tab without leaving the page", async () => {
      vi.mocked(loadAllOwnedEvents).mockResolvedValue([]);
      const { router, user } = setup("/events?tab=my");
      await screen.findByText("You don't have any event created");
      await user.click(screen.getByRole("tab", { name: "All Events" }));
      expect(router.state.location.search).toBe("");
      expect(await screen.findByRole("tabpanel", { name: "All Events" })).toBeTruthy();
    });

    it("should fall back to All Events, without sending to sign in, when the member signs out on it", async () => {
      vi.mocked(loadAllOwnedEvents).mockResolvedValue([]);
      const { router } = setup("/events?tab=my");
      await screen.findByText("You don't have any event created");
      auth.identity = null;
      await act(() => router.navigate("/events?tab=my&day=1"));
      await waitFor(() => expect(router.state.location.search).toBe("?day=1"));
      expect(await screen.findByRole("heading", { name: "All Events" })).toBeTruthy();
      expect(signIn.openSignIn).not.toHaveBeenCalled();
    });
  });

  it("should send a signed-out visitor to sign in and back to My Events, reading nothing", async () => {
    const assign = vi.fn();
    vi.stubGlobal("location", { origin: "https://example.org", pathname: "/events", search: "?tab=my", hash: "", assign });
    setup("/events?tab=my");
    await waitFor(() => expect(assign).toHaveBeenCalledWith("/auth/login?redirectTo=%2Fevents%3Ftab%3Dmy"));
    expect(assign).toHaveBeenCalledOnce();
    expect(vi.mocked(loadAllOwnedEvents)).not.toHaveBeenCalled();
    expect(screen.queryByRole("tabpanel", { name: "My Events" })).toBeNull();
  });
});
