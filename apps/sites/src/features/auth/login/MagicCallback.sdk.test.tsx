// @vitest-environment jsdom
// The callback against a stand-in Magic SDK, for the case the nonce exists for: Magic still holding an earlier
// account's session, so it answers "already signed in" (-32600) instead of finishing an OAuth return.
import { act, cleanup, render } from "@testing-library/react";
import { privateKeyToAccount } from "viem/accounts";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { getIdentity, setIdentity } from "@/data/lib/auth/session";

import MagicCallback from "./MagicCallback";

vi.hoisted(() => {
  Object.assign(window, { __vite_plugin_react_preamble_installed__: true });
});

const ACCOUNT = privateKeyToAccount(`0x${"56".repeat(32)}`);
const sdk = vi.hoisted(() => ({ signs: 0, address: "", sign: (_raw: string): Promise<string> => Promise.reject(new Error("unset")) }));
vi.mock("magic-sdk", () => ({
  Magic: class {
    user = { isLoggedIn: async () => true, logout: async () => true };
    oauth2 = {
      getRedirectResult: async () => {
        throw Object.assign(new Error("User is already logged in"), { code: -32600 });
      },
    };
    rpcProvider = {
      request: async (req: { method: string; params?: unknown[] }) => {
        if (req.method === "eth_accounts") return [sdk.address];
        sdk.signs++;
        return sdk.sign(req.params![0] as string);
      },
    };
  },
}));
vi.mock("@magic-ext/oauth2", () => ({ OAuthExtension: class {} }));

const NONCE = "cd".repeat(16);
const STATE = btoa(JSON.stringify({ customData: JSON.stringify({ redirectTo: "/shop", nonce: NONCE }) }));
const assign = vi.fn();
const replace = vi.fn();

beforeEach(() => {
  sdk.address = ACCOUNT.address;
  sdk.sign = (raw) => ACCOUNT.signMessage({ message: { raw: raw as `0x${string}` } });
  sdk.signs = 0;
  localStorage.clear();
  sessionStorage.clear();
  setIdentity(null);
  vi.stubGlobal("location", {
    href: `http://localhost:3000/auth/callback?state=${encodeURIComponent(STATE)}`,
    origin: "http://localhost:3000",
    search: `?state=${encodeURIComponent(STATE)}`,
    assign,
    replace,
  });
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.clearAllMocks();
});

describe("returning from Magic while it holds an account's session", () => {
  it("signs that account in when this tab started the sign-in", async () => {
    sessionStorage.setItem("dcl:auth:magic-return", NONCE);
    await act(async () => {
      render(<MagicCallback magicApiKey="pk_example123" />);
    });
    await vi.waitFor(() => expect(assign).toHaveBeenCalledWith("/shop"));
    expect(getIdentity()?.signer).toBe(ACCOUNT.address.toLowerCase());
    expect(localStorage.getItem("dcl:auth:magic-session")).toBe("1");
  });

  it("signs nothing for a return replayed from the history after sign-out", async () => {
    await act(async () => {
      render(<MagicCallback magicApiKey="pk_example123" />);
    });
    expect(replace).toHaveBeenCalledWith("/auth/login?redirectTo=%2Fshop");
    expect(sdk.signs).toBe(0);
    expect(getIdentity()).toBeNull();
    expect(Object.keys(localStorage).filter((k) => k.startsWith("single-sign-on-"))).toEqual([]);
  });
});
