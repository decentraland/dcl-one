// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { createMemoryRouter, RouterProvider, useLocation } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() => {
  Object.assign(window, { __vite_plugin_react_preamble_installed__: true });
});

import AccountIndex from "./account._index";

function Where() {
  const { pathname, search } = useLocation();
  return <p>at {pathname + search}</p>;
}

function at(url: string | { pathname: string; search?: string; state: unknown }, desktop: boolean) {
  const query = vi.fn((media: string) => ({ matches: desktop && media === "(min-width: 1024px)", media }));
  vi.stubGlobal("matchMedia", query);
  const router = createMemoryRouter(
    [
      {
        path: "/account",
        element: (
          <>
            <Where />
            <AccountIndex />
          </>
        ),
      },
      { path: "/account/notifications", element: <Where /> },
    ],
    { initialEntries: [url] },
  );
  render(<RouterProvider router={router} />);
  return { router, query };
}

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

describe("/account index", () => {
  it("opens notifications on a desktop, keeping the query and leaving no /account entry behind", async () => {
    const { router } = at("/account?utm=mail", true);

    expect(await screen.findByText("at /account/notifications?utm=mail")).toBeTruthy();
    expect(router.state.historyAction).toBe("REPLACE");
  });

  it("keeps where the settings were entered when it opens notifications", async () => {
    const { router } = at({ pathname: "/account", state: { accountEntry: 2 } }, true);

    expect(await screen.findByText("at /account/notifications")).toBeTruthy();
    expect(router.state.location.state).toEqual({ accountEntry: 2 });
  });

  it("stays on the section menu on a phone", () => {
    const { query } = at("/account?utm=mail", false);

    expect(query).toHaveBeenCalledWith("(min-width: 1024px)");
    expect(screen.getByText("at /account?utm=mail")).toBeTruthy();
  });
});
