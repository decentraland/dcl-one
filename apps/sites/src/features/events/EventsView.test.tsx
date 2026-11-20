// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

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
vi.mock("../components/auth/signin-store", async (original) => {
  const real = await original<typeof import("../components/auth/signin-store")>();
  signIn.openSignIn.mockImplementation(real.openSignIn);
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
  signIn.openSignIn.mockClear();
  // Creator faces: no profiles unless a test says otherwise, and never the real network.
  vi.stubGlobal(
    "fetch",
    vi.fn(async () => Response.json([])),
  );
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

/** The event card (an article) whose title link has this name. */
function cardIn(scope: HTMLElement, name: string): HTMLElement {
  const card = within(scope).getAllByRole("article").find((article) => within(article).queryByRole("link", { name }));
  if (!card) throw new Error(`No card titled ${name}`);
  return card;
}

async function findCard(name: string): Promise<HTMLElement> {
  await screen.findByRole("link", { name });
  return cardIn(document.body, name);
}

const FACE = "https://profile-images.decentraland.org/entities/bafyface/face.png";
const profilesReply = (address: string) =>
  Response.json([{ avatars: [{ ethAddress: address, name: "OtoLam", avatar: { snapshots: { face256: FACE } } }] }]);

describe("EventsView", () => {
  it("should open Live Now with the Foundation's Genesis Plaza at 0,0 when nothing is live and mark empty days", async () => {
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
    expect(await screen.findAllByText("No events this day.")).not.toHaveLength(0);
  });

  it("should render no Live Now section while the live feed is unavailable", async () => {
    vi.mocked(fetchMostActivePlaces).mockRejectedValue(new CatalystError("down", "/places", 503));
    setup("/events");
    await screen.findByRole("heading", { name: "All Events" });
    await waitFor(() => expect(fetchMostActivePlaces).toHaveBeenCalled());
    expect(screen.queryByRole("heading", { name: "Live Now" })).toBeNull();
  });

  it("should link to the event hosting article on the site", async () => {
    setup("/events");
    expect((await screen.findByRole("link", { name: "Learn More" })).getAttribute("href")).toBe(
      "/blog/about-decentraland/how-to-make-and-run-an-event-in-decentraland",
    );
  });

  it("should show a busy place in Live Now with a Jump In to the web explorer on desktop", async () => {
    live = { events: [], places: [placeRecord({ title: "Meta GamiMall", base_position: "1,95", user_count: 6 })] };
    setup("/events");
    const section = await screen.findByRole("region", { name: "Live Now" });
    expect(within(section).getByRole("link", { name: "Meta GamiMall" }).getAttribute("href")).toBe(
      "/events?position=1%2C95",
    );
    expect(
      within(section)
        .getByRole("link", { name: /Jump In/ })
        .getAttribute("href"),
    ).toMatch(/\/play\/\?position=1%2C95$/);
    // The presence count, not a registration count, named for assistive technology.
    expect(within(section).getByText("6 online")).toBeTruthy();
  });

  it("should open the app on an Android phone instead of the web explorer", async () => {
    live = { events: [], places: [placeRecord({ base_position: "1,95", user_count: 6 })] };
    setup("/events", { kind: "mobile", os: "android" });
    const section = await screen.findByRole("region", { name: "Live Now" });
    expect(
      within(section)
        .getByRole("link", { name: /Jump In/ })
        .getAttribute("href"),
    ).toMatch(/^intent:\/\/open\?position=1%2C95#Intent;/);
  });

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

  it("should stop paging forward a year ahead and clamp a longer offset in the URL", async () => {
    const { router, user } = setup("/events?day=999");
    const next = await screen.findByRole("button", { name: "Navigate to next dates" });
    expect((next as HTMLButtonElement).disabled).toBe(true);
    await user.click(screen.getByRole("button", { name: "Navigate to previous dates" }));
    expect(router.state.location.search).toBe("?day=361");
    expect((screen.getByRole("button", { name: "Navigate to next dates" }) as HTMLButtonElement).disabled).toBe(false);
    await user.click(screen.getByRole("button", { name: "Navigate to next dates" }));
    expect(router.state.location.search).toBe("?day=365");
  });

  it("should offer All Events and My Events as link tabs to a signed-in member, the current one selected", async () => {
    auth.identity = { signer: "0xAB" } as unknown as AuthIdentity;
    const { router, user } = setup("/events");
    const views = await screen.findByRole("tablist", { name: "Events views" });
    expect(within(views).getByRole("tab", { name: "All Events" }).getAttribute("aria-selected")).toBe("true");
    const mine = within(views).getByRole("tab", { name: "My Events" });
    expect(mine.getAttribute("href")).toBe("/events?tab=my");
    await user.click(mine);
    expect(router.state.location.search).toBe("?tab=my");
  });

  it("should show the section title instead of the views when signed out", async () => {
    setup("/events");
    expect(await screen.findByRole("heading", { name: "All Events" })).toBeTruthy();
    expect(screen.queryByRole("tablist", { name: "Events views" })).toBeNull();
  });

  it("should report the page view once its reads settle, with their counts", async () => {
    let release: () => void = () => {};
    const gate = new Promise<void>((resolve) => (release = resolve));
    vi.mocked(fetchEvents).mockImplementation(async (params) => {
      if (params?.list === "upcoming") {
        await gate;
        return { data: upcoming, total: upcoming.length };
      }
      return { data: params?.list === "live" ? [] : calendar, total: 0 };
    });
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

  it("should count nothing live on a quiet node although Genesis Plaza opens the rail", async () => {
    live.places = [placeRecord({ id: "plaza", title: "Genesis Plaza", positions: ["0,0"], user_count: 0 })];
    setup("/events");
    expect(await screen.findByRole("link", { name: "Genesis Plaza" })).toBeTruthy();
    await waitFor(() =>
      expect(vi.mocked(track)).toHaveBeenCalledWith("lp_whatson_viewed", expect.objectContaining({ live_count: 0 }), expect.anything()),
    );
  });

  it("should count one live scene as one live item", async () => {
    live.places = [placeRecord({ id: "busy", title: "Busy Club", positions: ["9,9"], user_count: 30 })];
    setup("/events");
    expect(await screen.findByRole("link", { name: "Busy Club" })).toBeTruthy();
    await waitFor(() =>
      expect(vi.mocked(track)).toHaveBeenCalledWith("lp_whatson_viewed", expect.objectContaining({ live_count: 1 }), expect.anything()),
    );
  });

  it("should show an unavailable calendar with a Retry that reads it again", async () => {
    let fail = true;
    vi.mocked(fetchEvents).mockImplementation(async (params) => {
      if (params?.list === "active" && fail) throw new CatalystError("down", "/events", 502);
      return { data: params?.list === "active" ? calendar : [], total: 0 };
    });
    const { user } = setup("/events");
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("Events are temporarily unavailable.");
    fail = false;
    await user.click(within(alert).getByRole("button", { name: "Retry" }));
    await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
    expect((await screen.findAllByRole("link", { name: "Poker Night" })).length).toBeGreaterThan(0);
  });

  it("should open a card's detail in the URL and close it with Escape", async () => {
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

  it("should open a deep-linked event and keep focus inside the dialog", async () => {
    const { user } = setup("/events?id=cal");
    const dialog = await screen.findByRole("dialog", { name: "Poker Night" });
    await waitFor(() => expect(dialog.contains(document.activeElement)).toBe(true));
    for (let i = 0; i < 8; i++) await user.tab();
    expect(dialog.contains(document.activeElement)).toBe(true);
  });

  it("should close a deep link to an event the service does not know", async () => {
    const { router } = setup("/events?id=missing&day=4");
    await waitFor(() => expect(router.state.location.search).toBe("?day=4"));
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("should keep a deep link open in the URL while the events service is down", async () => {
    vi.mocked(fetchEvent).mockRejectedValue(new CatalystError("down", "/events", 503));
    const { router } = setup("/events?id=cal");
    await screen.findByRole("heading", { name: "All Events" });
    await waitFor(() => expect(fetchEvent).toHaveBeenCalled());
    expect(router.state.location.search).toBe("?id=cal");
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("should open a place deep link at the parcel it names", async () => {
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
    setup("/events?position=0,0");
    const dialog = await screen.findByRole("dialog", { name: "Genesis Plaza" });
    expect(within(dialog).getByText("(0,0)")).toBeTruthy();
    expect(within(dialog).getByText("11 people here now")).toBeTruthy();
  });

  it("should send a signed-out visitor to the sign-in page from Remind me, to come back here, and write nothing", async () => {
    const { user } = setup("/events");
    const button = await screen.findByRole("button", { name: "Remind me: Watch Party" });
    const assign = vi.fn();
    vi.stubGlobal("location", { origin: "https://example.org", pathname: "/events", search: "?tab=all", hash: "", assign });
    await user.click(button);
    vi.unstubAllGlobals();
    expect(assign).toHaveBeenCalledWith("/auth/login?redirectTo=%2Fevents%3Ftab%3Dall");
    expect(http.signedGetJSON).not.toHaveBeenCalled();
    expect(http.postJSON).not.toHaveBeenCalled();
  });

  it("should not send a signed-in member to sign in from Remind me, and save the reminder", async () => {
    auth.identity = { signer: "0xAB" } as unknown as AuthIdentity;
    http.postJSON.mockResolvedValue({ ok: true });
    const { user } = setup("/events");
    const button = await screen.findByRole("button", { name: "Remind me: Watch Party" });
    await waitFor(() => expect((button as HTMLButtonElement).disabled).toBe(false));
    await user.click(button);
    await waitFor(() => expect(http.postJSON).toHaveBeenCalled());
    expect(signIn.openSignIn).not.toHaveBeenCalled();
  });

  it("should roll a failed reminder back and say so", async () => {
    auth.identity = { signer: "0xAB" } as unknown as AuthIdentity;
    let fail: (error: Error) => void = () => {};
    http.postJSON.mockReturnValue(new Promise((_, reject) => (fail = reject)));
    const { user } = setup("/events");
    const button = await screen.findByRole("button", { name: "Remind me: Watch Party" });
    await waitFor(() => expect((button as HTMLButtonElement).disabled).toBe(false));
    await user.click(button);
    await waitFor(() => expect(button.getAttribute("aria-pressed")).toBe("true"));
    await act(async () => fail(new Error("401")));
    await waitFor(() => expect(button.getAttribute("aria-pressed")).toBe("false"));
    expect(await screen.findByText("Couldn't update your reminder. Try again.")).toBeTruthy();
  });

  it("should show creator faces from one batched profile read per section", async () => {
    const wallet = "0x1111111111111111111111111111111111111111";
    const fetchMock = vi.fn(async () => profilesReply(wallet));
    vi.stubGlobal("fetch", fetchMock);
    calendar = [soon("cal", "Poker Night"), soon("cal2", "Dance Night", 3)];
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

  it("should keep the creator's initial when the profile read fails", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => new Response("down", { status: 502 })),
    );
    setup("/events");
    await screen.findAllByRole("link", { name: "Watch Party" });
    await waitFor(() => expect(fetch).toHaveBeenCalled());
    expect(document.querySelector('img[src*="/profile-images/"]')).toBeNull();
    expect(screen.getAllByText("O").length).toBeGreaterThan(0);
  });
});

describe("when calendar occupancy is available or unknown", () => {
  it.each([
    { connected: undefined, expected: null, state: "missing" },
    { connected: null, expected: null, state: "null" },
    { connected: [], expected: "0 online", state: "known zero" },
    {
      connected: ["0xABC", "0xabc", "0xDEF"],
      expected: "2 online",
      state: "known distinct users",
    },
  ])("should use $state occupancy without substituting registrations", async ({ connected, expected }) => {
    calendar = [
      {
        ...soon("occupancy", "Occupancy check", -0.5),
        live: true,
        total_attendees: 26,
        connected_addresses: connected,
      } as Event,
    ];
    setup("/events");
    const card = await findCard("Occupancy check");
    expect(within(card).getByText("LIVE")).toBeTruthy();
    expect(within(card).queryByText("26 online")).toBeNull();
    if (expected) expect(within(card).getByText(expected)).toBeTruthy();
    else expect(within(card).queryByText(/online$/)).toBeNull();
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

  it("should give a live event in the phone list its JUMP IN instead of Remind me", async () => {
    vi.stubGlobal("innerWidth", 390);
    calendar = [{ ...soon("now", "Live on phone", -0.5), live: true, x: 3, y: 7 }];
    setup("/events");
    const list = await screen.findByRole("list", { name: /^Events for / });
    const liveCard = cardIn(list, "Live on phone");
    expect(within(liveCard).getByRole("link", { name: "Jump in: Live on phone" }).getAttribute("href")).toMatch(
      /\/play\/\?position=3%2C7(&|$)/,
    );
    expect(within(liveCard).queryByRole("button", { name: /Remind me/ })).toBeNull();
    expect(within(liveCard).getByRole("button", { name: "Copy link: Live on phone" })).toBeTruthy();
  });

  it("should not present future event registrations as current occupancy", async () => {
    calendar = [soon("future", "Future registration count")];
    calendar[0]!.total_attendees = 26;
    setup("/events");
    const card = await findCard("Future registration count");
    expect(within(card).queryByText(/online$/)).toBeNull();
    expect(within(card).queryByText("LIVE")).toBeNull();
  });
});

describe("columnsFor", () => {
  it("should show five days on wide screens, four from desktop, two on tablets and a list on phones", () => {
    expect([1920, 1536, 1535, 1024, 1023, 768, 767, 390].map(columnsFor)).toEqual([5, 5, 4, 4, 2, 2, 1, 1]);
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
