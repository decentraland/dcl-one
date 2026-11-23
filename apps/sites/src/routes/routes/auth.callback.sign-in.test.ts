import { generatePrivateKey, privateKeyToAccount, type PrivateKeyAccount } from "viem/accounts";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

// What each sign-in method posts to /auth/callback, through the client helpers it calls, answered by
// the real action and its real verification: the Segment `userId` that root's loader then reads is
// the wallet decentraland.org's auth dapp identifies every method by (lowercased). Only the in-app
// wallet's API is stood in for. Which method calls which helper is covered in
// routes/sign-in-user-id.test.tsx.

const thirdweb = vi.hoisted(() => ({ getWalletForToken: vi.fn() }));
vi.mock("@/data/lib/auth/thirdweb/api", async (original) => ({
  ...(await original<typeof import("@/data/lib/auth/thirdweb/api")>()),
  ...thirdweb,
}));

import { readVerifiedWallet } from "@/core/lib/telemetry/session";
import { createIdentityFromPrivateKey, createIdentityWith } from "@/data/lib/auth/identity";
import { signRequest } from "@/data/lib/auth/signer";
import { restoreTrackedUser, trackedUserDropped } from "@/core/lib/telemetry/tracked-user";
import {
  confirmSignIn,
  postAuthCallback,
  resetSessionChangeForTests,
  switchSignedInAccount,
} from "@/data/lib/auth/session-change";

import { action } from "./auth.callback";

const ORIGIN = "https://example.test";
const EMAIL_TOKEN = "tw-email-token-example";

let account: PrivateKeyAccount;
let key: `0x${string}`;
let wallet: string;
/** The cookies the browser sends on the next request, after the Set-Cookie answers it got. */
let jar: Map<string, string>;
let posts: number;
let offline: boolean;

function userId(): string | null {
  const cookie = [...jar].map(([name, value]) => `${name}=${value}`).join("; ");
  return readVerifiedWallet(new Request(`${ORIGIN}/`, { headers: { cookie } }));
}

function applySetCookie(header: string | null) {
  if (!header) return;
  const [pair, ...attrs] = header.split(";").map((part) => part.trim());
  const eq = pair.indexOf("=");
  const name = pair.slice(0, eq);
  const value = pair.slice(eq + 1);
  if (attrs.includes("Max-Age=0") || !value) jar.delete(name);
  else jar.set(name, value);
}

async function callAction(request: Request): Promise<Response> {
  return (await action({ request } as never)) as Response;
}

beforeEach(() => {
  key = generatePrivateKey();
  account = privateKeyToAccount(key);
  wallet = account.address.toLowerCase();
  jar = new Map([["sid", "visitor-sid"]]);
  posts = 0;
  offline = false;
  resetSessionChangeForTests();
  restoreTrackedUser();
  thirdweb.getWalletForToken.mockImplementation(async (token: string) => (token === EMAIL_TOKEN ? wallet : null));
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: RequestInfo | URL, init: RequestInit = {}) => {
      posts += 1;
      if (offline) throw new TypeError("Failed to fetch");
      const request = new Request(new URL(String(input), ORIGIN), { method: init.method, headers: init.headers, body: init.body as BodyInit });
      const response = await callAction(request);
      applySetCookie(response.headers.get("Set-Cookie"));
      return response;
    }),
  );
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.clearAllMocks();
});

describe("the userId after each sign-in method's post", () => {
  it("names the wallet after a session the account signed: Magic, a browser wallet, WalletConnect", async () => {
    // finishMagicSignIn, signInWithBrowserWallet and WalletConnect all sign the same login delegation.
    const identity = await createIdentityWith(account.address, (message) => account.signMessage({ message }));
    expect(await confirmSignIn(identity)).toEqual({ ok: true, status: 200 });
    expect(userId()).toBe(wallet);
  });

  it("names the wallet after an e-mail sign-in, by its in-app token", async () => {
    expect(await postAuthCallback({ token: EMAIL_TOKEN })).toEqual({ ok: true, status: 200 });
    expect(thirdweb.getWalletForToken).toHaveBeenCalledWith(EMAIL_TOKEN, expect.anything());
    expect(userId()).toBe(wallet);
  });

  it("names nobody after signing out of a Magic sign-in", async () => {
    await confirmSignIn(await createIdentityFromPrivateKey(key));
    expect(userId()).toBe(wallet);
    expect(await postAuthCallback({ signout: true })).toEqual({ ok: true, status: 200 });
    expect(userId()).toBeNull();
  });
});

describe("a sign-in the callback refuses", () => {
  const EARLIER = `0x${"9".repeat(40)}`;

  it("refuses a session naming the wallet but signed by another key, clearing an earlier account", async () => {
    jar.set("dcl_wallet_verified", EARLIER);
    const other = privateKeyToAccount(generatePrivateKey());
    const forged = await createIdentityWith(wallet, (message) => other.signMessage({ message }));
    expect(await confirmSignIn(forged)).toEqual({ ok: false, status: 401 });
    expect(userId()).toBeNull();
  });

  it("refuses an e-mail token the in-app wallet does not know, clearing an earlier account", async () => {
    jar.set("dcl_wallet_verified", EARLIER);
    expect(await postAuthCallback({ token: "tw-expired-example" })).toEqual({ ok: false, status: 401 });
    expect(userId()).toBeNull();
  });

  it("posts nothing for a session that expired before it could sign", async () => {
    const expired = await createIdentityFromPrivateKey(key, { expirationMs: -1_000 });
    expect(await confirmSignIn(expired)).toBeNull();
    expect(posts).toBe(0);
    expect(userId()).toBeNull();
  });

  it("changes nothing when the post never reaches the server", async () => {
    jar.set("dcl_wallet_verified", EARLIER);
    offline = true;
    expect(await confirmSignIn(await createIdentityFromPrivateKey(key))).toBeNull();
    expect(posts).toBe(1);
    expect(userId()).toBe(EARLIER);
  });

  it("refuses a post with neither a token nor a signature", async () => {
    const response = await callAction(new Request(`${ORIGIN}/auth/callback`, { method: "POST", body: "{}" }));
    expect(response.status).toBe(401);
    expect(response.headers.get("Set-Cookie")).toBe("dcl_wallet_verified=; Path=/; Max-Age=0; SameSite=Lax; HttpOnly; Secure");
  });

  it("refuses a signature made for another path, and one replayed after ten minutes", async () => {
    const identity = await createIdentityFromPrivateKey(key);
    const elsewhere = await signRequest(identity, "POST", "/somewhere-else");
    const moved = await callAction(new Request(`${ORIGIN}/auth/callback`, { method: "POST", headers: elsewhere.headers, body: "{}" }));
    expect(moved.status).toBe(401);

    const signed = await signRequest(identity, "POST", "/auth/callback");
    vi.useFakeTimers({ now: Number(signed.timestamp) + 11 * 60_000, toFake: ["Date"] });
    try {
      const stale = await callAction(new Request(`${ORIGIN}/auth/callback`, { method: "POST", headers: signed.headers, body: "{}" }));
      expect(stale.status).toBe(401);
    } finally {
      vi.useRealTimers();
    }
  });

  it("keeps the token's verdict when a post carries both a refused token and a valid signature", async () => {
    const signed = await signRequest(await createIdentityFromPrivateKey(key), "POST", "/auth/callback");
    const response = await callAction(
      new Request(`${ORIGIN}/auth/callback`, { method: "POST", headers: signed.headers, body: JSON.stringify({ token: "tw-expired-example" }) }),
    );
    expect(response.status).toBe(401);
  });
});

describe("the cookie a signed sign-in sets", () => {
  it("names the signer, lowercased, HttpOnly and Secure, for 30 days", async () => {
    const signed = await signRequest(await createIdentityFromPrivateKey(key), "POST", "/auth/callback");
    const response = await callAction(new Request(`${ORIGIN}/auth/callback`, { method: "POST", headers: signed.headers, body: "{}" }));
    expect(response.status).toBe(200);
    expect(await response.json()).toEqual({ ok: true, address: wallet });
    expect(response.headers.get("Set-Cookie")).toBe(`dcl_wallet_verified=${wallet}; Path=/; Max-Age=2592000; SameSite=Lax; HttpOnly; Secure`);
  });
});

describe("a wallet switching the session to another account", () => {
  const PREVIOUS = `0x${"9".repeat(40)}`;

  beforeEach(() => jar.set("dcl_wallet_verified", PREVIOUS));

  it("names the new account once the server verified it", async () => {
    expect(await switchSignedInAccount(await createIdentityFromPrivateKey(key))).toEqual({ ok: true, status: 200 });
    expect(userId()).toBe(wallet);
    expect(trackedUserDropped()).toBe(false);
  });

  it("leaves no cookie naming the previous account when the new one is refused", async () => {
    const other = privateKeyToAccount(generatePrivateKey());
    const forged = await createIdentityWith(wallet, (message) => other.signMessage({ message }));
    expect(await switchSignedInAccount(forged)).toEqual({ ok: false, status: 401 });
    expect(userId()).toBeNull();
    expect(trackedUserDropped()).toBe(true);
  });

  it("clears the previous account when the new session cannot sign", async () => {
    const expired = await createIdentityFromPrivateKey(key, { expirationMs: -1_000 });
    expect(await switchSignedInAccount(expired)).toBeNull();
    expect(posts).toBe(1);
    expect(userId()).toBeNull();
  });

  it("names no account in page views when the network fails both posts", async () => {
    offline = true;
    expect(await switchSignedInAccount(await createIdentityFromPrivateKey(key))).toBeNull();
    expect(posts).toBe(2);
    // The cookie could not be cleared; page views leave the account out for this document.
    expect(trackedUserDropped()).toBe(true);
  });
});
