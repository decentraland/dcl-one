// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
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
}: {
  auth?: ChromeAuth;
  children?: React.ReactNode;
}) {
  const [client] = React.useState(
    () => new QueryClient({ defaultOptions: { queries: { retry: false } } }),
  );
  return (
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={["/shop"]}>
        <ChromeAuthContext.Provider value={auth}>
          <ChromeNavBridge>{children}</ChromeNavBridge>
        </ChromeAuthContext.Provider>
      </MemoryRouter>
    </QueryClientProvider>
  );
}
import * as React from "react";
beforeEach(() => {
  window.matchMedia = vi
    .fn()
    .mockImplementation((query) => ({
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
      screen.getByRole("link", { name: "What's on" }).getAttribute("href"),
    ).toBe("/whats-on");
    expect(screen.getByRole("link", { name: "Sign in" })).toBeTruthy();
    expect(screen.queryByLabelText("Credits")).toBeNull();
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
