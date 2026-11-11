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
import { loadJumpPlace } from "@/data/lib/catalyst/jump/place-lookup";
import { fetchEvent, fetchEvents, type Event } from "@/data/lib/catalyst/places/events";
import { fetchMostActivePlaces } from "@/data/lib/catalyst/places/index";
import type { DetectedPlatform } from "../download/options";
import { EventsView } from "./EventsView";
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

beforeEach(() => {
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
  http.signedGetJSON.mockReset().mockResolvedValue({ ok: true, data: { attending: false } });
  http.postJSON.mockReset();
  signIn.openSignIn.mockReset();
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("EventsView", () => {
  it("should render no Live Now section when nothing is live and mark empty days", async () => {
    setup("/events");
    await screen.findByRole("heading", { name: "All Events" });
    expect(screen.queryByRole("heading", { name: "Live Now" })).toBeNull();
    expect(await screen.findAllByText("No events this day.")).not.toHaveLength(0);
  });

  it("should show a busy place in Live Now with a Jump In to the web explorer on desktop", async () => {
    live = { events: [], places: [placeRecord({ title: "Meta GamiMall", base_position: "1,95", user_count: 6 })] };
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

  it("should open the app on an Android phone instead of the web explorer", async () => {
    live = { events: [], places: [placeRecord({ base_position: "1,95", user_count: 6 })] };
    setup("/events", { kind: "mobile", os: "android" });
    const section = (await screen.findByRole("heading", { name: "Live Now" })).closest("section")!;
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

  it("should ask a signed-out visitor to sign in from Remind me and write nothing", async () => {
    const { user } = setup("/events");
    await user.click(await screen.findByRole("button", { name: "Remind me: Watch Party" }));
    expect(signIn.openSignIn).toHaveBeenCalledWith({ inPlace: true });
    expect(http.signedGetJSON).not.toHaveBeenCalled();
    expect(http.postJSON).not.toHaveBeenCalled();
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
});
