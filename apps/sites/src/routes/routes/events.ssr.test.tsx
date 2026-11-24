// @vitest-environment jsdom
import { SitesLocaleContext } from "@dcl/one-ui";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import {
  createMemoryRouter,
  Link,
  RouterContextProvider,
  RouterProvider,
} from "react-router";
import { afterEach, beforeEach, describe, expect, it, type MockInstance, vi } from "vitest";

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

const details = (data: Loaded["data"]) =>
  data.dehydratedState.queries
    .filter((query) => query.queryKey[1] === "detail")
    .map((query) => query.queryKey[2]);

describe("the /events route, rendered on the server and hydrated", () => {
  let client: QueryClient;

  beforeEach(() => {
    vi.mocked(fetchEvents).mockResolvedValue({
      data: [eventRecord({ id: "listed", name: "Listed" })],
      total: 1,
    });
    vi.mocked(fetchMostActivePlaces).mockResolvedValue([]);
    vi.mocked(fetchEvent).mockImplementation(async (id) => eventRecord({ id, name: `Event ${id}` }));
    // Creator faces; never the real network.
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => Response.json([])),
    );
    client = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
    vi.resetAllMocks();
    vi.unstubAllGlobals();
  });

  describe("when two requests interleave", () => {
    let firstData: Loaded["data"];
    let second: Loaded["data"];

    beforeEach(async () => {
      let releaseFirst: () => void = () => {};
      vi.mocked(fetchEvent).mockImplementation(async (id) => {
        if (id === "a")
          await new Promise<void>((resolve) => (releaseFirst = resolve));
        return eventRecord({ id, name: `Event ${id}` });
      });
      const first = load("/events?id=a");
      second = await load("/events?id=b");
      releaseFirst();
      firstData = await first;
    });

    it("should hand the first request only its own event", () => {
      expect(details(firstData)).toEqual(["a"]);
    });

    it("should hand the second request only its own event", () => {
      expect(details(second)).toEqual(["b"]);
    });
  });

  describe("when the first document hydrates", () => {
    beforeEach(async () => {
      const data = await load("/events?id=deep");
      vi.mocked(fetchEvents).mockClear();
      vi.mocked(fetchMostActivePlaces).mockClear();
      vi.mocked(fetchEvent).mockClear();
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
    });

    it("should open the deep-linked event", async () => {
      expect(
        await screen.findByRole("dialog", { name: "Event deep" }),
      ).toBeTruthy();
    });

    it("should seed the browser cache with the calendar", async () => {
      await screen.findByRole("dialog", { name: "Event deep" });
      expect(client.getQueryData(eventsPageKeys.calendar())).toBeDefined();
    });

    it("should render the footer's newsletter controls", async () => {
      await screen.findByRole("dialog", { name: "Event deep" });
      expect(screen.getByRole("button", { name: "Subscribe" })).toBeTruthy();
      expect(screen.getByRole("link", { name: "Privacy Policy" })).toBeTruthy();
    });

    it("should not ask again for what the loader sent", async () => {
      await screen.findByRole("dialog", { name: "Event deep" });
      expect(fetchEvents).not.toHaveBeenCalled();
      expect(fetchMostActivePlaces).not.toHaveBeenCalled();
      expect(fetchEvent).not.toHaveBeenCalled();
    });
  });

  describe("when the shared locale is Spanish", () => {
    let user: ReturnType<typeof userEvent.setup>;
    let footer: ReturnType<typeof within>;

    beforeEach(async () => {
      user = userEvent.setup();
      const data = await load("/events?id=deep");
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

    describe("and the reader subscribes with an empty email", () => {
      beforeEach(async () => {
        vi.mocked(fetch).mockClear();
        await user.click(footer.getByRole("button", { name: "Suscribirse" }));
      });

      it("should show a localized validation error", () => {
        expect(footer.getByRole("alert").textContent).toBe("Ingresá un correo electrónico válido.");
      });

      it("should not send a request", () => {
        expect(vi.mocked(fetch).mock.calls.some(([, options]) => options?.method === "POST")).toBe(false);
      });
    });

    describe.each([
      { status: 201, role: "status", text: "¡Te suscribiste!", remainingEmail: "" },
      { status: 503, role: "alert", text: "No se pudo completar la suscripción. Volvé a intentarlo.", remainingEmail: "person@example.org" },
    ])("and the newsletter answers $status to a subscription", ({ status, role, text, remainingEmail }) => {
      let email: HTMLInputElement;

      beforeEach(async () => {
        vi.mocked(fetch).mockImplementation(async (request) =>
          request === "/v1/newsletter" ? new Response(null, { status }) : Response.json([]),
        );
        email = footer.getByRole("textbox", { name: "Correo electrónico" }) as HTMLInputElement;
        await user.type(email, "person@example.org");
        await user.click(footer.getByRole("button", { name: "Suscribirse" }));
      });

      it("should show localized newsletter feedback", async () => {
        expect((await footer.findByRole(role)).textContent).toBe(text);
      });

      it(`should leave the email field ${remainingEmail ? "filled" : "empty"}`, async () => {
        await footer.findByRole(role);
        expect(email.value).toBe(remainingEmail);
      });
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

  describe("when the reader navigates to /events in the browser", () => {
    let serverLoader: MockInstance<typeof eventsLoader.loadEventsPage>;
    let router: ReturnType<typeof createMemoryRouter>;

    beforeEach(async () => {
      serverLoader = vi.spyOn(eventsLoader, "loadEventsPage");
      router = createMemoryRouter(
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
    });

    it("should render the listing", async () => {
      expect(
        (await screen.findAllByRole("link", { name: "Listed" })).length,
      ).toBeGreaterThan(0);
    });

    it("should hand no loader data and not run the server loader", async () => {
      await screen.findAllByRole("link", { name: "Listed" });
      expect(router.state.loaderData.events).toBeNull();
      expect(serverLoader).not.toHaveBeenCalled();
    });

    it("should fetch through the browser's query cache", async () => {
      await waitFor(() =>
        expect(client.getQueryData(eventsPageKeys.calendar())).toBeDefined(),
      );
      expect(fetchEvents).toHaveBeenCalled();
    });
  });
});
