// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import HelpRoute from "./help";

vi.hoisted(() =>
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshReg$: () => {},
    $RefreshSig$: () => (type: unknown) => type,
  }),
);
vi.mock("@ui/web/frames/SitesChrome", () => ({ default: ({ children }: { children: ReactNode }) => <>{children}</> }));

beforeEach(() => {
  vi.stubGlobal(
    "fetch",
    vi.fn(async () => new Response(null, { status: 200 })),
  );
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  for (const script of document.querySelectorAll("script[data-intercom]")) script.remove();
});

describe("/help route", () => {
  it("renders the help center with its tabs, the FAQ and the chat banner", async () => {
    const router = createMemoryRouter([{ id: "help", path: "/help", Component: HelpRoute as never }], {
      initialEntries: ["/help"],
      hydrationData: {
        loaderData: {
          help: {
            services: [{ name: "Node", url: "https://node.example/about" }],
            statusPageUrl: "https://status.example/",
            intercomAppId: "app-test",
            sid: "sid-example",
            wallet: null,
          },
        },
      },
    });
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={client}>
        <RouterProvider router={router} />
      </QueryClientProvider>,
    );
    expect(screen.getByRole("heading", { level: 1, name: "Help Center" })).toBeTruthy();
    expect(screen.getAllByRole("tab").map((tab) => tab.textContent)).toEqual(["FAQ", "CREATOR FUND", "SUPPORT UPDATES"]);
    expect(screen.getByRole("heading", { name: "Frequently Asked Questions" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "CHAT NOW" })).toBeTruthy();
    expect(await screen.findByRole("button", { name: "System Status" })).toBeTruthy();
  });
});
