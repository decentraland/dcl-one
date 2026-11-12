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
import { sitesNavSections } from "./SitesTopBar";

vi.hoisted(() => {
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshSig$: () => (type: unknown) => type,
    $RefreshReg$: () => {},
  });
});

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
afterEach(cleanup);

describe("Sites native chrome integration", () => {
  it("renders one banner and keeps signed-out links and dropdown destinations", () => {
    render(<Host />);
    expect(screen.getAllByRole("banner")).toHaveLength(1);
    expect(
      screen.getByRole("link", { name: "Shop" }).getAttribute("href"),
    ).toBe("/shop");
    expect(
      screen.getByRole("link", { name: "Shop" }).getAttribute("aria-current"),
    ).toBe("page");
    fireEvent.click(screen.getByRole("button", { name: "Play" }));
    expect(
      screen.getByRole("link", { name: "Jump in" }).getAttribute("href"),
    ).toBe("/play/");
    expect(
      screen.getByRole("link", { name: "Events" }).getAttribute("href"),
    ).toBe("/events");
    expect(screen.getByRole("link", { name: "Sign in" })).toBeTruthy();
    expect(screen.queryByLabelText("Credits")).toBeNull();
  });
  it("shows the canonical Events destination as selected inside the mobile menu", () => {
    render(
      <Host pathname="/events" children={<DclTopBar active="events" />} />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Open menu" }));
    const menu = within(screen.getByRole("dialog", { name: "Menu" }));
    const events = menu.getByRole("link", { name: "Events" });
    expect(events.getAttribute("href")).toBe("/events");
    expect(events.getAttribute("aria-current")).toBe("page");
    expect(menu.queryByRole("link", { name: "What's on" })).toBeNull();
  });
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

describe("when native navigation uses the shared Sites link definitions", () => {
  it("should preserve destinations and selected Play children", () => {
    const sections = sitesNavSections("events", "/events");
    expect(sections.map(({ label }) => label)).toEqual([
      "Play",
      "Shop",
      "Create",
      "Social",
      "Learn",
    ]);
    expect(sections[0]?.items).toEqual([
      { label: "Jump in", href: "/play/", active: false },
      { label: "Events", href: "/events", active: true },
    ]);
    expect(sections[1]?.href).toBe("/shop");
    expect(sections[2]?.href).toBe("/create");
    expect(sections[3]?.href).toBe("/social/");
  });
  it("should select Docs or Blog without selecting the other Learn child", () => {
    for (const [pathname, expected] of [
      ["/docs/player/", [true, false]],
      ["/blog/post", [false, true]],
    ] as const) {
      const learn = sitesNavSections("learn", pathname)[4];
      expect(learn?.items?.map(({ active }) => active)).toEqual(expected);
    }
    expect(
      sitesNavSections(undefined, "/").every(({ active }) => !active),
    ).toBe(true);
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
  fireEvent.click(screen.getByRole("button", { name: /^Notifications/ }));
  await screen.findByRole("alert");
  expect(screen.getByRole("link", { name: /Cached notice/ })).toBeTruthy();
  expect(fetchNotifications).toHaveBeenCalledTimes(2);
});
