// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { createMemoryRouter, Link, RouterProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() =>
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshReg$: () => {},
    $RefreshSig$: () => (type: unknown) => type,
  }),
);

vi.mock("@ui/web/frames/SitesChrome", () => ({ default: ({ children }: { children: ReactNode }) => <>{children}</> }));
vi.mock("@/data/lib/auth/context", () => ({ useAuth: () => ({ identity: null, isConnected: false }) }));
vi.mock("@/core/lib/telemetry/track", () => ({ track: vi.fn() }));
vi.mock("@/data/lib/catalyst/places/events", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/places/events")>()),
  fetchEvents: vi.fn(),
  fetchEvent: vi.fn(),
}));
vi.mock("@/data/lib/catalyst/places/index", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/places/index")>()),
  fetchMostActivePlaces: vi.fn(),
}));

import { fetchEvent, fetchEvents } from "@/data/lib/catalyst/places/events";
import { fetchMostActivePlaces } from "@/data/lib/catalyst/places/index";
import { eventsPageKeys } from "@/data/lib/catalyst/places/events-page";
import { eventRecord } from "@/features/events/fixtures";
import * as eventsLoader from "../lib/events-loader";
import { loadEventsPage } from "../lib/events-loader";
import Events, { clientLoader } from "./events";

type Loaded = { data: { dehydratedState: { queries: { queryKey: readonly unknown[]; state: { data: unknown } }[] } } };

const load = async (path: string) =>
  ((await loadEventsPage(new Request(`https://example.org${path}`))) as unknown as Loaded).data;

beforeEach(() => {
  vi.mocked(fetchEvents)
    .mockReset()
    .mockResolvedValue({ data: [eventRecord({ id: "listed", name: "Listed" })], total: 1 });
  vi.mocked(fetchMostActivePlaces).mockReset().mockResolvedValue([]);
  vi.mocked(fetchEvent)
    .mockReset()
    .mockImplementation(async (id) => eventRecord({ id, name: `Event ${id}` }));
  // Creator faces; never the real network.
  vi.stubGlobal(
    "fetch",
    vi.fn(async () => Response.json([])),
  );
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

describe("the /events route, rendered on the server and hydrated", () => {
  it("should keep two interleaved requests apart: each hands over only its own event", async () => {
    let releaseFirst: () => void = () => {};
    vi.mocked(fetchEvent).mockImplementation(async (id) => {
      if (id === "a") await new Promise<void>((resolve) => (releaseFirst = resolve));
      return eventRecord({ id, name: `Event ${id}` });
    });
    const first = load("/events?id=a");
    const second = await load("/events?id=b");
    releaseFirst();
    const firstData = await first;
    const details = (data: Loaded["data"]) =>
      data.dehydratedState.queries.filter((query) => query.queryKey[1] === "detail").map((query) => query.queryKey[2]);
    expect(details(firstData)).toEqual(["a"]);
    expect(details(second)).toEqual(["b"]);
  });

  it("should hydrate the first document without asking again for what the loader sent", async () => {
    const data = await load("/events?id=deep");
    vi.mocked(fetchEvents).mockClear();
    vi.mocked(fetchMostActivePlaces).mockClear();
    vi.mocked(fetchEvent).mockClear();
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const router = createMemoryRouter([{ id: "events", path: "/events", Component: Events as never }], {
      initialEntries: ["/events?id=deep"],
      hydrationData: { loaderData: { events: data } },
    });
    render(
      <QueryClientProvider client={client}>
        <RouterProvider router={router} />
      </QueryClientProvider>,
    );
    expect(await screen.findByRole("dialog", { name: "Event deep" })).toBeTruthy();
    expect(client.getQueryData(eventsPageKeys.calendar())).toBeDefined();
    expect(fetchEvents).not.toHaveBeenCalled();
    expect(fetchMostActivePlaces).not.toHaveBeenCalled();
    expect(fetchEvent).not.toHaveBeenCalled();
  });

  it("should fetch through the browser's query cache on a client navigation, without the server loader", async () => {
    const serverLoader = vi.spyOn(eventsLoader, "loadEventsPage");
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const router = createMemoryRouter(
      [
        { path: "/", element: <Link to="/events">Events</Link> },
        {
          id: "events",
          path: "/events",
          // The route module's own clientLoader, which never calls the server loader.
          loader: () => clientLoader(),
          Component: Events as never,
        },
      ],
      { initialEntries: ["/"] },
    );
    render(
      <QueryClientProvider client={client}>
        <RouterProvider router={router} />
      </QueryClientProvider>,
    );
    screen.getByRole("link", { name: "Events" }).click();
    expect((await screen.findAllByRole("link", { name: "Listed" })).length).toBeGreaterThan(0);
    expect(router.state.loaderData.events).toBeNull();
    expect(serverLoader).not.toHaveBeenCalled();
    await waitFor(() => expect(client.getQueryData(eventsPageKeys.calendar())).toBeDefined());
    expect(fetchEvents).toHaveBeenCalled();
  });
});
