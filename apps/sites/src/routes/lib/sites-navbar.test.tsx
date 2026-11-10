// @vitest-environment jsdom
import "@/features/locale/test-setup";
import {
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { ChromeAuthContext, type ChromeAuth } from "@ui/web/frames/chrome-auth";

const ACCOUNT = "0x1111111111111111111111111111111111112222";
const SESSION = { sid: "sid-1", story: "discover" };
const signedFetch = vi.fn();
const balance = vi.fn();

let connected = true;
vi.mock("@/data/lib/auth/index", () => ({
  useAuth: () => ({
    isConnected: connected,
    address: ACCOUNT,
    fetch: signedFetch,
    disconnect: vi.fn(),
  }),
}));
vi.mock("@ui/shop/credits/hooks", () => ({ useBalance: () => balance() }));
const readMana = vi.fn();
vi.mock("@ui/shop/checkout/lib/mana", () => ({
  readManaBalanceWei: (...args: unknown[]) => readMana(...args),
}));
vi.mock("@ui/shop/checkout/chain/config", () => ({
  ethereumChainId: () => 1,
  settlementChainId: () => 137,
}));
vi.mock("../shop/host", () => ({ installSitesShopClient: () => undefined }));
const openSignIn = vi.fn();
vi.mock("@/features/components/auth/signin-store", () => ({
  openSignIn: (...args: unknown[]) => openSignIn(...args),
}));
const track = vi.fn();
vi.mock("@/core/lib/telemetry/track", () => ({
  track: (...args: unknown[]) => track(...args),
}));

import type { LocaleMessages } from "@/features/locale/catalogs";
import { LocaleProvider } from "@/features/locale/LocaleProvider";

import SitesNavbarConnected, { type SitesHeaderVariant } from "./sites-navbar";

const messages: LocaleMessages = { locale: "en", messages: {}, fallback: {} };
const json = (body: unknown) =>
  new Response(JSON.stringify(body), { status: 200 });

function renderNavbar(
  auth: Partial<ChromeAuth> = {},
  {
    variant = "full",
    path = "/",
    onJumpIn,
  }: {
    variant?: SitesHeaderVariant;
    path?: string;
    onJumpIn?: () => void;
  } = {},
) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const value: ChromeAuth = {
    signedIn: true,
    account: ACCOUNT,
    mana: "",
    name: "Alice",
    committee: false,
    onSignOut: vi.fn(),
    ...auth,
  };
  render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={[path]}>
        <LocaleProvider initial={messages} load={vi.fn()}>
          <ChromeAuthContext.Provider value={value}>
            <SitesNavbarConnected
              variant={variant}
              session={SESSION}
              onJumpIn={onJumpIn}
            />
          </ChromeAuthContext.Provider>
        </LocaleProvider>
      </MemoryRouter>
    </QueryClientProvider>,
  );
  return { queryClient, auth: value };
}

beforeEach(() => {
  Object.defineProperty(window, "innerWidth", {
    value: 1440,
    configurable: true,
  });
  connected = true;
  track.mockClear();
  // The server: a PUT marks rows read, and a later list reflects it.
  const read = new Set(["n2"]);
  signedFetch
    .mockReset()
    .mockImplementation(async (_url: string, init?: RequestInit) => {
      if (init?.method === "PUT") {
        for (const id of (
          JSON.parse(String(init.body)) as { notificationIds: string[] }
        ).notificationIds)
          read.add(id);
        return json({ updated: 1 });
      }
      return json({
        notifications: [
          {
            id: "n1",
            type: "item_sold",
            address: ACCOUNT,
            timestamp: Date.now(),
            read: read.has("n1"),
            metadata: { title: "Sold" },
          },
          {
            id: "n2",
            type: "item_sold",
            address: ACCOUNT,
            timestamp: Date.now(),
            read: read.has("n2"),
            metadata: { title: "Old" },
          },
        ],
      });
    });
  balance.mockReset().mockReturnValue({
    data: { credits: 1250, balanceCents: "12500" },
    isError: false,
  });
  readMana
    .mockReset()
    .mockImplementation(
      async (_address: string, chainId: number) =>
        (chainId === 1 ? 10n : 5n) * 10n ** 18n,
    );
});

describe("SitesNavbarConnected", () => {
  describe("when the variant is full", () => {
    it("shows the menus and Sign In for a signed-out visitor", () => {
      renderNavbar({ signedIn: false, account: "" });

      expect(screen.getByRole("banner")).toBeInTheDocument();
      expect(
        screen.getByRole("button", {
          name: "Sign In",
        }),
      ).toBeInTheDocument();
      expect(
        screen.queryByRole("button", {
          name: "Profile",
        }),
      ).not.toBeInTheDocument();
    });

    it("signs in back to the same page and reports the click", async () => {
      openSignIn.mockClear();
      renderNavbar({ signedIn: false, account: "" }, { path: "/privacy?x=1" });

      await userEvent.click(
        screen.getByRole("button", {
          name: "Sign In",
        }),
      );

      expect(openSignIn).toHaveBeenCalledWith({ redirectTo: "/privacy?x=1" });
      expect(track).toHaveBeenCalledWith(
        "Click",
        expect.objectContaining({ place: "Landing Navbar", action: "sign_in" }),
        SESSION,
      );
    });

    it("shows the account for a signed-in visitor", () => {
      renderNavbar();

      expect(
        screen.getByRole("button", {
          name: "Profile",
        }),
      ).toBeInTheDocument();
      expect(
        screen.queryByRole("button", {
          name: "Sign In",
        }),
      ).not.toBeInTheDocument();
    });
  });

  it("links Discover to the real Events section when the signed-in source view provides tabs", () => {
    renderNavbar();
    expect(screen.getByRole("link", { name: "Discover" })).toHaveAttribute(
      "href",
      "/events",
    );
  });

  describe("when the variant is landing", () => {
    it("shows the logo and Sign In, without the menus, and signs in back to the same page", async () => {
      openSignIn.mockClear();
      renderNavbar(
        { signedIn: false, account: "" },
        { variant: "landing", path: "/?utm=x" },
      );

      expect(
        screen.getByRole("link", {
          name: "Decentraland",
        }),
      ).toHaveAttribute("href", "/");
      expect(
        screen.queryByRole("link", { name: "Learn" }),
      ).not.toBeInTheDocument();
      await userEvent.click(
        screen.getByRole("button", {
          name: "Sign In",
        }),
      );
      expect(openSignIn).toHaveBeenCalledWith({ redirectTo: "/?utm=x" });
      expect(track).toHaveBeenCalledWith(
        "Click",
        expect.objectContaining({ place: "Landing Navbar", action: "sign_in" }),
        SESSION,
      );
    });

    it("retains the source Sign In action after scrolling", () => {
      renderNavbar({ signedIn: false, account: "" }, { variant: "landing" });
      fireEvent.scroll(window);
      expect(
        screen.getByRole("button", { name: "Sign In" }),
      ).toBeInTheDocument();
    });

    it("reports the logo as the full navbar does", () => {
      renderNavbar({ signedIn: false, account: "" }, { variant: "landing" });

      const logo = screen.getByRole("link", {
        name: "Decentraland",
      });
      logo.addEventListener("click", (event) => event.preventDefault());
      fireEvent.click(logo);

      expect(track).toHaveBeenCalledWith(
        "Click",
        expect.objectContaining({
          place: "Landing Navbar",
          section: "home",
          action: "logo",
          href: "/",
        }),
        SESSION,
      );
    });
  });

  it("shows the full navbar for a signed-in visitor without the opt-in", () => {
    renderNavbar();

    expect(
      screen.getByRole("button", {
        name: "Profile",
      }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", {
        name: "Sign In",
      }),
    ).not.toBeInTheDocument();
  });

  it("does not invent a balance binding missing from the generated public API", () => {
    renderNavbar();
    expect(
      screen.queryByRole("link", {
        name: "component.landing.navbar.credits_balance",
      }),
    ).not.toBeInTheDocument();
  });

  it("hides the credits chip when the balance read fails", () => {
    balance.mockReturnValue({ data: undefined, isError: true });
    renderNavbar();

    expect(
      screen.queryByRole("link", {
        name: "component.landing.navbar.credits_balance",
      }),
    ).not.toBeInTheDocument();
  });

  it("marks the shown unread ones read when the panel opens", async () => {
    renderNavbar();

    const bell = await screen.findByRole("button", { name: "Notifications" });
    await userEvent.click(bell);

    await waitFor(() =>
      expect(signedFetch).toHaveBeenCalledWith(
        expect.stringMatching(/\/notifications\/notifications\/read$/),
        expect.objectContaining({
          method: "PUT",
          body: JSON.stringify({ notificationIds: ["n1"] }),
        }),
      ),
    );
    await waitFor(() =>
      expect(
        screen.getByRole("button", {
          name: "Notifications",
        }),
      ).toBeInTheDocument(),
    );
    expect(
      within(
        screen.getByRole("dialog", {
          name: "component.landing.navbar.notifications_title",
        }),
      ).getByText("Sold"),
    ).toBeInTheDocument();
  });

  it("marks only the rows the panel shows as read; rows past the cap stay unread", async () => {
    const rows = Array.from({ length: 40 }, (_, i) => ({
      id: `n${i}`,
      type: "item_sold",
      address: ACCOUNT,
      timestamp: Date.now() - i * 1000,
      read: false,
      metadata: { title: `Row ${i}` },
    }));
    signedFetch.mockImplementation(async (_url: string, init?: RequestInit) =>
      init?.method === "PUT"
        ? json({ updated: 30 })
        : json({ notifications: rows }),
    );
    renderNavbar();

    await userEvent.click(
      await screen.findByRole("button", { name: "Notifications" }),
    );
    await waitFor(() =>
      expect(
        signedFetch.mock.calls.some(([, init]) => init?.method === "PUT"),
      ).toBe(true),
    );

    const put = signedFetch.mock.calls.find(
      ([, init]) => init?.method === "PUT",
    )!;
    const ids = (
      JSON.parse(String(put[1].body)) as { notificationIds: string[] }
    ).notificationIds;
    expect(ids).toHaveLength(30);
    expect(ids).toEqual(rows.slice(0, 30).map((row) => row.id));
    expect(ids).not.toContain("n30");
  });

  it("marks the rows of a list that arrives after the panel opened", async () => {
    let release: (value: Response) => void = () => undefined;
    signedFetch.mockImplementation((_url: string, init?: RequestInit) => {
      if (init?.method === "PUT") return Promise.resolve(json({ updated: 1 }));
      return new Promise<Response>((resolve) => (release = resolve));
    });
    renderNavbar();

    await userEvent.click(
      screen.getByRole("button", {
        name: "Notifications",
      }),
    );
    expect(
      signedFetch.mock.calls.some(([, init]) => init?.method === "PUT"),
    ).toBe(false);
    release(
      json({
        notifications: [
          {
            id: "late",
            type: "item_sold",
            address: ACCOUNT,
            timestamp: Date.now(),
            read: false,
            metadata: {},
          },
        ],
      }),
    );

    await waitFor(() =>
      expect(signedFetch).toHaveBeenCalledWith(
        expect.stringMatching(/\/read$/),
        expect.objectContaining({
          method: "PUT",
          body: JSON.stringify({ notificationIds: ["late"] }),
        }),
      ),
    );
  });

  it.each([
    ["rejected", () => Promise.reject(new Error("offline"))],
    ["HTTP 500", () => Promise.resolve(new Response("{}", { status: 500 }))],
  ])(
    "keeps rows unread after a %s write, without retrying while open, and retries on the next opening",
    async (_case, failure) => {
      const state = { read: false, failNext: true };
      signedFetch.mockImplementation(
        async (_url: string, init?: RequestInit) => {
          if (init?.method === "PUT") {
            if (state.failNext) {
              state.failNext = false;
              return failure();
            }
            state.read = true;
            return json({ updated: 1 });
          }
          return json({
            notifications: [
              {
                id: "n1",
                type: "item_sold",
                address: ACCOUNT,
                timestamp: Date.now(),
                read: state.read,
                metadata: {},
              },
            ],
          });
        },
      );
      const puts = () =>
        signedFetch.mock.calls.filter(([, init]) => init?.method === "PUT")
          .length;
      renderNavbar();

      const bell = await screen.findByRole("button", { name: "Notifications" });
      await userEvent.click(bell);
      await waitFor(() => expect(puts()).toBe(1));
      // The failed row comes back unread from the reload, and is not written again while open.
      await waitFor(() =>
        expect(
          screen.getByRole("button", { name: "Notifications" }),
        ).toBeInTheDocument(),
      );
      await new Promise((resolve) => setTimeout(resolve, 30));
      expect(puts()).toBe(1);

      await userEvent.click(
        screen.getByRole("button", {
          name: "component.landing.navbar.close_menu",
        }),
      );
      await userEvent.click(
        screen.getByRole("button", { name: "Notifications" }),
      );
      await waitFor(() => expect(puts()).toBe(2));
      await waitFor(() =>
        expect(
          screen.getByRole("button", {
            name: "Notifications",
          }),
        ).toBeInTheDocument(),
      );
    },
  );

  it("sends nothing before the client session is live, even when the panel opens", async () => {
    connected = false;
    renderNavbar();

    await userEvent.click(
      screen.getByRole("button", {
        name: "Notifications",
      }),
    );
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(signedFetch).not.toHaveBeenCalled();
  });

  it("reads the MANA balances only when the account card opens", async () => {
    renderNavbar();
    await userEvent.click(
      screen.getByRole("button", {
        name: "Profile",
      }),
    );

    const card = screen.getByRole("dialog", {
      name: "Alice",
    });
    await waitFor(() => expect(card).toHaveTextContent("10"));
    expect(card).toHaveTextContent("Alice");
  });

  it("reports a failed MANA read instead of showing 0", async () => {
    readMana.mockImplementation(async (_address: string, chainId: number) => {
      if (chainId === 137) throw new Error("rpc down");
      return 10n * 10n ** 18n;
    });
    renderNavbar();
    await userEvent.click(
      screen.getByRole("button", {
        name: "Profile",
      }),
    );

    const card = screen.getByRole("dialog", {
      name: "Alice",
    });
    await waitFor(() =>
      expect(card).toHaveTextContent("component.landing.navbar.mana_error"),
    );
  });

  it("signs out through the shared chrome and clears every cached query", async () => {
    const { queryClient, auth } = renderNavbar();
    await waitFor(() =>
      expect(queryClient.getQueryCache().getAll().length).toBeGreaterThan(0),
    );

    await userEvent.click(
      screen.getByRole("button", {
        name: "Profile",
      }),
    );
    await userEvent.click(
      screen.getByRole("button", { name: "component.landing.navbar.logout" }),
    );

    expect(auth.onSignOut).toHaveBeenCalledTimes(1);
    expect(queryClient.getQueryCache().getAll()).toHaveLength(0);
  });

  describe("navbar clicks, as source's navbar reports them", () => {
    // jsdom does not navigate; keep it from trying.
    const stop = (event: Event) => event.preventDefault();
    beforeEach(() => document.addEventListener("click", stop));
    afterEach(() => document.removeEventListener("click", stop));
    const sent = () =>
      track.mock.calls
        .filter(([event]) => event === "Click")
        .map(([, props]) => props);
    const click = (props: Record<string, unknown>) =>
      expect.objectContaining({
        place: "Landing Navbar",
        event: "click",
        track_deferred: false,
        ...props,
      });

    it("sends the logo as action logo in section home", async () => {
      renderNavbar();
      await userEvent.click(
        screen.getByRole("link", {
          name: "Decentraland",
        }),
      );
      expect(sent()).toEqual([
        click({ section: "home", action: "logo", href: "/" }),
      ]);
      expect(track.mock.calls[0]![2]).toEqual(SESSION);
    });

    it("reports the generated Discover link under its actual section", async () => {
      renderNavbar();
      await userEvent.click(screen.getByRole("link", { name: "Discover" }));
      expect(sent()).toEqual([
        click({ section: "discover", action: "events", href: "/events" }),
      ]);
    });

    it("sends Learn", async () => {
      renderNavbar();
      await userEvent.click(screen.getByRole("link", { name: "Learn" }));
      expect(sent()).toEqual([
        click({ section: "learn", action: "learn", href: "/blog" }),
      ]);
    });

    it("sends a user card link under user_menu", async () => {
      renderNavbar();
      await userEvent.click(
        screen.getByRole("button", {
          name: "Profile",
        }),
      );
      await userEvent.click(
        screen.getByRole("link", {
          name: "component.landing.navbar.my_assets",
        }),
      );
      expect(sent()).toEqual([
        click({
          section: "user_menu",
          action: "my_assets",
          href: "/shop/my-items",
        }),
      ]);
    });
  });

  it("offers switching account instead when the Explorer host owns the session", async () => {
    const onSwitchAccount = vi.fn();
    renderNavbar({ onSignOut: undefined, onSwitchAccount });
    await userEvent.click(
      screen.getByRole("button", {
        name: "Profile",
      }),
    );

    expect(
      screen.queryByRole("button", { name: "component.landing.navbar.logout" }),
    ).not.toBeInTheDocument();
    await userEvent.click(
      screen.getByRole("button", {
        name: "component.landing.navbar.switch_account",
      }),
    );
    expect(onSwitchAccount).toHaveBeenCalledTimes(1);
  });
});
