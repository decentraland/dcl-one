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
const signIn = vi.hoisted(() => ({ openSignIn: vi.fn() }));
vi.mock("../components/auth/signin-store", () => signIn);
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

import type { AuthIdentity } from "@/data/lib/auth/types";
import { CatalystError } from "@/data/lib/catalyst/client";
import { track } from "@/core/lib/telemetry/track";
import { loadJumpPlace } from "@/data/lib/catalyst/jump/place-lookup";
import { fetchEvent, fetchEvents, type Event } from "@/data/lib/catalyst/places/events";
import { fetchMostActivePlaces } from "@/data/lib/catalyst/places/index";
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

function setup(path: string, platform: DetectedPlatform = DESKTOP) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const router = createMemoryRouter(
    [
      {
        path: "/events",
        element: <EventsView sid="sid" platform={platform} origin="https://example.org" initial={null} />,
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
  signIn.openSignIn.mockReset();
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
    it("should render no Live Now section and mark empty days", async () => {
      setup("/events");
      await screen.findByRole("heading", { name: "All Events" });
      expect(screen.queryByRole("heading", { name: "Live Now" })).toBeNull();
      expect(await screen.findAllByText("No events this day.")).not.toHaveLength(0);
    });

    it("should link to the event hosting article on the site", async () => {
      setup("/events");
      expect((await screen.findByRole("link", { name: "Learn More" })).getAttribute("href")).toBe(
        "/blog/about-decentraland/how-to-make-and-run-an-event-in-decentraland",
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

    it("should offer All Events and My Events as links, the current one marked", async () => {
      const { router, user } = setup("/events");
      const views = await screen.findByRole("navigation", { name: "Events views" });
      expect(within(views).queryByRole("tab")).toBeNull();
      expect(within(views).getByRole("link", { name: "All Events" }).getAttribute("aria-current")).toBe("page");
      const mine = within(views).getByRole("link", { name: "My Events" });
      expect(mine.getAttribute("href")).toBe("/events?tab=my");
      await user.click(mine);
      expect(router.state.location.search).toBe("?tab=my");
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
      expect(screen.queryByRole("navigation", { name: "Events views" })).toBeNull();
    });

    describe("and they click Remind me", () => {
      beforeEach(async () => {
        const { user } = setup("/events");
        await user.click(await screen.findByRole("button", { name: "Remind me: Watch Party" }));
      });

      it("should ask them to sign in", () => {
        expect(signIn.openSignIn).toHaveBeenCalledWith({ inPlace: true });
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
