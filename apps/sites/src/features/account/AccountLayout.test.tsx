// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() => {
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshSig$: () => (type: unknown) => type,
    $RefreshReg$: () => {},
  });
});
const state = vi.hoisted(() => ({ auth: { identity: null as unknown, status: "anonymous" }, openSignIn: vi.fn() }));
vi.mock("@/data/lib/auth/index", () => ({ useAuth: () => state.auth }));
vi.mock("../components/auth/signin-store", () => ({ openSignIn: state.openSignIn }));
vi.mock("@ui/web/frames/SitesChrome", () => ({ default: ({ children }: { children: ReactNode }) => <>{children}</> }));

import { AccountLayout } from "./AccountLayout";

const identity = { signer: "0x4e9c4a2502fdf71e93ed8ed6ca9ddbd891d6f295", ephemeral: {}, expiration: "2999-01-01T00:00:00.000Z", authChain: [] };

function show() {
  const router = createMemoryRouter(
    [
      {
        path: "/account/notifications",
        element: <AccountLayout active="notifications">{(id) => <p>Settings for {id.signer}</p>}</AccountLayout>,
      },
    ],
    { initialEntries: ["/account/notifications"] },
  );
  return render(<RouterProvider router={router} />);
}

afterEach(() => {
  cleanup();
  state.auth = { identity: null, status: "anonymous" };
  vi.resetAllMocks();
});

describe("AccountLayout", () => {
  it("lists only the built sections, marking the current one", () => {
    state.auth = { identity, status: "connected" };
    show();

    const links = screen.getAllByRole("link").filter((link) => link.closest("nav"));
    expect(links.map((link) => link.textContent)).toEqual(["Notifications", "Credits Settings"]);
    expect(links[0]!.getAttribute("aria-current")).toBe("page");
    expect(screen.getByText(`Settings for ${identity.signer}`)).toBeTruthy();
  });

  it("offers sign-in again when the browser has no session, instead of waiting forever", async () => {
    const user = userEvent.setup();
    show();

    expect(state.openSignIn).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("status")).toBeNull();
    await user.click(screen.getByRole("button", { name: "Sign In" }));
    expect(state.openSignIn).toHaveBeenCalledTimes(2);
    expect(screen.queryByText(/Settings for/)).toBeNull();
  });
});
