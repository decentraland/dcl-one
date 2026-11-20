// @vitest-environment jsdom
import { SitesLocaleContext } from "@dcl/one-ui/apps/sites/chrome/sites-locale";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import {
  createMemoryRouter,
  Link,
  RouterContextProvider,
  RouterProvider,
} from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() =>
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshReg$: () => {},
    $RefreshSig$: () => (type: unknown) => type,
  }),
);

vi.mock("@/features/components/chrome/SitesTopBar", () => ({
  default: () => null,
}));
vi.mock("@/data/lib/auth/context", () => ({
  useAuth: () => ({ identity: null, isConnected: false }),
}));
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

type Loaded = {
  data: {
    dehydratedState: {
      queries: { queryKey: readonly unknown[]; state: { data: unknown } }[];
    };
  };
};

const load = async (path: string) =>
  (
    (await loadEventsPage(
      new Request(`https://example.org${path}`),
      new RouterContextProvider(),
    )) as unknown as Loaded
  ).data;

beforeEach(() => {
  vi.mocked(fetchEvents)
    .mockReset()
    .mockResolvedValue({
      data: [eventRecord({ id: "listed", name: "Listed" })],
      total: 1,
    });
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
      if (id === "a")
        await new Promise<void>((resolve) => (releaseFirst = resolve));
      return eventRecord({ id, name: `Event ${id}` });
    });
    const first = load("/events?id=a");
    const second = await load("/events?id=b");
    releaseFirst();
    const firstData = await first;
    const details = (data: Loaded["data"]) =>
      data.dehydratedState.queries
        .filter((query) => query.queryKey[1] === "detail")
        .map((query) => query.queryKey[2]);
    expect(details(firstData)).toEqual(["a"]);
    expect(details(second)).toEqual(["b"]);
  });

  it("should hydrate the first document without asking again for what the loader sent", async () => {
    const data = await load("/events?id=deep");
    vi.mocked(fetchEvents).mockClear();
    vi.mocked(fetchMostActivePlaces).mockClear();
    vi.mocked(fetchEvent).mockClear();
    const client = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    const router = createMemoryRouter(
      [{ id: "events", path: "/events", Component: Events as never }],
      {
        initialEntries: ["/events?id=deep"],
        hydrationData: { loaderData: { events: data } },
      },
    );
    render(
      <QueryClientProvider client={client}>
        <RouterProvider router={router} />
      </QueryClientProvider>,
    );
    expect(
      await screen.findByRole("dialog", { name: "Event deep" }),
    ).toBeTruthy();
    expect(client.getQueryData(eventsPageKeys.calendar())).toBeDefined();
    expect(screen.getByRole("button", { name: "Subscribe" })).toBeTruthy();
    expect(screen.getByRole("link", { name: "Privacy Policy" })).toBeTruthy();
    expect(fetchEvents).not.toHaveBeenCalled();
    expect(fetchMostActivePlaces).not.toHaveBeenCalled();
    expect(fetchEvent).not.toHaveBeenCalled();
  });

  describe("and the shared locale is Spanish", () => {
    let user: ReturnType<typeof userEvent.setup>;
    let footer: ReturnType<typeof within>;

    beforeEach(async () => {
      user = userEvent.setup();
      const data = await load("/events?id=deep");
      const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
      const router = createMemoryRouter(
        [{ id: "events", path: "/events", Component: Events as never }],
        { initialEntries: ["/events?id=deep"], hydrationData: { loaderData: { events: data } } },
      );
      render(
        <SitesLocaleContext.Provider value={{ locale: "es", onSelect: vi.fn() }}>
          <QueryClientProvider client={client}>
            <RouterProvider router={router} />
          </QueryClientProvider>
        </SitesLocaleContext.Provider>,
      );
      footer = within(screen.getByRole("contentinfo"));
    });

    it("should translate listing, detail and footer controls while preserving the event name", async () => {
      expect(await screen.findByRole("dialog", { name: "Event deep" })).toBeTruthy();
      expect(screen.getByRole("button", { name: "Cerrar" })).toBeTruthy();
      expect(screen.getByText("Todos los eventos")).toBeTruthy();
      expect(footer.getByRole("button", { name: "Suscribirse" })).toBeTruthy();
      expect(footer.getByRole("textbox", { name: "Correo electrónico" })).toBeTruthy();
      expect(footer.getByRole("link", { name: "Política de privacidad" }).getAttribute("href")).toBe("/privacy");
      expect(footer.getByRole("link", { name: "Términos de uso" }).getAttribute("href")).toBe("/terms");
      expect(footer.getByRole("link", { name: "Política de contenido" }).getAttribute("href")).toBe("/content-policy");
      expect(footer.getByRole("link", { name: "Código de ética" }).getAttribute("href")).toBe("/ethics");
    });

    it("should show a localized validation error for an empty email without sending a request", async () => {
      vi.mocked(fetch).mockClear();
      await user.click(footer.getByRole("button", { name: "Suscribirse" }));
      expect(footer.getByRole("alert").textContent).toBe("Ingresá un correo electrónico válido.");
      expect(vi.mocked(fetch).mock.calls.some(([, options]) => options?.method === "POST")).toBe(false);
    });

    it.each([
      { status: 201, role: "status", text: "¡Te suscribiste!", remainingEmail: "" },
      { status: 503, role: "alert", text: "No se pudo completar la suscripción. Volvé a intentarlo.", remainingEmail: "person@example.org" },
    ])("should show localized newsletter feedback for a $status response", async ({ status, role, text, remainingEmail }) => {
      vi.mocked(fetch).mockImplementation(async (request) =>
        request === "/v1/newsletter" ? new Response(null, { status }) : Response.json([]),
      );
      const email = footer.getByRole("textbox", { name: "Correo electrónico" }) as HTMLInputElement;
      await user.type(email, "person@example.org");
      await user.click(footer.getByRole("button", { name: "Suscribirse" }));
      expect((await footer.findByRole(role)).textContent).toBe(text);
      expect(email.value).toBe(remainingEmail);
    });
  });

  describe("and the public catalyst base differs from the page origin", () => {
    const PAGE = "http://127.0.0.1:5321";
    const CATALYST = "https://catalyst.example.org";
    const POSTER = "/events-assets-099ac00/poster";

    beforeEach(() => {
      Object.assign(window, { __CATALYST_BASE__: CATALYST });
      vi.mocked(fetchEvents).mockResolvedValue({
        data: [
          eventRecord({ id: "local", name: "Local cover", image: `${PAGE}${POSTER}/local.png` }),
          eventRecord({ id: "catalyst", name: "Catalyst cover", image: `${CATALYST}${POSTER}/catalyst.png` }),
        ],
        total: 2,
      });
    });

    afterEach(() => {
      delete (window as Window & { __CATALYST_BASE__?: string }).__CATALYST_BASE__;
    });

    it("should hand over the request's origin as the page origin, apart from the share origin", async () => {
      const data = (await loadEventsPage(new Request(`${PAGE}/events`), new RouterContextProvider())) as unknown as {
        data: { origin: string; pageOrigin: string };
      };
      expect(data.data.origin).toBe(CATALYST);
      expect(data.data.pageOrigin).toBe(PAGE);
    });

    it("should render an http cover on the page origin as its path and leave an https cover under the catalyst base unchanged", async () => {
      const data = (await loadEventsPage(new Request(`${PAGE}/events`), new RouterContextProvider())) as unknown as Loaded;
      const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
      const router = createMemoryRouter(
        [{ id: "events", path: "/events", Component: Events as never }],
        { initialEntries: ["/events"], hydrationData: { loaderData: { events: data.data } } },
      );
      render(
        <QueryClientProvider client={client}>
          <RouterProvider router={router} />
        </QueryClientProvider>,
      );
      const covers = async (name: string) =>
        (await screen.findAllByRole("link", { name })).map((link) => link.closest("li, article")?.querySelector("img")?.getAttribute("src"));
      expect(new Set(await covers("Local cover"))).toEqual(new Set([`${POSTER}/local.png`]));
      expect(new Set(await covers("Catalyst cover"))).toEqual(new Set([`${CATALYST}${POSTER}/catalyst.png`]));
    });
  });

  it("should fetch through the browser's query cache on a client navigation, without the server loader", async () => {
    const serverLoader = vi.spyOn(eventsLoader, "loadEventsPage");
    const client = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
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
    expect(
      (await screen.findAllByRole("link", { name: "Listed" })).length,
    ).toBeGreaterThan(0);
    expect(router.state.loaderData.events).toBeNull();
    expect(serverLoader).not.toHaveBeenCalled();
    await waitFor(() =>
      expect(client.getQueryData(eventsPageKeys.calendar())).toBeDefined(),
    );
    expect(fetchEvents).toHaveBeenCalled();
  });
});
