import { afterEach, describe, expect, test, vi } from "vitest";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from "@testing-library/react";

import DclTopBar from "./DclTopBar";
import ChromeShell from "../../components/ChromeShell";
import {
  ChromeNavContext,
  chromeLinkProps,
  isSameDocumentHref,
  type ChromeNavigate,
  type SitesNavBarProps,
} from "./chrome-nav";
import { ChromeAuthContext, type ChromeAuth } from "./chrome-auth";
import { getJSON } from "../../data/catalyst/client";

vi.mock("../../data/catalyst/client", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../data/catalyst/client")>()),
  getJSON: vi.fn(async () => ({})),
}));

afterEach(() => cleanup());

function mountBar(navigate: ChromeNavigate | undefined) {
  return render(
    <ChromeNavContext.Provider value={{ navigate }}>
      <DclTopBar variant="sites" active="whatson" signedIn={false} />
    </ChromeNavContext.Provider>,
  );
}

describe("top bar links hand same-document hrefs to the chrome navigator", () => {
  test("selects the generated navigation viewport at mobile, desktop, and large desktop widths", () => {
    const originalWidth = window.innerWidth;
    const renderSitesNavBar = vi.fn((props: SitesNavBarProps) => (
      <nav data-viewport={props.viewport} />
    ));
    const { unmount } = render(
      <ChromeNavContext value={{ renderSitesNavBar }}>
        <DclTopBar variant="sites" signedIn={false} />
      </ChromeNavContext>,
    );

    expect(renderSitesNavBar).toHaveBeenLastCalledWith(
      expect.objectContaining({ viewport: "desktop" }),
    );
    Object.defineProperty(window, "innerWidth", {
      configurable: true,
      value: 390,
    });
    fireEvent(window, new Event("resize"));
    expect(renderSitesNavBar).toHaveBeenLastCalledWith(
      expect.objectContaining({ viewport: "mobile" }),
    );
    Object.defineProperty(window, "innerWidth", {
      configurable: true,
      value: 1600,
    });
    fireEvent(window, new Event("resize"));
    expect(renderSitesNavBar).toHaveBeenLastCalledWith(
      expect.objectContaining({ viewport: "largeDesktop" }),
    );

    unmount();
    Object.defineProperty(window, "innerWidth", {
      configurable: true,
      value: originalWidth,
    });
  });

  test("the shared Sites bar keeps auth, profile, notification, and mobile-menu actions connected", async () => {
    const auth: ChromeAuth = {
      signedIn: true,
      account: "0x1111",
      mana: "",
      name: "Alice",
      committee: false,
      onSignOut: vi.fn(),
      onSwitchAccount: vi.fn(),
      fetchNotifications: vi.fn(async () => ({ notifications: [] })),
    };
    const navigate = vi.fn<ChromeNavigate>(() => true);
    const renderSitesNavBar = (props: SitesNavBarProps) => (
      <nav aria-label="Shared navigation" onClick={props.onClick}>
        <button onClick={props.onMenu}>Menu</button>
        <button onClick={props.onNotifications}>Notifications</button>
        <button onClick={props.onProfile}>Profile</button>
        <button onClick={props.onSignIn}>Sign in</button>
        <a href={props.label2Href}>Shop</a>
      </nav>
    );
    render(
      <ChromeAuthContext value={auth}>
        <ChromeNavContext value={{ navigate, renderSitesNavBar }}>
          <DclTopBar variant="sites" />
        </ChromeNavContext>
      </ChromeAuthContext>,
    );

    expect(
      screen.getByRole("navigation", { name: "Shared navigation" }),
    ).toBeInTheDocument();
    expect(fireEvent.click(screen.getByRole("link", { name: "Shop" }))).toBe(
      false,
    );
    expect(navigate).toHaveBeenCalledWith(expect.stringMatching(/\/shop$/));
    fireEvent.click(screen.getByRole("button", { name: "Menu" }));
    expect(
      screen.getByRole("link", { name: "What's on", hidden: true }),
    ).toHaveAttribute("href", "/whats-on");
    fireEvent.click(screen.getByRole("button", { name: "Profile" }));
    expect(screen.getByRole("menu", { name: "My account" })).toHaveTextContent(
      "Alice",
    );
    fireEvent.click(screen.getByRole("link", { name: "Shop" }));
    expect(screen.queryByRole("menu", { name: "My account" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Notifications" }));
    expect(
      await screen.findByRole("dialog", { name: "Notifications" }),
    ).toHaveTextContent("No notifications yet");
  });

  test("the signed-out desktop navigation keeps the existing Sites destinations", () => {
    const renderSitesNavBar = () => <nav aria-label="Generated navigation" />;
    render(
      <ChromeNavContext value={{ renderSitesNavBar }}>
        <DclTopBar variant="sites" signedIn={false} />
      </ChromeNavContext>,
    );

    expect(
      screen.getByRole("navigation", { name: "Decentraland sections" }),
    ).toContainElement(screen.getByRole("link", { name: "Shop" }));
    expect(screen.getByRole("link", { name: "Social" })).toHaveAttribute(
      "href",
      "/social/",
    );
    fireEvent.mouseEnter(
      screen.getByRole("link", { name: "Learn" }).parentElement!,
    );
    expect(screen.getByRole("menuitem", { name: "Docs" })).toHaveAttribute(
      "href",
      "/docs/",
    );
  });

  test("the signed-out mobile menu keeps section and submenu links reachable", () => {
    const originalWidth = window.innerWidth;
    Object.defineProperty(window, "innerWidth", {
      configurable: true,
      value: 390,
    });
    const renderSitesNavBar = () => <nav aria-label="Generated navigation" />;
    const { unmount } = render(
      <ChromeNavContext value={{ renderSitesNavBar }}>
        <DclTopBar variant="sites" signedIn={false} />
      </ChromeNavContext>,
    );

    const menuTrigger = document.querySelector<HTMLButtonElement>(
      ".dtb__shared-menu-trigger",
    );
    expect(menuTrigger).toHaveAttribute("aria-label", "Open navigation menu");
    fireEvent.click(menuTrigger!);
    expect(
      document.querySelector('.dtb__globalmenu a[href="/social/"]'),
    ).not.toBeNull();
    expect(
      document.querySelector('.dtb__globalmenu a[href="/docs/"]'),
    ).not.toBeNull();

    unmount();
    Object.defineProperty(window, "innerWidth", {
      configurable: true,
      value: originalWidth,
    });
  });

  test("the signed-in shared bar keeps legacy destinations in its More menu", () => {
    const renderSitesNavBar = () => <nav aria-label="Generated navigation" />;
    render(
      <ChromeNavContext value={{ renderSitesNavBar }}>
        <DclTopBar variant="sites" signedIn account="0x1111" />
      </ChromeNavContext>,
    );

    fireEvent.click(screen.getByRole("button", { name: "More" }));
    expect(screen.getByRole("menuitem", { name: "Jump in" })).toHaveAttribute(
      "href",
      "/play/",
    );
    expect(screen.getByRole("menuitem", { name: "Social" })).toHaveAttribute(
      "href",
      "/social/",
    );
    expect(screen.getByRole("menuitem", { name: "Docs" })).toHaveAttribute(
      "href",
      "/docs/",
    );
  });

  test("a host can keep its existing wallet connection control while signed out", () => {
    const connect = vi.fn();
    render(
      <DclTopBar
        active="social"
        accountControl={<button onClick={connect}>Connect wallet</button>}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Connect wallet" }));
    expect(connect).toHaveBeenCalledOnce();
    expect(screen.queryByRole("link", { name: "Sign in" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Notifications" })).toBeNull();
    expect(
      document.querySelector('.dtb__links [aria-current="page"]'),
    ).toHaveTextContent("Social");
  });
  test("notifications are only available while signed in, and late responses cannot reopen a signed-out inbox", async () => {
    let resolveNotifications!: (value: unknown) => void;
    const fetchNotifications = vi.fn(
      () =>
        new Promise((resolve) => {
          resolveNotifications = resolve;
        }),
    );
    const onSignIn = vi.fn();
    const { rerender } = render(
      <DclTopBar
        signedIn={false}
        onSignIn={onSignIn}
        fetchNotifications={fetchNotifications}
      />,
    );
    expect(screen.queryByRole("button", { name: "Notifications" })).toBeNull();
    expect(fetchNotifications).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Sign in" }));
    expect(onSignIn).toHaveBeenCalledOnce();

    rerender(<DclTopBar signedIn fetchNotifications={fetchNotifications} />);
    fireEvent.click(screen.getByRole("button", { name: "Notifications" }));
    expect(
      screen.getByRole("dialog", { name: "Notifications" }),
    ).toHaveTextContent("Loading");
    expect(fetchNotifications).toHaveBeenCalledOnce();
    rerender(
      <DclTopBar
        signedIn={false}
        onSignIn={onSignIn}
        fetchNotifications={fetchNotifications}
      />,
    );
    await act(async () => resolveNotifications({ notifications: [] }));
    expect(screen.queryByRole("dialog", { name: "Notifications" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Notifications" })).toBeNull();

    rerender(<DclTopBar signedIn fetchNotifications={fetchNotifications} />);
    expect(
      screen.getByRole("button", { name: "Notifications" }),
    ).toHaveAttribute("aria-expanded", "false");
    fireEvent.click(screen.getByRole("button", { name: "Notifications" }));
    await act(async () => resolveNotifications({ notifications: [] }));
    expect(
      screen.getByRole("dialog", { name: "Notifications" }),
    ).toHaveTextContent("No notifications yet");
  });

  test("reuses the shared profile on page entry and clears the avatar on an account switch", () => {
    vi.mocked(getJSON).mockClear();
    const auth: ChromeAuth = {
      signedIn: true,
      account: "0x1111",
      name: "Alice",
      mana: "",
      committee: false,
      avatarUrl: "https://images.example/alice.png",
    };
    const { rerender } = render(
      <ChromeAuthContext value={auth}>
        <DclTopBar />
      </ChromeAuthContext>,
    );
    expect(
      document.querySelector<HTMLImageElement>(".dtb__avatar img")?.src,
    ).toBe(auth.avatarUrl);
    rerender(
      <ChromeAuthContext
        value={{ ...auth, account: "0x2222", name: "", avatarUrl: undefined }}
      >
        <DclTopBar />
      </ChromeAuthContext>,
    );
    // The previous account's face goes; with none to show, the disc carries the Decentraland mark, as ui2's.
    expect(document.querySelector(".dtb__avatar .u-avatar__img")).toBeNull();
    expect(
      document.querySelector("[data-testid=topbar-avatar-logo]"),
    ).not.toBeNull();
    expect(getJSON).not.toHaveBeenCalled();
  });

  test("a plain click on Shop is routed through the navigator and the document load is prevented", () => {
    const navigate = vi.fn<ChromeNavigate>(() => true);
    mountBar(navigate);
    const shop = screen.getAllByRole("link", { name: "Shop" })[0]!;
    expect(shop).toHaveAttribute("href", "/shop");
    expect(shop).toHaveAttribute("data-discover", "true");
    const notPrevented = fireEvent.click(shop);
    expect(navigate).toHaveBeenCalledWith("/shop");
    expect(notPrevented).toBe(false);
  });

  test("modified clicks keep the browser's own new-tab behaviour", () => {
    const navigate = vi.fn<ChromeNavigate>(() => true);
    mountBar(navigate);
    const shop = screen.getAllByRole("link", { name: "Shop" })[0]!;
    expect(fireEvent.click(shop, { ctrlKey: true })).toBe(true);
    expect(fireEvent.click(shop, { metaKey: true })).toBe(true);
    expect(fireEvent.click(shop, { button: 1 })).toBe(true);
    expect(navigate).not.toHaveBeenCalled();
  });

  test("when the navigator declines an href the anchor falls back to a document load", () => {
    const navigate = vi.fn<ChromeNavigate>((href) => href !== "/docs/");
    mountBar(navigate);
    fireEvent.mouseEnter(document.querySelectorAll(".dtb__dd")[1]!);
    const docs = document.querySelector<HTMLAnchorElement>(
      '.dtb__dditem[href="/docs/"]',
    )!;
    expect(fireEvent.click(docs)).toBe(true);
    expect(navigate).toHaveBeenCalledWith("/docs/");
    const blog = document.querySelector<HTMLAnchorElement>(
      '.dtb__dditem[href="/blog"]',
    )!;
    expect(fireEvent.click(blog)).toBe(false);
  });

  test("without a navigator the links are plain anchors", () => {
    mountBar(undefined);
    const shop = screen.getAllByRole("link", { name: "Shop" })[0]!;
    expect(shop).not.toHaveAttribute("data-discover");
    expect(fireEvent.click(shop)).toBe(true);
  });

  test("ChromeShell tabs read the same navigator", () => {
    const navigate = vi.fn<ChromeNavigate>(() => true);
    render(
      <ChromeNavContext.Provider value={{ navigate }}>
        <ChromeShell
          tabs={[{ id: "cart", label: "Cart", href: "/marketplace/cart" }]}
          active="cart"
          tabsLabel="Sections"
        />
      </ChromeNavContext.Provider>,
    );
    const cart = screen.getAllByRole("link", { name: "Cart" })[0]!;
    expect(fireEvent.click(cart)).toBe(false);
    expect(navigate).toHaveBeenCalledWith("/marketplace/cart");
  });
});

describe("chromeLinkProps", () => {
  test("only same-document hrefs are discoverable or intercepted", () => {
    expect(isSameDocumentHref("/shop")).toBe(true);
    expect(isSameDocumentHref("//evil.example/shop")).toBe(false);
    expect(isSameDocumentHref("https://example.org/shop")).toBe(false);
    expect(isSameDocumentHref("mailto:x@y.z")).toBe(false);
    const go = vi.fn<ChromeNavigate>(() => true);
    expect(
      chromeLinkProps("https://x.example/", go)["data-discover"],
    ).toBeUndefined();
    expect(chromeLinkProps("/shop", go)["data-discover"]).toBe("true");
    expect(
      chromeLinkProps("/shop", undefined)["data-discover"],
    ).toBeUndefined();
  });
});
