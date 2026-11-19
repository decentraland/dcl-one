// @vitest-environment jsdom
import { act, cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState, type ReactNode } from "react";
import { createMemoryRouter, RouterProvider, useLocation } from "react-router";
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

import { ChromeAuthContext, type ChromeAuth } from "@ui/web/frames/chrome-auth";

import { AccountLayout, type AccountSection } from "./AccountLayout";

const identity = { signer: "0x4e9c4a2502fdf71e93ed8ed6ca9ddbd891d6f295", ephemeral: {}, expiration: "2999-01-01T00:00:00.000Z", authChain: [] };

function Section({ signer }: { signer: string }) {
  return <p>Settings for {signer}</p>;
}

/** Keeps the signer it mounted with: only a remount shows a new one. */
function FirstSigner({ signer }: { signer: string }) {
  const [first] = useState(signer);
  return <p>First seen {first}</p>;
}

const signedOutChrome: ChromeAuth = { signedIn: false, account: "", mana: "", name: "", committee: false };

function Where() {
  return <p>at {useLocation().pathname}</p>;
}

function show({
  active = "notifications",
  chrome = signedOutChrome,
  entries = [`/account/${active ?? ""}`],
}: {
  active?: AccountSection | null;
  chrome?: ChromeAuth;
  entries?: (string | { pathname: string; state: unknown })[];
} = {}) {
  const router = createMemoryRouter(
    [
      {
        path: "/account/:section?",
        element: (
          <ChromeAuthContext.Provider value={chrome}>
            <AccountLayout active={active}>{(id) => <Section signer={id.signer} />}</AccountLayout>
          </ChromeAuthContext.Provider>
        ),
      },
      { path: "*", element: <Where /> },
    ],
    { initialEntries: entries, initialIndex: entries.length - 1 },
  );
  return { router, ...render(<RouterProvider router={router} />) };
}

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  state.auth = { identity: null, status: "anonymous" };
  vi.resetAllMocks();
});

describe("AccountLayout", () => {
  it("lists only the built sections, marking the current one", () => {
    state.auth = { identity, status: "connected" };
    show();

    const links = screen.getAllByRole("link").filter((link) => link.closest("nav"));
    expect(links.map((link) => link.textContent)).toEqual(["Wallets", "Notifications", "Credits Settings"]);
    expect(links.map((link) => link.getAttribute("href"))).toEqual(["/account/wallets", "/account/notifications", "/account/credits"]);
    expect(links.map((link) => link.getAttribute("aria-current"))).toEqual([null, "page", null]);
    expect(screen.getByText(`Settings for ${identity.signer}`)).toBeTruthy();
  });

  it("heads the menu with the member's profile name, avatar and address", () => {
    state.auth = { identity, status: "connected" };
    const { container } = show({
      chrome: { ...signedOutChrome, signedIn: true, account: identity.signer.toUpperCase().replace("0X", "0x"), name: "Jane", avatarUrl: "https://example.com/face.png" },
    });

    const nav = screen.getByRole("navigation", { name: "Account Settings" });
    expect(nav.textContent).toContain("Jane");
    expect(nav.textContent).toContain("0x4e9c\u{2026}f295");
    expect(container.querySelector("nav img")?.getAttribute("src")).toBe("https://example.com/face.png");
    expect(screen.getByRole("button", { name: "Copy address" })).toBeTruthy();
  });

  it("names the member by address, without another account's profile, while the chrome is behind", () => {
    state.auth = { identity, status: "connected" };
    const { container } = show({
      chrome: { ...signedOutChrome, signedIn: true, account: "0x1111111111111111111111111111111111111111", name: "Someone else", avatarUrl: "https://example.com/other.png" },
    });

    const nav = screen.getByRole("navigation", { name: "Account Settings" });
    expect(nav.textContent).not.toContain("Someone else");
    expect(screen.getByText("0x4e9c\u{2026}f295", { selector: "p" })).toBeTruthy();
    expect(container.querySelector("nav img")).toBeNull();
  });

  it("logs out from the menu when the chrome can, and offers no dead row when it cannot", async () => {
    const user = userEvent.setup();
    state.auth = { identity, status: "connected" };
    const onSignOut = vi.fn();
    show({ chrome: { ...signedOutChrome, signedIn: true, account: identity.signer, onSignOut } });

    await user.click(screen.getByRole("button", { name: "Log out" }));
    expect(onSignOut).toHaveBeenCalledTimes(1);

    cleanup();
    show();
    expect(screen.queryByRole("button", { name: "Log out" })).toBeNull();

    cleanup();
    show({ chrome: { ...signedOutChrome, signedIn: true, account: "0x1111111111111111111111111111111111111111", onSignOut } });
    expect(screen.queryByRole("button", { name: "Log out" })).toBeNull();
  });

  it("shows the menu alone on /account, with no section header", () => {
    state.auth = { identity, status: "connected" };
    const { container } = show({ active: null, entries: ["/account"] });

    expect(container.querySelector('[data-view="menu"]')).not.toBeNull();
    expect(screen.queryByRole("link", { name: /back to Account Settings/ })).toBeNull();
    expect(screen.queryByRole("button", { name: "Close" })).toBeNull();
    for (const link of screen.getAllByRole("link")) expect(link.getAttribute("aria-current")).toBeNull();
  });

  it("goes back to the menu from a section's header", async () => {
    const user = userEvent.setup();
    state.auth = { identity, status: "connected" };
    const { container, router } = show({ active: "credits" });

    expect(container.querySelector('[data-view="section"]')).not.toBeNull();
    await user.click(screen.getByRole("link", { name: "Credits Settings, back to Account Settings" }));
    expect(router.state.location.pathname).toBe("/account");
  });

  it("closes the settings back past the menu to the page they were entered from", async () => {
    const user = userEvent.setup();
    state.auth = { identity, status: "connected" };
    vi.spyOn(window.history, "state", "get").mockReturnValue({ idx: 2 });
    const { router } = show({
      entries: ["/events", "/account", { pathname: "/account/notifications", state: { accountEntry: 1 } }],
    });

    await user.click(screen.getByRole("button", { name: "Close" }));
    expect(router.state.location.pathname).toBe("/events");
  });

  it("closes a section opened straight from another page back to it, or home when it was the first page", async () => {
    const user = userEvent.setup();
    state.auth = { identity, status: "connected" };
    vi.spyOn(window.history, "state", "get").mockReturnValue({ idx: 1 });
    const { router } = show({ entries: ["/events", "/account/notifications"] });

    await user.click(screen.getByRole("button", { name: "Close" }));
    expect(router.state.location.pathname).toBe("/events");

    cleanup();
    vi.spyOn(window.history, "state", "get").mockReturnValue(null);
    const first = show();
    await user.click(screen.getByRole("button", { name: "Close" }));
    expect(first.router.state.location.pathname).toBe("/");
  });

  it("carries where the settings were entered through the menu and the back link, ignoring a malformed one", async () => {
    const user = userEvent.setup();
    state.auth = { identity, status: "connected" };
    vi.spyOn(window.history, "state", "get").mockReturnValue({ idx: 3 });
    const { router } = show({ entries: [{ pathname: "/account/notifications", state: { accountEntry: 1 } }] });

    await user.click(screen.getByRole("link", { name: "Credits Settings" }));
    expect(router.state.location.state).toEqual({ accountEntry: 1 });

    cleanup();
    const fresh = show({ entries: [{ pathname: "/account/notifications", state: { accountEntry: "x" } }] });
    await user.click(screen.getByRole("link", { name: "Notifications, back to Account Settings" }));
    expect(fresh.router.state.location.state).toEqual({ accountEntry: 3 });
  });

  it("names a section screen with a level-one heading for the phone header", () => {
    state.auth = { identity, status: "connected" };
    show({ active: "credits" });

    expect(screen.getByRole("heading", { level: 1, name: "Credits Settings, back to Account Settings" })).toBeTruthy();
  });

  it("starts the section afresh when the account switches", async () => {
    state.auth = { identity, status: "connected" };
    let switchAccount!: () => void;
    function Host() {
      const [, setTick] = useState(0);
      switchAccount = () => setTick((n) => n + 1);
      return <AccountLayout active="notifications">{(id) => <FirstSigner signer={id.signer} />}</AccountLayout>;
    }
    const router = createMemoryRouter([{ path: "/", element: <Host /> }]);
    render(<RouterProvider router={router} />);
    expect(screen.getByText(`First seen ${identity.signer}`)).toBeTruthy();

    const other = { ...identity, signer: "0x1111111111111111111111111111111111111111" };
    state.auth = { identity: other, status: "connected" };
    act(() => switchAccount());

    expect(screen.getByText(`First seen ${other.signer}`)).toBeTruthy();
  });

  it("offers sign-in again when the browser has no session, instead of waiting forever", async () => {
    const user = userEvent.setup();
    show();

    expect(state.openSignIn).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("status")).toBeNull();
    await user.click(screen.getByRole("button", { name: "Sign In" }));
    expect(state.openSignIn).toHaveBeenCalledTimes(2);
    expect(screen.queryByText(/Settings for/)).toBeNull();
    expect(screen.queryByRole("navigation", { name: "Account Settings" })).toBeNull();
  });
});
