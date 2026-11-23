// @vitest-environment jsdom
import { act, cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState, type ReactNode } from "react";
import { createMemoryRouter, RouterProvider, useLocation } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from "vitest";

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
vi.mock("../components/chrome/SitesLayout", () => ({ default: ({ children }: { children: ReactNode }) => <>{children}</> }));

import { ChromeAuthContext, type ChromeAuth } from "@dcl/one-ui/components/AppChrome/chrome-auth";

import { AccountLayout, type AccountSection } from "./AccountLayout";

type Identity = { signer: string; ephemeral: object; expiration: string; authChain: unknown[] };

let identity: Identity;

function Section({ signer }: { signer: string }) {
  return <p>Settings for {signer}</p>;
}

/** Keeps the signer it mounted with: only a remount shows a new one. */
function FirstSigner({ signer }: { signer: string }) {
  const [first] = useState(signer);
  return <p>First seen {first}</p>;
}

let signedOutChrome: ChromeAuth;

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

beforeEach(() => {
  identity = { signer: "0x4e9c4a2502fdf71e93ed8ed6ca9ddbd891d6f295", ephemeral: {}, expiration: "2999-01-01T00:00:00.000Z", authChain: [] };
  signedOutChrome = { signedIn: false, account: "", mana: "", name: "", committee: false };
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  state.auth = { identity: null, status: "anonymous" };
  vi.resetAllMocks();
});

describe("AccountLayout", () => {
  let user: ReturnType<typeof userEvent.setup>;
  let container: HTMLElement;
  let router: ReturnType<typeof show>["router"];

  beforeEach(() => {
    user = userEvent.setup();
  });

  describe("when the member is signed in", () => {
    beforeEach(() => {
      state.auth = { identity, status: "connected" };
    });

    describe("and the notifications section is open", () => {
      beforeEach(() => {
        show();
      });

      it("should list only the built sections, with their links", () => {
        const links = screen.getAllByRole("link").filter((link) => link.closest("nav"));
        expect(links.map((link) => link.textContent)).toEqual(["Wallets", "Notifications", "Credits Settings"]);
        expect(links.map((link) => link.getAttribute("href"))).toEqual(["/account/wallets", "/account/notifications", "/account/credits"]);
      });

      it("should mark the current section", () => {
        const links = screen.getAllByRole("link").filter((link) => link.closest("nav"));
        expect(links.map((link) => link.getAttribute("aria-current"))).toEqual([null, "page", null]);
      });

      it("should render the section for the signed-in member", () => {
        expect(screen.getByText(`Settings for ${identity.signer}`)).toBeTruthy();
      });
    });

    describe("and the chrome holds the member's profile", () => {
      beforeEach(() => {
        ({ container } = show({
          chrome: { ...signedOutChrome, signedIn: true, account: identity.signer.toUpperCase().replace("0X", "0x"), name: "Jane", avatarUrl: "https://example.com/face.png" },
        }));
      });

      it("should head the menu with the member's profile name and address", () => {
        const nav = screen.getByRole("navigation", { name: "Account Settings" });
        expect(nav.textContent).toContain("Jane");
        expect(nav.textContent).toContain("0x4e9c\u{2026}f295");
      });

      it("should show the member's avatar", () => {
        expect(container.querySelector("nav img")?.getAttribute("src")).toBe("https://example.com/face.png");
      });

      it("should offer to copy the address", () => {
        expect(screen.getByRole("button", { name: "Copy address" })).toBeTruthy();
      });
    });

    describe("and the chrome is still behind on another account's profile", () => {
      beforeEach(() => {
        ({ container } = show({
          chrome: { ...signedOutChrome, signedIn: true, account: "0x1111111111111111111111111111111111111111", name: "Someone else", avatarUrl: "https://example.com/other.png" },
        }));
      });

      it("should not show the other account's name", () => {
        const nav = screen.getByRole("navigation", { name: "Account Settings" });
        expect(nav.textContent).not.toContain("Someone else");
      });

      it("should name the member by address", () => {
        expect(screen.getByText("0x4e9c\u{2026}f295", { selector: "p" })).toBeTruthy();
      });

      it("should not show the other account's avatar", () => {
        expect(container.querySelector("nav img")).toBeNull();
      });
    });

    describe("and the chrome can sign the member out", () => {
      let onSignOut: Mock<() => void>;

      beforeEach(async () => {
        onSignOut = vi.fn();
        show({ chrome: { ...signedOutChrome, signedIn: true, account: identity.signer, onSignOut } });
        await user.click(screen.getByRole("button", { name: "Logout" }));
      });

      it("should log out from the menu", () => {
        expect(onSignOut).toHaveBeenCalledTimes(1);
      });
    });

    describe("and the chrome cannot sign out", () => {
      beforeEach(() => {
        show();
      });

      it("should offer no logout row", () => {
        expect(screen.queryByRole("button", { name: "Logout" })).toBeNull();
      });
    });

    describe("and the chrome's sign-out belongs to another account", () => {
      beforeEach(() => {
        show({ chrome: { ...signedOutChrome, signedIn: true, account: "0x1111111111111111111111111111111111111111", onSignOut: vi.fn() } });
      });

      it("should offer no logout row", () => {
        expect(screen.queryByRole("button", { name: "Logout" })).toBeNull();
      });
    });

    describe("and the page is /account", () => {
      beforeEach(() => {
        ({ container } = show({ active: null, entries: ["/account"] }));
      });

      it("should show the menu alone", () => {
        expect(container.querySelector('[data-view="menu"]')).not.toBeNull();
      });

      it("should show no section header", () => {
        expect(screen.queryByRole("link", { name: /back to Account Settings/ })).toBeNull();
        expect(screen.queryByRole("button", { name: "Close" })).toBeNull();
      });

      it("should mark no section as current", () => {
        for (const link of screen.getAllByRole("link")) expect(link.getAttribute("aria-current")).toBeNull();
      });
    });

    describe("and the credits section is open", () => {
      beforeEach(() => {
        ({ container, router } = show({ active: "credits" }));
      });

      it("should show the section view", () => {
        expect(container.querySelector('[data-view="section"]')).not.toBeNull();
      });

      it("should name the section screen with a level-one heading for the phone header", () => {
        expect(screen.getByRole("heading", { level: 1, name: "Credits Settings, back to Account Settings" })).toBeTruthy();
      });

      describe("and its header's back link is clicked", () => {
        beforeEach(async () => {
          await user.click(screen.getByRole("link", { name: "Credits Settings, back to Account Settings" }));
        });

        it("should go back to the menu", () => {
          expect(router.state.location.pathname).toBe("/account");
        });
      });
    });

    describe("and the settings were entered through the menu from another page", () => {
      beforeEach(async () => {
        vi.spyOn(window.history, "state", "get").mockReturnValue({ idx: 2 });
        ({ router } = show({
          entries: ["/events", "/account", { pathname: "/account/notifications", state: { accountEntry: 1 } }],
        }));
        await user.click(screen.getByRole("button", { name: "Close" }));
      });

      it("should close back past the menu to the page they were entered from", () => {
        expect(router.state.location.pathname).toBe("/events");
      });
    });

    describe("and a section was opened straight from another page", () => {
      beforeEach(async () => {
        vi.spyOn(window.history, "state", "get").mockReturnValue({ idx: 1 });
        ({ router } = show({ entries: ["/events", "/account/notifications"] }));
        await user.click(screen.getByRole("button", { name: "Close" }));
      });

      it("should close back to that page", () => {
        expect(router.state.location.pathname).toBe("/events");
      });
    });

    describe("and the section was the first page", () => {
      beforeEach(async () => {
        vi.spyOn(window.history, "state", "get").mockReturnValue(null);
        ({ router } = show());
        await user.click(screen.getByRole("button", { name: "Close" }));
      });

      it("should close home", () => {
        expect(router.state.location.pathname).toBe("/");
      });
    });

    describe("and the settings carry where they were entered", () => {
      beforeEach(() => {
        vi.spyOn(window.history, "state", "get").mockReturnValue({ idx: 3 });
      });

      describe("and another section is opened from the menu", () => {
        beforeEach(async () => {
          ({ router } = show({ entries: [{ pathname: "/account/notifications", state: { accountEntry: 1 } }] }));
          await user.click(screen.getByRole("link", { name: "Credits Settings" }));
        });

        it("should carry the entry to the next section", () => {
          expect(router.state.location.state).toEqual({ accountEntry: 1 });
        });
      });

      describe("and the carried entry is malformed and the back link is clicked", () => {
        beforeEach(async () => {
          ({ router } = show({ entries: [{ pathname: "/account/notifications", state: { accountEntry: "x" } }] }));
          await user.click(screen.getByRole("link", { name: "Notifications, back to Account Settings" }));
        });

        it("should ignore it and carry the current history index", () => {
          expect(router.state.location.state).toEqual({ accountEntry: 3 });
        });
      });
    });

    describe("and the account switches", () => {
      let switchAccount: () => void;
      let other: Identity;

      beforeEach(() => {
        switchAccount = () => undefined;
        function Host() {
          const [, setTick] = useState(0);
          switchAccount = () => setTick((n) => n + 1);
          return <AccountLayout active="notifications">{(id) => <FirstSigner signer={id.signer} />}</AccountLayout>;
        }
        const hostRouter = createMemoryRouter([{ path: "/", element: <Host /> }]);
        render(<RouterProvider router={hostRouter} />);
      });

      it("should start the section with the first account", () => {
        expect(screen.getByText(`First seen ${identity.signer}`)).toBeTruthy();
      });

      describe("and another account takes over", () => {
        beforeEach(() => {
          other = { ...identity, signer: "0x1111111111111111111111111111111111111111" };
          state.auth = { identity: other, status: "connected" };
          act(() => switchAccount());
        });

        it("should start the section afresh", () => {
          expect(screen.getByText(`First seen ${other.signer}`)).toBeTruthy();
        });
      });
    });
  });

  describe("when the browser has no session", () => {
    beforeEach(() => {
      show();
    });

    it("should ask to sign in once", () => {
      expect(state.openSignIn).toHaveBeenCalledTimes(1);
    });

    it("should not wait forever on a status", () => {
      expect(screen.queryByRole("status")).toBeNull();
    });

    describe("and Sign In is clicked", () => {
      beforeEach(async () => {
        await user.click(screen.getByRole("button", { name: "Sign In" }));
      });

      it("should offer sign-in again", () => {
        expect(state.openSignIn).toHaveBeenCalledTimes(2);
      });

      it("should not render the section or the menu", () => {
        expect(screen.queryByText(/Settings for/)).toBeNull();
        expect(screen.queryByRole("navigation", { name: "Account Settings" })).toBeNull();
      });
    });
  });
});
