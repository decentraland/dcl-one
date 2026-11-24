// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, render, screen } from "@testing-library/react";
import { renderToString } from "react-dom/server";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

import { useEventsCopy } from "@dcl/one-ui/apps/sites/events/locale";
import { SitesLocaleContext, type SitesLocale } from "@dcl/one-ui/apps/sites/chrome/sites-locale";

import CommunityDetailRoute, { shouldRevalidate } from "./social_.communities.$id";

vi.hoisted(() => {
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshSig$: () => (type: unknown) => type,
    $RefreshReg$: () => {},
  });
});

vi.mock("@/pages/social/CommunityPage", () => ({
  CommunityPage: ({ sid }: { sid: string }) => {
    const { text } = useEventsCopy();
    return (
      <p>
        {text("Jump In")} | session {sid || "none"}
      </p>
    );
  },
}));

const ID = "e99471aa-31c4-4952-abf6-99905445f43b";

afterEach(cleanup);

function renderRoute(locale: SitesLocale, loaderData: unknown) {
  const router = createMemoryRouter(
    [{ id: "community", path: "/social/communities/:id", Component: CommunityDetailRoute as never }],
    { initialEntries: [`/social/communities/${ID}`], hydrationData: { loaderData: { community: loaderData } } },
  );
  return renderToString(
    <SitesLocaleContext.Provider value={{ locale, onSelect: () => {} }}>
      <QueryClientProvider client={new QueryClient()}>
        <RouterProvider router={router} />
      </QueryClientProvider>
    </SitesLocaleContext.Provider>,
  );
}

const loaded = { sid: "sid-123", id: ID, now: 0, title: "", description: "", ownerName: null, image: null, dehydratedState: { queries: [], mutations: [] } };

describe("when the community route renders", () => {
  describe("and the visitor reads Spanish", () => {
    it("should give the event detail's copy the visitor's language", () => {
      expect(renderRoute("es", loaded)).toContain("Entrar");
    });
  });

  describe("and the visitor reads English", () => {
    it("should keep the event detail's copy in English", () => {
      expect(renderRoute("en", loaded)).toContain("Jump In");
    });
  });

  describe("and the first document carries a session", () => {
    it("should hand the page that session", () => {
      expect(renderRoute("en", loaded)).toContain("session <!-- -->sid-123");
    });
  });

  describe("and a client navigation brought no loader data", () => {
    it("should hand the page no session", () => {
      expect(renderRoute("en", null)).toContain("session <!-- -->none");
    });
  });
});

describe("when the visitor moves in the app from one community to another", () => {
  it("should hand the next community no session, as any client navigation", async () => {
    const router = createMemoryRouter(
      [{ id: "community", path: "/social/communities/:id", Component: CommunityDetailRoute as never }],
      { initialEntries: [`/social/communities/${ID}`], hydrationData: { loaderData: { community: loaded } } },
    );
    render(
      <QueryClientProvider client={new QueryClient()}>
        <RouterProvider router={router} />
      </QueryClientProvider>,
    );
    expect(screen.getByText(/session sid-123/)).toBeTruthy();

    await act(() => router.navigate("/social/communities/another"));

    expect(screen.getByText(/session none/)).toBeTruthy();
  });
});

describe("when the community route decides whether to read again", () => {
  const args = (from: string, to: string) =>
    ({
      currentUrl: new URL(from, "https://dcl.test"),
      nextUrl: new URL(to, "https://dcl.test"),
      defaultShouldRevalidate: true,
    }) as Parameters<typeof shouldRevalidate>[0];

  describe("and only the query changes, as opening an event's detail does", () => {
    it("should not read again", () => {
      expect(shouldRevalidate(args(`/social/communities/${ID}`, `/social/communities/${ID}?event=ev-1`))).toBe(false);
    });
  });

  describe("and a fetcher elsewhere submits while the page shows", () => {
    it("should not read again, keeping the page's title", () => {
      expect(
        shouldRevalidate({
          ...args(`/social/communities/${ID}`, `/social/communities/${ID}`),
          formMethod: "POST",
          formAction: "/api/something",
        }),
      ).toBe(false);
    });
  });

  describe("and the visitor moves to another community", () => {
    it("should read again", () => {
      expect(shouldRevalidate(args(`/social/communities/${ID}`, "/social/communities/other"))).toBe(true);
    });
  });
});
