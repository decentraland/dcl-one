// @vitest-environment jsdom
import { cleanup, render, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { createMemoryRouter, Outlet, RouterProvider } from "react-router";
import { afterEach, expect, it, vi } from "vitest";

vi.hoisted(() =>
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshReg$: () => {},
    $RefreshSig$: () => (type: unknown) => type,
  }),
);

const landing = vi.hoisted(() => vi.fn((_props: { sid?: string; userId?: string }) => null));
vi.mock("@/features/create-landing/CreateLanding", () => ({ default: landing }));
vi.mock("@ui/web/frames/SitesChrome", () => ({ default: ({ children }: { children: ReactNode }) => <>{children}</> }));
vi.mock("@/features/creators/components/SubNav", () => ({ SubNav: () => null }));

import CreateRoute from "./create._index";

afterEach(() => {
  cleanup();
  landing.mockClear();
});

function renderCreate(routeData: { sid?: string } | null) {
  const router = createMemoryRouter(
    [
      {
        id: "root",
        path: "/",
        loader: () => ({ sid: "root-sid", analyticsUserId: "0xabc" }),
        Component: Outlet,
        HydrateFallback: () => null,
        children: [
          {
            path: "create",
            loader: () => routeData,
            Component: () => <CreateRoute loaderData={routeData as never} params={{}} matches={[] as never} />,
          },
        ],
      },
    ],
    { initialEntries: ["/create"] },
  );
  render(
    <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
}

it("hands the hero the root's session after a client navigation, whose clientLoader carries no sid", async () => {
  renderCreate(null);
  await waitFor(() => expect(landing).toHaveBeenCalled());
  expect(landing.mock.lastCall?.[0]).toMatchObject({ sid: "root-sid", userId: "0xabc" });
});

it("keeps the route's own session on a server render", async () => {
  renderCreate({ sid: "route-sid" });
  await waitFor(() => expect(landing).toHaveBeenCalled());
  expect(landing.mock.lastCall?.[0]).toMatchObject({ sid: "route-sid" });
});
