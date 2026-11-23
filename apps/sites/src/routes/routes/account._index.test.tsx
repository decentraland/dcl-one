// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { createMemoryRouter, RouterProvider, useLocation } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

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

describe("/account index", () => {
  let rendered: ReturnType<typeof at>;

  afterEach(() => {
    cleanup();
    vi.resetAllMocks();
    vi.unstubAllGlobals();
  });

  describe("when it opens on a desktop", () => {
    describe("and the URL carries a query", () => {
      beforeEach(() => {
        rendered = at("/account?utm=mail", true);
      });

      it("should open notifications keeping the query", async () => {
        expect(await screen.findByText("at /account/notifications?utm=mail")).toBeTruthy();
      });

      it("should leave no /account entry behind", async () => {
        await screen.findByText("at /account/notifications?utm=mail");
        expect(rendered.router.state.historyAction).toBe("REPLACE");
      });
    });

    describe("and the location carries where the settings were entered", () => {
      beforeEach(() => {
        rendered = at({ pathname: "/account", state: { accountEntry: 2 } }, true);
      });

      it("should open notifications", async () => {
        expect(await screen.findByText("at /account/notifications")).toBeTruthy();
      });

      it("should keep where the settings were entered", async () => {
        await screen.findByText("at /account/notifications");
        expect(rendered.router.state.location.state).toEqual({ accountEntry: 2 });
      });
    });
  });

  describe("when it opens on a phone", () => {
    beforeEach(() => {
      rendered = at("/account?utm=mail", false);
    });

    it("should query the desktop breakpoint", () => {
      expect(rendered.query).toHaveBeenCalledWith("(min-width: 1024px)");
    });

    it("should stay on the section menu", () => {
      expect(screen.getByText("at /account?utm=mail")).toBeTruthy();
    });
  });
});
