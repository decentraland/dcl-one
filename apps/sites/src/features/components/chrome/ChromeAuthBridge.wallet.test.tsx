// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, render, screen } from "@testing-library/react";
import { createBrowserRouter, Outlet, RouterProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { clearIdentity, setBrowserWalletIdentity } from "@/data/lib/auth/session";
import { storeShared } from "@/data/lib/auth/single-sign-on";
import type { AuthIdentity } from "@/data/lib/auth/types";
import type { Eip1193Provider } from "@/data/lib/auth/wallet";
import { CREDITS_BALANCE_QUERY_KEY } from "@shop/credits/balance-query";
import { onBeforeBrowserSignIn } from "../auth/signin-store";

import ChromeAuthBridge from "./ChromeAuthBridge";

vi.hoisted(() => {
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshSig$: () => (type: unknown) => type,
    $RefreshReg$: () => {},
  });
});
vi.mock("@/data/lib/auth/use-profile-name", () => ({ useProfileIdentity: () => ({ name: "", avatarUrl: "" }) }));

const OLD_ACCOUNT = `0x${"ab".repeat(20)}`;
const NEW_ACCOUNT = `0x${"cd".repeat(20)}`;
const RETURN_TO = "/shop/items?sortBy=newest#results";
const PRIVATE_KEY = ["private", OLD_ACCOUNT, "account", "subscription"];
const BALANCE_KEY = [CREDITS_BALANCE_QUERY_KEY, OLD_ACCOUNT];

function identity(signer: string): AuthIdentity {
  return {
    signer,
    expiration: new Date(Date.now() + 60_000).toISOString(),
    ephemeral: { address: signer, privateKey: `0x${"12".repeat(32)}`, publicKey: `0x${"34".repeat(65)}` },
    authChain: [{ type: "SIGNER", payload: signer, signature: "" }],
  };
}

describe("when a browser wallet selects a different account", () => {
  let client: QueryClient;
  let router: ReturnType<typeof createBrowserRouter>;
  let previousUrl: string;
  let selectAccount: (address: string) => void;

  beforeEach(async () => {
    previousUrl = window.location.href;
    window.history.replaceState(null, "", RETURN_TO);
    clearIdentity();
    setBrowserWalletIdentity(identity(OLD_ACCOUNT), "injected");

    let activeAccount = OLD_ACCOUNT;
    const listeners = new Set<(...args: unknown[]) => void>();
    const wallet: Eip1193Provider = {
      request: async () => [activeAccount],
      on: (_event, listener) => { listeners.add(listener); },
      removeListener: (_event, listener) => { listeners.delete(listener); },
    };
    selectAccount = (account) => {
      activeAccount = account;
      for (const listener of [...listeners]) listener([account]);
    };
    vi.stubGlobal("ethereum", wallet);
    client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    client.setQueryData(PRIVATE_KEY, "old subscription");
    client.setQueryData(BALANCE_KEY, 10);
    router = createBrowserRouter([{
      id: "root",
      element: <ChromeAuthBridge><Outlet /></ChromeAuthBridge>,
      children: [
        { path: "/shop/items", element: <h1>Shop items</h1> },
        { path: "/auth/login", element: <h1>Sign in</h1> },
      ],
    }]);
    render(<QueryClientProvider client={client}><RouterProvider router={router} /></QueryClientProvider>);
    await screen.findByRole("heading", { name: "Shop items" });
  });

  afterEach(() => {
    cleanup();
    router.dispose();
    client.clear();
    clearIdentity();
    vi.unstubAllGlobals();
    window.history.replaceState(null, "", previousUrl);
  });

  describe("and that account needs to sign in", () => {
    it("should open sign-in with the full return path and remove cached data for the old account", async () => {
      await act(async () => selectAccount(NEW_ACCOUNT));
      await screen.findByRole("heading", { name: "Sign in" });
      expect(new URLSearchParams(window.location.search).get("redirectTo")).toBe(RETURN_TO);
      expect(client.getQueryData(PRIVATE_KEY)).toBeUndefined();
      expect(client.getQueryData(BALANCE_KEY)).toBeUndefined();
    });
  });

  describe("and that account already has a saved session", () => {
    let reload: ReturnType<typeof vi.fn>;
    let allowNavigation: () => void;
    let confirm: () => void;

    beforeEach(() => {
      storeShared(identity(NEW_ACCOUNT));
      reload = vi.fn();
      const { pathname, search, hash } = window.location;
      vi.stubGlobal("location", { pathname, search, hash, reload });
      allowNavigation = onBeforeBrowserSignIn((_returnTo, resume) => {
        confirm = resume;
        return false;
      });
    });

    afterEach(() => allowNavigation());

    it("should reload for the selected account after the page allows navigation", async () => {
      await act(async () => selectAccount(NEW_ACCOUNT));
      expect(reload).not.toHaveBeenCalled();
      expect(document.cookie).toContain(NEW_ACCOUNT);
      allowNavigation();
      confirm();
      expect(reload).toHaveBeenCalledOnce();
    });
  });
});
