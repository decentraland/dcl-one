// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { recoverMessageAddress } from "viem";
import { privateKeyToAccount } from "viem/accounts";

import { claimMagicReturn, finishMagicSignIn, isConsumedSession, returnFromState, startMagicSignIn } from "./magic";

// A made-up account standing in for the wallet Magic holds, so its signatures can be checked for real.
const ACCOUNT = privateKeyToAccount(`0x${"56".repeat(32)}`);
const SIGNER = ACCOUNT.address;

const sdk = vi.hoisted(() => {
  const calls: { method: string; params?: unknown[] }[] = [];
  const state = {
    loggedIn: false,
    redirectResult: (async () => ({ oauth: { userInfo: { email: "jane.doe@example.com" } } })) as () => Promise<unknown>,
    accounts: [] as string[],
    keys: [] as string[],
    logout: 0,
    loginWithRedirect: [] as unknown[],
  };
  class Magic {
    constructor(key: string) {
      state.keys.push(key);
    }
    user = {
      isLoggedIn: async () => state.loggedIn,
      logout: async () => {
        state.logout++;
      },
    };
    oauth2 = {
      loginWithRedirect: async (config: unknown) => {
        state.loginWithRedirect.push(config);
      },
      getRedirectResult: () => state.redirectResult(),
    };
    rpcProvider = {
      request: async (req: { method: string; params?: unknown[] }) => {
        calls.push(req);
        if (req.method === "eth_accounts") return state.accounts;
        if (req.method === "personal_sign") return ACCOUNT.signMessage({ message: { raw: req.params![0] as `0x${string}` } });
        return null;
      },
    };
  }
  return { Magic, calls, state };
});
vi.mock("magic-sdk", () => ({ Magic: sdk.Magic }));
vi.mock("@magic-ext/oauth2", () => ({ OAuthExtension: class {} }));

beforeEach(() => {
  localStorage.clear();
  sessionStorage.clear();
  sdk.calls.length = 0;
  Object.assign(sdk.state, { loggedIn: false, accounts: [SIGNER], keys: [], logout: 0, loginWithRedirect: [] });
  sdk.state.redirectResult = async () => ({ oauth: { userInfo: { email: "jane.doe@example.com" } } });
});
afterEach(() => localStorage.clear());

describe("social sign-in through Magic", () => {
  it("ends an earlier session, forgets earlier emails and sends the reader to the provider with the return path", async () => {
    sdk.state.loggedIn = true;
    localStorage.setItem("dcl_magic_user_email", "old@example.com");
    localStorage.setItem("dcl_thirdweb_user_email", "old@example.com");
    await startMagicSignIn("pk_example123", "x", "/shop?a=1");
    expect(sdk.state.keys).toEqual(["pk_example123"]);
    expect(sdk.state.logout).toBe(1);
    expect(localStorage.length).toBe(0);
    const nonce = sessionStorage.getItem("dcl:auth:magic-return");
    expect(nonce).toMatch(/^[0-9a-f]{32}$/);
    expect(sdk.state.loginWithRedirect).toEqual([
      { provider: "twitter", redirectURI: `${window.location.origin}/auth/callback`, customData: JSON.stringify({ redirectTo: "/shop?a=1", nonce }) },
    ]);
  });

  it("accepts the return of the sign-in this tab started, once, and no other", async () => {
    await startMagicSignIn("pk_example123", "google", "/shop");
    const customData = (sdk.state.loginWithRedirect[0] as { customData: string }).customData;
    const state = btoa(JSON.stringify({ customData }));
    const forged = btoa(JSON.stringify({ customData: JSON.stringify({ redirectTo: "/shop", nonce: "0".repeat(32) }) }));
    expect(claimMagicReturn(forged)).toBe(false);
    // A wrong guess uses the pending sign-in up too: it cannot be retried until it matches.
    expect(claimMagicReturn(state)).toBe(false);

    await startMagicSignIn("pk_example123", "google", "/shop");
    const fresh = btoa(JSON.stringify({ customData: (sdk.state.loginWithRedirect[1] as { customData: string }).customData }));
    expect(claimMagicReturn(fresh)).toBe(true);
    expect(claimMagicReturn(fresh)).toBe(false);
    expect(claimMagicReturn(null)).toBe(false);
  });

  it("recovers the return path from the callback's state, and nothing from a broken one", () => {
    const state = btoa(JSON.stringify({ customData: JSON.stringify({ redirectTo: "/shop?a=1" }) }));
    expect(returnFromState(state)).toBe("/shop?a=1");
    expect(returnFromState(null)).toBeNull();
    expect(returnFromState("%%%")).toBeNull();
    expect(returnFromState(btoa(JSON.stringify({ customData: "{}" })))).toBeNull();
  });

  it("signs a month-long session with Magic's account and keeps the email for profile setup", async () => {
    const before = Date.now();
    const identity = await finishMagicSignIn("pk_example123");
    expect(identity.signer).toBe(SIGNER.toLowerCase());
    expect(identity.authChain[0]).toEqual({ type: "SIGNER", payload: SIGNER.toLowerCase(), signature: "" });
    const sign = sdk.calls.find((c) => c.method === "personal_sign")!;
    expect(sign.params![1]).toBe(SIGNER.toLowerCase());
    // The chain's signature recovers to the account over the chain's own message: what a catalyst checks.
    const link = identity.authChain[1]!;
    expect(link.payload).toContain("Decentraland Login");
    expect((await recoverMessageAddress({ message: link.payload, signature: link.signature as `0x${string}` })).toLowerCase()).toBe(
      SIGNER.toLowerCase(),
    );
    const days = (new Date(identity.expiration).getTime() - before) / 86_400_000;
    expect(days).toBeGreaterThan(29.99);
    expect(localStorage.getItem("dcl_magic_user_email")).toBe("jane.doe@example.com");
  });

  it("uses Magic's existing session when it skipped the OAuth check, and fails without an account", async () => {
    sdk.state.redirectResult = async () => {
      throw Object.assign(new Error("User is already logged in"), { code: -32600 });
    };
    expect((await finishMagicSignIn("pk_example123")).signer).toBe(SIGNER.toLowerCase());

    sdk.state.accounts = [];
    localStorage.clear();
    await expect(finishMagicSignIn("pk_example123")).rejects.toThrow("Could not connect to your account");
    // Magic holds a session even though nothing was signed: sign-out must still end it.
    expect(localStorage.getItem("dcl:auth:magic-session")).toBe("1");
  });

  it("passes any other provider failure on, and recognises a consumed OAuth session", async () => {
    sdk.state.redirectResult = async () => {
      throw Object.assign(new Error("PKCE missing"), { code: "MISSING_PKCE_METADATA" });
    };
    const err = await finishMagicSignIn("pk_example123").catch((e: unknown) => e);
    expect(isConsumedSession(err)).toBe(true);
    expect(isConsumedSession(new Error("other"))).toBe(false);
  });
});
