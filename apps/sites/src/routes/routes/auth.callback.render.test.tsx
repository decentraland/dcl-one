// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

import AuthCallbackRoute from "./auth.callback";

vi.hoisted(() => {
  Object.assign(window, { __vite_plugin_react_preamble_installed__: true });
});
vi.mock("@/features/auth/login/MagicCallback", () => ({ default: () => <p>magic callback</p> }));
vi.mock("@/data/lib/auth/context", () => ({ useAuth: () => ({ completeInAppSession: vi.fn(), error: null }) }));

function at(search: string) {
  const router = createMemoryRouter([{ id: "callback", path: "/auth/callback", Component: AuthCallbackRoute as never }], {
    initialEntries: [`/auth/callback${search}`],
    hydrationData: { loaderData: { callback: { magicApiKey: "pk_example123" } } },
  });
  render(<RouterProvider router={router} />);
}

afterEach(cleanup);

describe("the sign-in callback", () => {
  it("hands Magic's return, which carries only its state, to Magic", () => {
    at("?state=abc&magic_oauth_request_id=1");
    expect(screen.getByText("magic callback")).toBeTruthy();
  });

  it("keeps an in-app return that came back without a result on its own message", () => {
    at("?next=%2Fshop");
    expect(screen.queryByText("magic callback")).toBeNull();
    expect(screen.getByText("Sign-in didn't return a result. Please try again.")).toBeTruthy();
  });
});
