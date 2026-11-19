// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import DclTopBar from "@ui/web/frames/DclTopBar";
import SiteLayout from "@ui/web/frames/SiteLayout";
import { ChromeAuthContext, type ChromeAuth } from "@ui/web/frames/chrome-auth";
import { ChromeNavContext } from "@ui/web/frames/chrome-nav";
import ChromeNavBridge from "./ChromeNavBridge";

vi.hoisted(() => {
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshSig$: () => (type: unknown) => type,
    $RefreshReg$: () => {},
  });
});

const marketplace = vi.hoisted(() => ({ eligible: false }));
vi.mock("@/data/lib/shop/marketplace-access", () => ({
  useMarketplaceHere: () => marketplace.eligible,
}));

const baseAuth: ChromeAuth = {
  signedIn: false,
  account: "",
  mana: "",
  name: "",
  committee: false,
};
function Host({
  auth = baseAuth,
  children = <DclTopBar active="shop" />,
  pathname = "/shop",
}: {
  auth?: ChromeAuth;
  children?: React.ReactNode;
  pathname?: string;
}) {
  const [client] = React.useState(
    () => new QueryClient({ defaultOptions: { queries: { retry: false } } }),
  );
  return (
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={[pathname]}>
        <ChromeAuthContext.Provider value={auth}>
          <ChromeNavBridge>{children}</ChromeNavBridge>
        </ChromeAuthContext.Provider>
      </MemoryRouter>
    </QueryClientProvider>
  );
}
import * as React from "react";
beforeEach(() => {
  window.matchMedia = vi.fn().mockImplementation((query) => ({
    matches: false,
    media: query,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  }));
  HTMLDialogElement.prototype.showModal = function () {
    this.open = true;
  };
  HTMLDialogElement.prototype.close = function () {
    this.open = false;
  };
});
afterEach(() => {
  cleanup();
  marketplace.eligible = false;
  vi.unstubAllGlobals();
});

const shopAccount = "0x" + "ab".repeat(20);

describe("Sites native chrome integration", () => {
  it("renders one banner and keeps signed-out links and dropdown destinations", () => {
    render(<Host />);
    expect(screen.getAllByRole("banner")).toHaveLength(1);
    fireEvent.click(screen.getByRole("button", { name: "Shop" }));
    expect(screen.getByRole("link", { name: "Shop All" }).getAttribute("href")).toBe("/shop/overview");
    expect(screen.queryByRole("link", { name: "LAND" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Discover" }));
    expect(
      screen.getByRole("link", { name: "Events" }).getAttribute("href"),
    ).toBe("/events");
    expect(
      screen.getByRole("link", { name: "Places" }).getAttribute("href"),
    ).toBe("/places");
    expect(screen.getByRole("link", { name: "Sign in" })).toBeTruthy();
    expect(screen.queryByLabelText("Credits")).toBeNull();
  });
  it("selects the Shop item from the router query and shows LAND after the wallet matches", async () => {
    marketplace.eligible = true;
    let resolveAccounts!: (accounts: string[]) => void;
    vi.stubGlobal("ethereum", {
      request: () => new Promise<string[]>((resolve) => { resolveAccounts = resolve; }),
    });
    render(
      <Host
        pathname="/shop/items?category=emote"
        auth={{ ...baseAuth, signedIn: true, account: shopAccount }}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Shop" }));
    expect(screen.getByRole("link", { name: "Emotes" }).getAttribute("aria-current")).toBe("page");
    expect(screen.getByRole("link", { name: /names/i }).getAttribute("href")).toBe("/shop/names");
    expect(screen.queryByRole("link", { name: "LAND" })).toBeNull();
    await act(async () => resolveAccounts([shopAccount]));
    expect((await screen.findByRole("link", { name: "LAND" })).getAttribute("href")).toBe("/shop/lands");

    fireEvent.click(screen.getByRole("button", { name: "Open menu" }));
    const menu = within(screen.getByRole("dialog", { name: "Menu" }));
    fireEvent.click(menu.getByRole("button", { name: "Shop" }));
    expect(menu.getByRole("link", { name: "LAND" }).getAttribute("href")).toBe("/shop/lands");
    expect(menu.getByRole("link", { name: "Emotes" }).getAttribute("aria-current")).toBe("page");
  });
  it("keeps LAND hidden for a different wallet, an empty list, or a rejected wallet read", async () => {
    marketplace.eligible = true;
    for (const accounts of [["0x" + "cd".repeat(20)], [], null]) {
      const request = vi.fn(async () => {
        if (accounts === null) throw new Error("Wallet read denied");
        return accounts;
      });
      vi.stubGlobal("ethereum", { request });
      const view = render(<Host auth={{ ...baseAuth, signedIn: true, account: shopAccount }} />);
      await act(async () => {});
      expect(request).toHaveBeenCalledWith({ method: "eth_accounts" });
      fireEvent.click(screen.getByRole("button", { name: "Shop" }));
      expect(screen.getByRole("link", { name: /names/i })).toBeTruthy();
      expect(screen.queryByRole("link", { name: "LAND" })).toBeNull();
      view.unmount();
    }
  });
  it("shows the canonical Events destination as selected inside the mobile menu", () => {
    render(
      <Host pathname="/events" children={<DclTopBar active="events" />} />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Open menu" }));
    const menu = within(screen.getByRole("dialog", { name: "Menu" }));
    fireEvent.click(menu.getByRole("button", { name: "Discover" }));
    const events = menu.getByRole("link", { name: "Events" });
    expect(events.getAttribute("href")).toBe("/events");
    expect(events.getAttribute("aria-current")).toBe("page");
    expect(menu.queryByRole("link", { name: "What's on" })).toBeNull();
  });
  it.each([false, true])(
    "keeps Places accessible in the mobile menu with signedIn=%s",
    (signedIn) => {
      render(
        <Host
          pathname="/places"
          auth={{ ...baseAuth, signedIn, account: signedIn ? "0xa" : "" }}
          children={<DclTopBar active="explore" />}
        />,
      );
      fireEvent.click(screen.getByRole("button", { name: "Open menu" }));
      const menu = within(screen.getByRole("dialog", { name: "Menu" }));
      fireEvent.click(menu.getByRole("button", { name: "Discover" }));
      const places = menu.getByRole("link", { name: "Places" });
      expect(places.getAttribute("href")).toBe("/places");
      expect(places.getAttribute("aria-current")).toBe("page");
      expect(
        menu.getByRole("link", { name: "Events" }).getAttribute("aria-current"),
      ).toBeNull();
    },
  );
  it("forwards account and balance slots once and closes mobile callback tabs", async () => {
    const tab = vi.fn();
    render(
      <Host
        children={
          <SiteLayout
            sections={(close) => (
              <button
                onClick={() => {
                  tab();
                  close();
                }}
              >
                Photos
              </button>
            )}
            navigation={
              <DclTopBar
                balances={<a href="/shop/credits">Real credits</a>}
                accountControl={<button>Host account</button>}
              />
            }
          >
            Body
          </SiteLayout>
        }
        auth={{ ...baseAuth, signedIn: true, account: "0xA" }}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Open menu" }));
    expect(screen.getAllByText("Real credits")).toHaveLength(1);
    expect(
      screen.getAllByRole("button", { name: "Host account" }),
    ).toHaveLength(1);
    fireEvent.click(
      within(screen.getByRole("dialog", { name: "Menu" })).getByRole("button", {
        name: "Photos",
      }),
    );
    expect(tab).toHaveBeenCalledOnce();
    await waitFor(() =>
      expect(screen.queryByRole("dialog", { name: "Menu" })).toBeNull(),
    );
  });
  it("does not spontaneously reopen account A notifications after A to B to A", async () => {
    const fetchNotifications = vi.fn(async () => ({ notifications: [] }));
    const a = {
      ...baseAuth,
      signedIn: true,
      account: "0xa",
      fetchNotifications,
    };
    const view = render(<Host auth={a} />);
    fireEvent.click(screen.getByRole("button", { name: "Open menu" }));
    fireEvent.click(
      within(screen.getByRole("dialog", { name: "Menu" })).getByRole("button", {
        name: "Notifications",
      }),
    );
    await screen.findByText("No notifications yet");
    view.rerender(<Host auth={{ ...a, account: "0xb" }} />);
    view.rerender(<Host auth={a} />);
    expect(screen.queryByRole("dialog", { name: "Notifications" })).toBeNull();
  });
  it("closes the mobile drawer before loading notifications and stays closed after the request settles", async () => {
    let release: (value: unknown) => void = () => {};
    const fetchNotifications = vi.fn(
      () =>
        new Promise<unknown>((resolve) => {
          release = resolve;
        }),
    );
    render(
      <Host
        auth={{
          ...baseAuth,
          signedIn: true,
          account: "0xa",
          fetchNotifications,
        }}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Open menu" }));
    fireEvent.click(
      within(screen.getByRole("dialog", { name: "Menu" })).getByRole("button", {
        name: "Notifications",
      }),
    );
    expect(screen.queryByRole("dialog", { name: "Menu" })).toBeNull();
    expect(screen.getByRole("dialog", { name: "Notifications" })).toBeTruthy();
    expect(screen.getByRole("status").textContent).toContain(
      "Loading notifications",
    );
    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    await act(async () => {
      release({ notifications: [] });
    });
    expect(screen.queryByRole("dialog", { name: "Notifications" })).toBeNull();
  });
  it("keeps standalone chrome fallback and refuses self-recursive renderer", () => {
    render(
      <MemoryRouter>
        <ChromeNavContext.Provider value={{ TopBar: DclTopBar }}>
          <DclTopBar />
        </ChromeNavContext.Provider>
      </MemoryRouter>,
    );
    expect(screen.getByRole("banner")).toBeTruthy();
    expect(
      screen.getByRole("button", { name: "Toggle navigation menu" }),
    ).toBeTruthy();
  });
});

describe("when a Sites page reports its area", () => {
  it.each([
    ["explore", "/places", "Places"],
    ["events", "/events", "Events"],
    ["whatson", "/events", "Events"],
  ] as const)(
    "should select Discover and %s's item on %s",
    (area, path, item) => {
      render(<Host pathname={path} children={<DclTopBar active={area} />} />);
      const discover = screen.getByRole("button", { name: "Discover" });
      fireEvent.click(discover);
      expect(
        screen.getByRole("link", { name: item }).getAttribute("aria-current"),
      ).toBe("page");
      const other = item === "Places" ? "Events" : "Places";
      expect(
        screen.getByRole("link", { name: other }).getAttribute("aria-current"),
      ).toBeNull();
    },
  );
  it("should select no section for an area outside the navigation", () => {
    render(
      <Host pathname="/social/" children={<DclTopBar active="social" />} />,
    );
    expect(document.querySelector('[aria-current="page"]')).toBeNull();
  });
});

describe("when the signed-in web visitor opens the profile", () => {
  const logout = vi.fn();
  const switchAccount = vi.fn();

  beforeEach(() => {
    logout.mockReset();
    switchAccount.mockReset();
    render(
      <Host pathname="/events" auth={{
        ...baseAuth, signedIn: true, account: "0xA", name: "Ada",
        onSignOut: logout, onSwitchAccount: switchAccount,
      }} />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Account: Ada" }));
  });

  it("should show the profile links in order and keep the logout action", () => {
    const profile = screen.getByRole("link", { name: "View Profile" });
    const panel = profile.closest("[tabindex='-1']")!;
    expect(within(panel as HTMLElement).getAllByRole("link").map((link) => link.getAttribute("href")))
      .toEqual(["/profile/0xa", "/shop/my-items", "/account", "/shop/settings"]);
    expect(screen.queryByRole("button", { name: "Switch account" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Logout" }));
    expect(logout).toHaveBeenCalledOnce();
  });
});

describe("when the embedded creator host owns the account", () => {
  const switchAccount = vi.fn();

  beforeEach(() => {
    switchAccount.mockReset();
    render(
      <Host pathname="/create/home" auth={{
        ...baseAuth, signedIn: true, account: "0xA", name: "Ada", onSwitchAccount: switchAccount,
      }} />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Account: Ada" }));
  });

  it("should keep the host switch action available", () => {
    expect(screen.queryByRole("button", { name: "Logout" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Switch account" }));
    expect(switchAccount).toHaveBeenCalledOnce();
  });
});

it("restores the actual bell opener after pointer activation and dialog dismissal", async () => {
  render(
    <Host
      auth={{
        ...baseAuth,
        signedIn: true,
        account: "0xa",
        fetchNotifications: async () => ({ notifications: [] }),
      }}
    />,
  );
  const bell = screen.getByRole("button", { name: "Notifications" });
  fireEvent.click(bell);
  await screen.findByText("No notifications yet");
  fireEvent.click(screen.getByRole("button", { name: "Close" }));
  expect(document.activeElement).toBe(bell);
});

it("keeps cached notifications alongside a background refresh error", async () => {
  const fetchNotifications = vi
    .fn()
    .mockResolvedValueOnce({
      notifications: [
        {
          id: "cached",
          type: "broadcast",
          timestamp: Date.now(),
          metadata: { title: "Cached notice", link: "/shop" },
        },
      ],
    })
    .mockRejectedValueOnce(new Error("offline"));
  render(
    <Host
      auth={{ ...baseAuth, signedIn: true, account: "0xa", fetchNotifications }}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "Notifications" }));
  await screen.findByRole("link", { name: /Cached notice/ });
  fireEvent.click(screen.getByRole("button", { name: "Close" }));
  fireEvent.click(
    screen.getByRole("button", { name: "Notifications, 1 unread" }),
  );
  await screen.findByRole("alert");
  expect(screen.getByRole("link", { name: /Cached notice/ })).toBeTruthy();
  expect(fetchNotifications).toHaveBeenCalledTimes(2);
});

describe("when a signed-in visitor has an unread notification", () => {
  it("should show the unread count before opening notifications", async () => {
    render(
      <Host
        pathname="/"
        auth={{
          ...baseAuth,
          signedIn: true,
          account: "0xa",
          fetchNotifications: async () => ({
            notifications: [
              {
                id: "unread",
                type: "broadcast",
                timestamp: Date.now(),
                read: false,
                metadata: { title: "Seeded notice" },
              },
              {
                id: "read",
                type: "broadcast",
                timestamp: Date.now(),
                read: true,
                metadata: { title: "Read notice" },
              },
            ],
          }),
        }}
      />,
    );
    const bell = await screen.findByRole("button", { name: /1 unread/ });
    expect(bell.getAttribute("aria-expanded")).toBe("false");
    fireEvent.click(bell);
    await screen.findByText("Seeded notice");
  });
});

describe("when Home notifications are opened", () => {
  it("should open a nonmodal inbox and restore focus on Escape", async () => {
    render(
      <Host
        pathname="/"
        auth={{
          ...baseAuth,
          signedIn: true,
          account: "0xa",
          fetchNotifications: async () => ({ notifications: [] }),
        }}
      />,
    );
    const bell = screen.getByRole("button", { name: "Notifications" });
    fireEvent.click(bell);
    const panel = screen.getByRole("dialog", { name: "Notifications" });
    await screen.findByText("No notifications yet");
    const close = within(panel).getByRole("button", { name: "Close" });
    close.focus();
    fireEvent.keyDown(close, { key: "Escape" });
    expect(screen.queryByRole("dialog", { name: "Notifications" })).toBeNull();
    expect(document.activeElement).toBe(bell);
  });
});
