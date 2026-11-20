// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from "vitest";
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
import { getAddress } from "viem";
import DclTopBar from "@ui/web/frames/DclTopBar";
import SiteLayout from "@ui/web/frames/SiteLayout";
import { ChromeAuthContext, type ChromeAuth } from "@ui/web/frames/chrome-auth";
import { ChromeNavContext } from "@ui/web/frames/chrome-nav";
import { SitesLocaleContext, type SitesLocale } from "@ui/web/frames/sites-locale";
import { forgetSignIn, recordSignIn } from "@/data/lib/auth/signin-kind";
import { setIdentity } from "@/data/lib/auth/session";
import { primeRemoteFlags } from "@ui/shop/discovery/lib/remoteFlags";
import ChromeNavBridge from "./ChromeNavBridge";

vi.hoisted(() => {
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshSig$: () => (type: unknown) => type,
    $RefreshReg$: () => {},
  });
});

const { readManaBalanceWei } = vi.hoisted(() => ({ readManaBalanceWei: vi.fn() }));
vi.mock("@ui/shop/checkout/lib/mana", async (importOriginal) => ({
  ...await importOriginal<typeof import("@ui/shop/checkout/lib/mana")>(),
  readManaBalanceWei,
}));

let baseAuth: ChromeAuth;
function Host({
  auth = baseAuth,
  children = <DclTopBar active="shop" />,
  pathname = "/shop",
  locale = "en",
}: {
  auth?: ChromeAuth;
  children?: React.ReactNode;
  pathname?: string;
  locale?: SitesLocale;
}) {
  const [client] = React.useState(
    () => new QueryClient({ defaultOptions: { queries: { retry: false } } }),
  );
  return (
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={[pathname]}>
        <ChromeAuthContext.Provider value={auth}>
          <SitesLocaleContext.Provider value={{ locale, onSelect: () => {} }}>
            <ChromeNavBridge>{children}</ChromeNavBridge>
          </SitesLocaleContext.Provider>
        </ChromeAuthContext.Provider>
      </MemoryRouter>
    </QueryClientProvider>
  );
}
import * as React from "react";
beforeEach(() => {
  baseAuth = {
    signedIn: false,
    account: "",
    mana: "",
    name: "",
    committee: false,
  };
  primeRemoteFlags({});
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
  setIdentity(null);
  primeRemoteFlags(null);
  forgetSignIn();
  vi.resetAllMocks();
  vi.unstubAllGlobals();
});

const shopAccount = "0x" + "ab".repeat(20);
const otherAccount = "0x" + "cd".repeat(20);
const checksummedAccount = getAddress(shopAccount);

function enableMarketplace(signer = shopAccount) {
  setIdentity({
    signer,
    ephemeral: { address: signer, privateKey: "0x00" },
    expiration: new Date(Date.now() + 60_000).toISOString(),
    authChain: [],
  });
  recordSignIn(signer, "web3");
  primeRemoteFlags({ "dapps-shop-marketplace-features": true });
}

function walletEvents() {
  type Listener = (...args: unknown[]) => void;
  const listeners = new Map<string, Set<Listener>>();
  return {
    on(event: string, listener: Listener) {
      const active = listeners.get(event) ?? new Set<Listener>();
      active.add(listener);
      listeners.set(event, active);
    },
    removeListener(event: string, listener: Listener) {
      listeners.get(event)?.delete(listener);
    },
    emit(event: string, ...args: unknown[]) {
      for (const listener of [...(listeners.get(event) ?? [])]) listener(...args);
    },
  };
}

function pendingWalletRead() {
  let resolve!: (accounts: string[]) => void;
  const promise = new Promise<string[]>((settle) => { resolve = settle; });
  return { request: () => promise, resolve };
}

type Menu = ReturnType<typeof within>;

describe("Sites native chrome integration", () => {
  describe("when the visitor is signed out", () => {
    beforeEach(() => {
      render(<Host />);
    });

    it("should render one banner", () => {
      expect(screen.getAllByRole("banner")).toHaveLength(1);
    });

    describe("and the Shop dropdown is opened", () => {
      beforeEach(() => {
        fireEvent.click(screen.getByRole("button", { name: "Shop" }));
      });

      it("should keep the Shop All destination", () => {
        expect(screen.getByRole("link", { name: "Shop All" }).getAttribute("href")).toBe("/shop/overview");
      });

      it("should not offer LAND", () => {
        expect(screen.queryByRole("link", { name: "LAND" })).toBeNull();
      });
    });

    describe("and the Discover dropdown is opened", () => {
      beforeEach(() => {
        fireEvent.click(screen.getByRole("button", { name: "Discover" }));
      });

      it("should keep the Events and Places destinations", () => {
        expect(
          screen.getByRole("link", { name: "Events" }).getAttribute("href"),
        ).toBe("/events");
        expect(
          screen.getByRole("link", { name: "Places" }).getAttribute("href"),
        ).toBe("/places");
      });
    });

    it("should offer sign-in and no credits", () => {
      expect(screen.getByRole("link", { name: "Sign in" })).toBeTruthy();
      expect(screen.queryByLabelText("Credits")).toBeNull();
    });
  });

  describe("when the marketplace is on and the wallet read is pending on an emote items page", () => {
    let wallet: ReturnType<typeof walletEvents>;
    let resolve: (accounts: string[]) => void;

    beforeEach(() => {
      enableMarketplace();
      wallet = walletEvents();
      const read = pendingWalletRead();
      resolve = read.resolve;
      vi.stubGlobal("ethereum", { ...wallet, request: read.request });
      render(
        <Host
          pathname="/shop/items?category=emote"
          auth={{ ...baseAuth, signedIn: true, account: shopAccount }}
        />,
      );
      fireEvent.click(screen.getByRole("button", { name: "Shop" }));
    });

    it("should select the Shop item from the router query", () => {
      expect(screen.getByRole("link", { name: "Emotes" }).getAttribute("aria-current")).toBe("page");
    });

    it("should link Names", () => {
      expect(screen.getByRole("link", { name: /names/i }).getAttribute("href")).toBe("/shop/names");
    });

    it("should not offer LAND yet", () => {
      expect(screen.queryByRole("link", { name: "LAND" })).toBeNull();
    });

    describe("and the wallet read matches the session", () => {
      beforeEach(async () => {
        await act(async () => resolve([checksummedAccount]));
      });

      it("should offer LAND", async () => {
        expect((await screen.findByRole("link", { name: "LAND" })).getAttribute("href")).toBe("/shop/lands");
      });

      describe("and the mobile menu's Shop section is opened", () => {
        let menu: Menu;

        beforeEach(async () => {
          await screen.findByRole("link", { name: "LAND" });
          fireEvent.click(screen.getByRole("button", { name: "Open menu" }));
          menu = within(screen.getByRole("dialog", { name: "Menu" }));
          fireEvent.click(menu.getByRole("button", { name: "Shop" }));
        });

        it("should offer LAND", () => {
          expect(menu.getByRole("link", { name: "LAND" }).getAttribute("href")).toBe("/shop/lands");
        });

        it("should select the Shop item", () => {
          expect(menu.getByRole("link", { name: "Emotes" }).getAttribute("aria-current")).toBe("page");
        });

        describe("and the wallet switches to another account", () => {
          beforeEach(() => {
            act(() => wallet.emit("accountsChanged", [otherAccount]));
          });

          it("should hide LAND", () => {
            expect(menu.queryByRole("link", { name: "LAND" })).toBeNull();
          });

          describe("and it switches back", () => {
            beforeEach(() => {
              act(() => wallet.emit("accountsChanged", [checksummedAccount]));
            });

            it("should offer LAND again", () => {
              expect(menu.getByRole("link", { name: "LAND" })).toBeTruthy();
            });
          });
        });
      });
    });
  });

  describe.each([
    { accounts: [otherAccount], reason: "a different account" },
    { accounts: [], reason: "an empty list" },
    { accounts: null, reason: "a rejection" },
  ])(
    "when the wallet read settles with $reason",
    ({ accounts }) => {
      let request: Mock<() => Promise<string[]>>;

      beforeEach(() => {
        enableMarketplace();
        request = vi.fn(async () => {
          if (accounts === null) throw new Error("Wallet read denied");
          return accounts;
        });
        vi.stubGlobal("ethereum", { request });
        render(<Host auth={{ ...baseAuth, signedIn: true, account: shopAccount }} />);
      });

      it("should read the wallet's accounts", () => {
        expect(request).toHaveBeenCalledWith({ method: "eth_accounts" });
      });

      describe("and the Shop dropdown is opened", () => {
        beforeEach(async () => {
          await act(async () => { await request.mock.results[0]!.value.catch(() => undefined); });
          fireEvent.click(screen.getByRole("button", { name: "Shop" }));
        });

        it("should show the marketplace items", () => {
          expect(screen.getByRole("link", { name: /names/i })).toBeTruthy();
        });

        it("should keep LAND hidden", () => {
          expect(screen.queryByRole("link", { name: "LAND" })).toBeNull();
        });
      });
    },
  );

  describe("when the chrome account differs from the Sites session", () => {
    beforeEach(async () => {
      enableMarketplace(otherAccount);
      const request = vi.fn().mockResolvedValue([checksummedAccount]);
      vi.stubGlobal("ethereum", { request });
      render(<Host auth={{ ...baseAuth, signedIn: true, account: shopAccount }} />);
      await act(async () => { await request.mock.results[0]?.value; });
      fireEvent.click(screen.getByRole("button", { name: "Shop" }));
    });

    it("should keep LAND hidden", () => {
      expect(screen.queryByRole("link", { name: "LAND" })).toBeNull();
    });
  });

  describe("when a stale wallet read resolves after an account change", () => {
    beforeEach(async () => {
      enableMarketplace();
      const wallet = walletEvents();
      const { request, resolve } = pendingWalletRead();
      vi.stubGlobal("ethereum", { ...wallet, request });
      render(<Host auth={{ ...baseAuth, signedIn: true, account: shopAccount }} />);
      fireEvent.click(screen.getByRole("button", { name: "Shop" }));

      act(() => wallet.emit("accountsChanged", [otherAccount]));
      await act(async () => resolve([checksummedAccount]));
    });

    it("should keep LAND hidden", () => {
      expect(screen.queryByRole("link", { name: "LAND" })).toBeNull();
    });
  });

  describe("when the mobile menu's Discover section is opened on /events", () => {
    let menu: Menu;

    beforeEach(() => {
      render(
        <Host pathname="/events" children={<DclTopBar active="events" />} />,
      );
      fireEvent.click(screen.getByRole("button", { name: "Open menu" }));
      menu = within(screen.getByRole("dialog", { name: "Menu" }));
      fireEvent.click(menu.getByRole("button", { name: "Discover" }));
    });

    it("should show the canonical Events destination as selected", () => {
      const events = menu.getByRole("link", { name: "Events" });
      expect(events.getAttribute("href")).toBe("/events");
      expect(events.getAttribute("aria-current")).toBe("page");
    });

    it("should not show a What's on item", () => {
      expect(menu.queryByRole("link", { name: "What's on" })).toBeNull();
    });
  });

  describe.each([false, true])(
    "when the mobile menu's Discover section is opened on /places with signedIn=%s",
    (signedIn) => {
      let menu: Menu;

      beforeEach(() => {
        render(
          <Host
            pathname="/places"
            auth={{ ...baseAuth, signedIn, account: signedIn ? "0xa" : "" }}
            children={<DclTopBar active="explore" />}
          />,
        );
        fireEvent.click(screen.getByRole("button", { name: "Open menu" }));
        menu = within(screen.getByRole("dialog", { name: "Menu" }));
        fireEvent.click(menu.getByRole("button", { name: "Discover" }));
      });

      it("should keep Places accessible and selected", () => {
        const places = menu.getByRole("link", { name: "Places" });
        expect(places.getAttribute("href")).toBe("/places");
        expect(places.getAttribute("aria-current")).toBe("page");
      });

      it("should not select Events", () => {
        expect(
          menu.getByRole("link", { name: "Events" }).getAttribute("aria-current"),
        ).toBeNull();
      });
    },
  );

  describe("when a SiteLayout forwards account and balance slots and the mobile menu opens", () => {
    let tab: Mock<() => void>;

    beforeEach(() => {
      tab = vi.fn();
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
    });

    it("should forward the balance slot once", () => {
      expect(screen.getAllByText("Real credits")).toHaveLength(1);
    });

    it("should forward the account slot once", () => {
      expect(
        screen.getAllByRole("button", { name: "Host account" }),
      ).toHaveLength(1);
    });

    describe("and a callback tab is picked", () => {
      beforeEach(() => {
        fireEvent.click(
          within(screen.getByRole("dialog", { name: "Menu" })).getByRole("button", {
            name: "Photos",
          }),
        );
      });

      it("should run the tab's callback", () => {
        expect(tab).toHaveBeenCalledOnce();
      });

      it("should close the mobile menu", async () => {
        await waitFor(() =>
          expect(screen.queryByRole("dialog", { name: "Menu" })).toBeNull(),
        );
      });
    });
  });

  describe("when account A's notifications were open and the account goes A to B to A", () => {
    beforeEach(async () => {
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
      await screen.findByText("No Notifications Yet");
      view.rerender(<Host auth={{ ...a, account: "0xb" }} />);
      view.rerender(<Host auth={a} />);
    });

    it("should not spontaneously reopen account A's notifications", () => {
      expect(screen.queryByRole("dialog", { name: "Notifications" })).toBeNull();
    });
  });

  describe("when the mobile drawer opens notifications", () => {
    let release: (value: unknown) => void;

    beforeEach(() => {
      release = () => {};
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
    });

    it("should close the mobile drawer", () => {
      expect(screen.queryByRole("dialog", { name: "Menu" })).toBeNull();
    });

    it("should open the notifications dialog", () => {
      expect(screen.getByRole("dialog", { name: "Notifications" })).toBeTruthy();
    });

    it("should show the notifications loading", () => {
      expect(screen.getByRole("status").textContent).toContain(
        "Loading notifications",
      );
    });

    describe("and it is closed before the request settles", () => {
      beforeEach(async () => {
        fireEvent.click(screen.getByRole("button", { name: "Close" }));
        await act(async () => {
          release({ notifications: [] });
        });
      });

      it("should stay closed", () => {
        expect(screen.queryByRole("dialog", { name: "Notifications" })).toBeNull();
      });
    });
  });

  describe("when the standalone chrome is its own nav renderer", () => {
    beforeEach(() => {
      render(
        <MemoryRouter>
          <ChromeNavContext.Provider value={{ TopBar: DclTopBar }}>
            <DclTopBar />
          </ChromeNavContext.Provider>
        </MemoryRouter>,
      );
    });

    it("should keep the standalone banner", () => {
      expect(screen.getByRole("banner")).toBeTruthy();
    });

    it("should refuse the self-recursive renderer and keep the standalone navigation toggle", () => {
      expect(
        screen.getByRole("button", { name: "Toggle navigation menu" }),
      ).toBeTruthy();
    });
  });
});

describe("when a Sites page reports its area", () => {
  describe.each([
    ["explore", "/places", "Places", "Events"],
    ["events", "/events", "Events", "Places"],
    ["whatson", "/events", "Events", "Places"],
  ] as const)(
    "and the area is %s on %s",
    (area, path, item, other) => {
      beforeEach(() => {
        render(<Host pathname={path} children={<DclTopBar active={area} />} />);
        const discover = screen.getByRole("button", { name: "Discover" });
        fireEvent.click(discover);
      });

      it(`should select Discover's ${item} item`, () => {
        expect(
          screen.getByRole("link", { name: item }).getAttribute("aria-current"),
        ).toBe("page");
      });

      it(`should not select ${other}`, () => {
        expect(
          screen.getByRole("link", { name: other }).getAttribute("aria-current"),
        ).toBeNull();
      });
    },
  );

  describe("and the area is outside the navigation", () => {
    beforeEach(() => {
      render(
        <Host pathname="/social/" children={<DclTopBar active="social" />} />,
      );
    });

    it("should select no section", () => {
      expect(document.querySelector('[aria-current="page"]')).toBeNull();
    });
  });
});

describe("when the signed-in web visitor opens the profile", () => {
  let logout: Mock<() => void>;
  let switchAccount: Mock<() => void>;

  beforeEach(() => {
    logout = vi.fn();
    switchAccount = vi.fn();
    render(
      <Host pathname="/events" auth={{
        ...baseAuth, signedIn: true, account: "0xA", name: "Ada",
        onSignOut: logout, onSwitchAccount: switchAccount,
      }} />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Account: Ada" }));
  });

  it("should show the profile links in order", () => {
    const profile = screen.getByRole("link", { name: "View Profile" });
    const panel = profile.closest("[tabindex='-1']")!;
    expect(within(panel as HTMLElement).getAllByRole("link").map((link) => link.getAttribute("href")))
      .toEqual(["/profile/0xa", "/shop/my-items", "/account", "/shop/settings"]);
  });

  it("should not offer to switch account", () => {
    expect(screen.queryByRole("button", { name: "Switch account" })).toBeNull();
  });

  describe("and Logout is clicked", () => {
    beforeEach(() => {
      fireEvent.click(screen.getByRole("button", { name: "Logout" }));
    });

    it("should log out", () => {
      expect(logout).toHaveBeenCalledOnce();
    });
  });
});

describe("when Nav Profile reads the matching browser wallet", () => {
  const bodyUrl = "https://example.com/avatar-body.png";
  let request: ReturnType<typeof vi.fn>;
  let wallet: ReturnType<typeof walletEvents>;
  let avatar: HTMLElement;
  let panel: HTMLElement;

  beforeEach(() => {
    wallet = walletEvents();
    request = vi.fn(async ({ method }: { method: string }) => {
      if (method === "eth_accounts") return [checksummedAccount];
      if (method === "eth_chainId") return "0x1";
      throw new Error(method);
    });
    vi.stubGlobal("ethereum", { ...wallet, request });
    recordSignIn(shopAccount, "web3");
    readManaBalanceWei.mockImplementation(async (_account, chain: number) => {
      if (chain === 1) return 0n;
      throw new Error("Polygon RPC unavailable");
    });
    render(
      <Host
        pathname="/"
        locale="es"
        auth={{ ...baseAuth, signedIn: true, account: shopAccount, name: "Ada", bodyUrl }}
      />,
    );
  });

  describe("and the panel stays closed", () => {
    beforeEach(async () => {
      await act(async () => {});
    });

    it("should read the wallet's accounts", () => {
      expect(request).toHaveBeenCalledWith({ method: "eth_accounts" });
    });

    it("should defer MANA until the panel opens", () => {
      expect(readManaBalanceWei).not.toHaveBeenCalled();
    });
  });

  describe("and the panel opens", () => {
    beforeEach(() => {
      avatar = screen.getByRole("button", { name: "Cuenta: Ada" });
      fireEvent.click(avatar);
    });

    it("should show the wallet network control", async () => {
      expect(await screen.findByRole("combobox", { name: "Red de la billetera" })).toBeTruthy();
    });

    describe("and the MANA reads settle with a Polygon RPC failure", () => {
      let openPanel: HTMLElement | null;

      beforeEach(async () => {
        await screen.findByRole("combobox", { name: "Red de la billetera" });
        openPanel = document.getElementById(avatar.getAttribute("aria-controls")!);
        await waitFor(() => expect(openPanel?.querySelector('[data-balance="mana"]')?.textContent).toBe("MANA: 0"));
      });

      it("should keep the available balance", () => {
        expect(openPanel?.querySelector('[data-balance="mana"]')?.textContent).toBe("MANA: 0");
      });

      it("should read both networks", () => {
        expect(readManaBalanceWei).toHaveBeenCalledTimes(2);
      });

      it("should drop the failed Polygon balance", () => {
        expect(openPanel?.querySelector('[data-balance="polygonMana"]')).toBeNull();
      });

      it("should show the avatar body lazily", () => {
        expect(openPanel?.querySelector('img[loading="lazy"]')?.getAttribute("src")).toBe(bodyUrl);
      });
    });

    describe("and the connected wallet changes", () => {
      beforeEach(async () => {
        await screen.findByRole("combobox", { name: "Red de la billetera" });
        await waitFor(() => expect(readManaBalanceWei).toHaveBeenCalledTimes(2));
        await act(async () => wallet.emit("accountsChanged", [otherAccount]));
      });

      it("should hide the wallet controls", () => {
        expect(screen.queryByRole("combobox", { name: "Red de la billetera" })).toBeNull();
      });

      it("should hide the MANA balance", () => {
        expect(document.querySelector('[data-balance="mana"]')).toBeNull();
      });

      describe("and the panel is toggled again", () => {
        beforeEach(async () => {
          await act(async () => {
            fireEvent.click(avatar);
            fireEvent.click(avatar);
          });
        });

        it("should not read MANA again", () => {
          expect(readManaBalanceWei).toHaveBeenCalledTimes(2);
        });
      });
    });
  });

  describe("and the eligible MANA reads are pending when the panel opens", () => {
    let resolveBalance: (value: bigint) => void;

    beforeEach(async () => {
      resolveBalance = () => undefined;
      const pending = new Promise<bigint>((resolve) => { resolveBalance = resolve; });
      readManaBalanceWei.mockReturnValue(pending);
      await act(async () => { await request.mock.results[0]!.value; });
      avatar = screen.getByRole("button", { name: "Cuenta: Ada" });
      fireEvent.click(avatar);
      panel = document.getElementById(avatar.getAttribute("aria-controls")!)!;
    });

    it("should reserve a busy balance row", () => {
      expect(panel.querySelector('[aria-busy="true"]')).toBeTruthy();
    });

    it("should show no amounts yet", () => {
      expect(panel.querySelector("[data-balance]")).toBeNull();
    });

    describe("and the reads resolve", () => {
      beforeEach(async () => {
        await act(async () => resolveBalance(0n));
        await waitFor(() => expect(panel.querySelector('[aria-busy="true"]')).toBeNull());
      });

      it("should show the Polygon balance", () => {
        expect(panel.querySelector('[data-balance="polygonMana"]')?.textContent).toBe("MANA en Polygon: 0");
      });
    });
  });

  describe("and both MANA reads fail when the panel opens", () => {
    let row: Element | null;

    beforeEach(async () => {
      readManaBalanceWei.mockRejectedValue(new Error("RPC unavailable"));
      await act(async () => { await request.mock.results[0]!.value; });
      avatar = screen.getByRole("button", { name: "Cuenta: Ada" });
      fireEvent.click(avatar);
      panel = document.getElementById(avatar.getAttribute("aria-controls")!)!;
      row = panel.querySelector('[aria-busy="true"]');
    });

    it("should reserve the balance row", () => {
      expect(row).toBeTruthy();
    });

    describe("and the reads settle", () => {
      beforeEach(async () => {
        await waitFor(() => expect(panel.querySelector('[aria-busy="true"]')).toBeNull());
      });

      it("should have read both networks", () => {
        expect(readManaBalanceWei).toHaveBeenCalledTimes(2);
      });

      it("should retain the balance row", () => {
        expect(panel.contains(row)).toBe(true);
      });

      it("should show no amounts", () => {
        expect(panel.querySelector("[data-balance]")).toBeNull();
      });
    });
  });

  describe("and the panel opens with a clipboard available", () => {
    let writeText: ReturnType<typeof vi.fn>;
    let clipboard: PropertyDescriptor | undefined;

    beforeEach(() => {
      writeText = vi.fn().mockResolvedValue(undefined);
      clipboard = Object.getOwnPropertyDescriptor(navigator, "clipboard");
      Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText } });
      fireEvent.click(screen.getByRole("button", { name: "Cuenta: Ada" }));
    });

    afterEach(() => {
      if (clipboard) Object.defineProperty(navigator, "clipboard", clipboard);
      else Reflect.deleteProperty(navigator, "clipboard");
    });

    it("should show the profile links in the Sites locale", () => {
      expect(screen.getByRole("link", { name: "Ver perfil" })).toBeTruthy();
      expect(screen.getByRole("link", { name: "Mis activos" })).toBeTruthy();
      expect(screen.getByRole("link", { name: "Configuración de la cuenta" })).toBeTruthy();
      expect(screen.getByRole("link", { name: "Autorizaciones de la tienda" })).toBeTruthy();
    });

    describe("and the address is copied", () => {
      beforeEach(async () => {
        fireEvent.click(screen.getByRole("button", { name: "Copiar ID de usuario" }));
        await waitFor(() => expect(screen.getByRole("status").textContent).toBe("ID de usuario copiado"));
      });

      it("should confirm the copy in the Sites locale", () => {
        expect(screen.getAllByText("ID de usuario copiado")).toHaveLength(2);
      });

      it("should copy the account address", () => {
        expect(writeText).toHaveBeenCalledExactlyOnceWith(shopAccount);
      });
    });
  });
});

describe("when the embedded creator host owns the account", () => {
  let switchAccount: Mock<() => void>;

  beforeEach(() => {
    switchAccount = vi.fn();
    render(
      <Host pathname="/create/home" auth={{
        ...baseAuth, signedIn: true, account: "0xA", name: "Ada", onSwitchAccount: switchAccount,
      }} />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Account: Ada" }));
  });

  it("should not offer Logout", () => {
    expect(screen.queryByRole("button", { name: "Logout" })).toBeNull();
  });

  describe("and Switch account is clicked", () => {
    beforeEach(() => {
      fireEvent.click(screen.getByRole("button", { name: "Switch account" }));
    });

    it("should hand the switch to the host", () => {
      expect(switchAccount).toHaveBeenCalledOnce();
    });
  });
});

describe("when the bell opens notifications by pointer and the dialog is dismissed", () => {
  let bell: HTMLElement;

  beforeEach(async () => {
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
    bell = screen.getByRole("button", { name: "Notifications" });
    fireEvent.click(bell);
    await screen.findByText("No Notifications Yet");
    fireEvent.click(screen.getByRole("button", { name: "Close" }));
  });

  it("should restore focus to the actual bell opener", () => {
    expect(document.activeElement).toBe(bell);
  });
});

describe("when the notifications include a community event and a sale", () => {
  let event: HTMLElement;
  let sale: HTMLElement;

  beforeEach(async () => {
    const fetchNotifications = vi.fn().mockResolvedValue({ notifications: ["events_started", "item_sold"].map(type => ({
      id: type, type, timestamp: Date.now(), metadata: { title: type, image: "https://example.org/item.png" }
    })) });
    render(<Host auth={{ ...baseAuth, signedIn: true, account: "0xa", fetchNotifications }} />);
    fireEvent.click(screen.getByRole("button", { name: "Notifications" }));
    event = (await screen.findByText("events_started")).closest("li")!;
    sale = screen.getByText("item_sold").closest("li")!;
  });

  it("should show standalone artwork for the community event", () => {
    expect(event.querySelectorAll("img")).toHaveLength(1);
    expect(event.querySelector("img")?.getAttribute("src")).not.toBe("https://example.org/item.png");
  });

  it("should show a badged thumbnail for the sale", () => {
    expect(sale.querySelectorAll("img")).toHaveLength(2);
    expect(sale.querySelector("img")?.getAttribute("src")).toBe("https://example.org/item.png");
  });
});

describe("when a background refresh of cached notifications fails", () => {
  let fetchNotifications: Mock<(signal?: AbortSignal) => Promise<unknown>>;

  beforeEach(async () => {
    fetchNotifications = vi
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
  });

  it("should keep the cached notifications", () => {
    expect(screen.getByRole("link", { name: /Cached notice/ })).toBeTruthy();
  });

  it("should have refreshed in the background", () => {
    expect(fetchNotifications).toHaveBeenCalledTimes(2);
  });
});

describe("when a signed-in visitor has an unread notification", () => {
  let bell: HTMLElement;

  beforeEach(async () => {
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
    bell = await screen.findByRole("button", { name: /1 unread/ });
  });

  it("should show the unread count before opening notifications", () => {
    expect(bell.getAttribute("aria-expanded")).toBe("false");
  });

  describe("and the bell is clicked", () => {
    beforeEach(() => {
      fireEvent.click(bell);
    });

    it("should show the unread notice", async () => {
      expect(await screen.findByText("Seeded notice")).toBeTruthy();
    });
  });
});

describe("when Home notifications are opened", () => {
  let bell: HTMLElement;

  beforeEach(async () => {
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
    bell = screen.getByRole("button", { name: "Notifications" });
    fireEvent.click(bell);
  });

  describe("and Escape is pressed on the nonmodal inbox", () => {
    beforeEach(async () => {
      const panel = screen.getByRole("dialog", { name: "Notifications" });
      await screen.findByText("No Notifications Yet");
      const close = within(panel).getByRole("button", { name: "Close" });
      close.focus();
      fireEvent.keyDown(close, { key: "Escape" });
    });

    it("should close the inbox", () => {
      expect(screen.queryByRole("dialog", { name: "Notifications" })).toBeNull();
    });

    it("should restore focus to the bell", () => {
      expect(document.activeElement).toBe(bell);
    });
  });
});
