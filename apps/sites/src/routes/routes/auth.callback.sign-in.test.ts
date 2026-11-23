import { generatePrivateKey, privateKeyToAccount, type PrivateKeyAccount } from "viem/accounts";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

// What each sign-in method posts to /auth/callback, through the client helpers it calls, answered by
// the real action and its real verification: the Segment `userId` that root's loader then reads is
// the wallet decentraland.org's auth dapp identifies every method by (lowercased). The browser is
// modelled as it sends a same-origin fetch (Origin, Sec-Fetch-Site, its cookies); only the in-app
// wallet's API is stood in for. Which method calls which helper is covered in
// routes/sign-in-user-id.test.tsx.

const thirdweb = vi.hoisted(() => ({ getWalletForToken: vi.fn() }));
vi.mock("@/data/lib/auth/thirdweb/api", async (original) => ({
  ...(await original<typeof import("@/data/lib/auth/thirdweb/api")>()),
  ...thirdweb,
}));

import { readVerifiedWallet } from "@/core/lib/telemetry/session";
import { restoreTrackedUser, trackedUserDropped } from "@/core/lib/telemetry/tracked-user";
import { createIdentityFromPrivateKey, createIdentityWith } from "@/data/lib/auth/identity";
import {
  beginSignOut,
  confirmSignIn,
  postAuthCallback,
  resetSessionChangeForTests,
  suspendSignedInAccount,
  switchSignedInAccount,
} from "@/data/lib/auth/session-change";
import { signRequest } from "@/data/lib/auth/signer";
import type { AuthIdentity } from "@/data/lib/auth/types";

import { action } from "./auth.callback";

const ORIGIN = "https://example.test";
const CALLBACK = `${ORIGIN}/auth/callback`;
const EMAIL_TOKEN = "tw-email-token-example";
const PREVIOUS = `0x${"9".repeat(40)}`;

let account: PrivateKeyAccount;
let key: `0x${string}`;
let wallet: string;
/** The cookies the browser sends on the next request, after the Set-Cookie answers it got. */
let jar: Map<string, string>;
/** Every post the browser made, challenges included. */
let posts: number;
/** The cookie-changing posts in the order the server answered them. */
let answered: string[];
let offline: boolean;
/** Set by a test to hold the server's answers until it releases them. */
let hold: ((label: string) => Promise<void>) | null;

function cookieHeader(): string {
  return [...jar].map(([name, value]) => `${name}=${value}`).join("; ");
}

function userId(): string | null {
  return readVerifiedWallet(new Request(`${ORIGIN}/`, { headers: { cookie: cookieHeader() } }));
}

/** What a page view sent now would carry as `userId`. */
function pageViewUserId(): string | null {
  return trackedUserDropped() ? null : userId();
}

function applySetCookies(response: Response) {
  for (const header of response.headers.getSetCookie()) {
    const [pair, ...attrs] = header.split(";").map((part) => part.trim());
    const eq = pair.indexOf("=");
    const name = pair.slice(0, eq);
    const value = pair.slice(eq + 1);
    if (attrs.includes("Max-Age=0") || !value) jar.delete(name);
    else jar.set(name, value);
  }
}

async function callAction(request: Request): Promise<Response> {
  return (await action({ request } as never)) as Response;
}

function labelOf(body: string, headers: Headers): string {
  const parsed = JSON.parse(body) as { signout?: true; challenge?: unknown; token?: string };
  if (parsed.signout) return "signout";
  if (parsed.challenge === true) return "challenge";
  if (parsed.token) return `token:${parsed.token}`;
  const signer = JSON.parse(headers.get("x-identity-auth-chain-0") ?? "{}") as { payload?: string };
  return `signed:${signer.payload?.toLowerCase()}`;
}

/** A same-origin `fetch` from a page on ORIGIN, as a browser sends it. */
async function browserFetch(input: RequestInfo | URL, init: RequestInit = {}): Promise<Response> {
  posts += 1;
  if (offline) throw new TypeError("Failed to fetch");
  const headers = new Headers(init.headers);
  headers.set("origin", ORIGIN);
  headers.set("sec-fetch-site", "same-origin");
  headers.set("cookie", cookieHeader());
  const body = String(init.body);
  const request = new Request(new URL(String(input), ORIGIN), { method: init.method, headers, body });
  const label = labelOf(body, headers);
  const response = await callAction(request);
  if (hold && label !== "challenge") await hold(label);
  if (label !== "challenge") answered.push(label);
  applySetCookies(response);
  return response;
}

beforeEach(() => {
  key = generatePrivateKey();
  account = privateKeyToAccount(key);
  wallet = account.address.toLowerCase();
  jar = new Map([["sid", "visitor-sid"]]);
  posts = 0;
  answered = [];
  offline = false;
  hold = null;
  resetSessionChangeForTests();
  restoreTrackedUser();
  thirdweb.getWalletForToken.mockImplementation(async (token: string) => (token === EMAIL_TOKEN ? wallet : null));
  vi.stubGlobal("fetch", vi.fn(browserFetch));
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.clearAllMocks();
  vi.useRealTimers();
});

describe("the userId after each sign-in method's post", () => {
  it("names the wallet after a session the account signed: Magic, a browser wallet, WalletConnect", async () => {
    // finishMagicSignIn, signInWithBrowserWallet and WalletConnect all sign the same login delegation.
    const identity = await createIdentityWith(account.address, (message) => account.signMessage({ message }));
    expect(await confirmSignIn(identity)).toEqual({ ok: true, status: 200 });
    expect(userId()).toBe(wallet);
    // The challenge was used up.
    expect(jar.has("dcl_wallet_challenge")).toBe(false);
  });

  it("names the wallet after an e-mail sign-in, by its in-app token", async () => {
    expect(await postAuthCallback({ token: EMAIL_TOKEN })).toEqual({ ok: true, status: 200 });
    expect(thirdweb.getWalletForToken).toHaveBeenCalledWith(EMAIL_TOKEN, expect.anything());
    expect(userId()).toBe(wallet);
    expect(jar.has("dcl_wallet_challenge")).toBe(false);
  });

  it("names nobody after signing out of a Magic sign-in", async () => {
    await confirmSignIn(await createIdentityFromPrivateKey(key));
    expect(userId()).toBe(wallet);
    expect(await postAuthCallback({ signout: true })).toEqual({ ok: true, status: 200 });
    expect(userId()).toBeNull();
  });
});

describe("a sign-in the callback refuses", () => {
  it("refuses a session naming the wallet but signed by another key, clearing an earlier account", async () => {
    jar.set("dcl_wallet_verified", PREVIOUS);
    const other = privateKeyToAccount(generatePrivateKey());
    const forged = await createIdentityWith(wallet, (message) => other.signMessage({ message }));
    expect(await confirmSignIn(forged)).toEqual({ ok: false, status: 401 });
    expect(userId()).toBeNull();
  });

  it("refuses an e-mail token the in-app wallet does not know, clearing an earlier account", async () => {
    jar.set("dcl_wallet_verified", PREVIOUS);
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
    jar.set("dcl_wallet_verified", PREVIOUS);
    offline = true;
    expect(await confirmSignIn(await createIdentityFromPrivateKey(key))).toBeNull();
    expect(posts).toBe(1);
    expect(userId()).toBe(PREVIOUS);
  });
});

describe("/auth/callback's boundary: only this site's own JSON posts change a cookie", () => {
  function post(body: string, headers: Record<string, string> = {}, method = "POST") {
    return callAction(
      new Request(CALLBACK, {
        method,
        headers: { origin: ORIGIN, "sec-fetch-site": "same-origin", "content-type": "application/json", ...headers },
        ...(method === "POST" ? { body } : {}),
      }),
    );
  }

  it("answers a trusted JSON sign-out, a charset included, by clearing the cookie", async () => {
    const response = await post('{"signout":true}', { "content-type": "application/json; charset=utf-8" });
    expect(response.status).toBe(200);
    expect(response.headers.getSetCookie()).toEqual(["dcl_wallet_verified=; Path=/; Max-Age=0; SameSite=Lax; HttpOnly; Secure"]);
  });

  it("trusts the scheme the gateway terminated TLS for", async () => {
    const request = new Request("http://example.test/auth/callback", {
      method: "POST",
      headers: { origin: ORIGIN, "x-forwarded-proto": "https", "content-type": "application/json" },
      body: '{"signout":true}',
    });
    expect((await callAction(request)).status).toBe(200);
  });

  it.each([
    ["a foreign Origin", { origin: "https://evil.example" }, 403],
    ["no Origin", { origin: "" }, 403],
    ["the right Origin from a cross-site fetch", { "sec-fetch-site": "cross-site" }, 403],
    ["a same-site subdomain", { "sec-fetch-site": "same-site" }, 403],
    ["a form's media type", { "content-type": "application/x-www-form-urlencoded" }, 415],
    ["plain text", { "content-type": "text/plain" }, 415],
    ["no media type", { "content-type": "" }, 415],
  ])("refuses a sign-out with %s, setting no cookie", async (_name, headers, status) => {
    const response = await post('{"signout":true}', headers);
    expect(response.status).toBe(status);
    expect(response.headers.getSetCookie()).toEqual([]);
  });

  it("refuses a challenge asked for from another site, issuing none", async () => {
    const response = await post('{"challenge":true}', { origin: "https://evil.example" });
    expect(response.status).toBe(403);
    expect(response.headers.getSetCookie()).toEqual([]);
  });

  it.each([
    ["not JSON", "signout=true"],
    ["an empty body", ""],
    ["an array", "[]"],
    ["null", "null"],
    ["no intent", "{}"],
    ["an unknown field", '{"signout":true,"wallet":"0x01"}'],
    ["two intents", '{"signout":true,"challenge":true}'],
    ["a sign-out that is not true", '{"signout":"yes"}'],
    ["a token without a challenge", '{"token":"tw-email-token-example"}'],
    ["a malformed challenge", '{"challenge":"short"}'],
    ["an empty token", `{"token":"","challenge":"${"a".repeat(43)}"}`],
  ])("refuses %s, setting no cookie", async (_name, body) => {
    const response = await post(body);
    expect(response.status).toBe(400);
    expect(response.headers.getSetCookie()).toEqual([]);
  });

  it("refuses anything but POST, setting no cookie", async () => {
    const response = await post("", {}, "GET");
    expect(response.status).toBe(405);
    expect(response.headers.getSetCookie()).toEqual([]);
  });
});

describe("/auth/callback's one-time challenge", () => {
  async function issue(): Promise<string> {
    const response = await browserFetch(CALLBACK, { method: "POST", headers: { "Content-Type": "application/json" }, body: '{"challenge":true}' });
    const cookie = response.headers.getSetCookie()[0];
    expect(cookie).toMatch(/^dcl_wallet_challenge=[A-Za-z0-9_-]{43}; Path=\/auth\/callback; Max-Age=300; SameSite=Strict; HttpOnly; Secure$/);
    return ((await response.json()) as { challenge: string }).challenge;
  }

  async function signedPost(identity: AuthIdentity, challenge: string, signed: string = challenge, cookie = cookieHeader()) {
    const s = await signRequest(identity, "POST", "/auth/callback", { challenge: signed });
    return callAction(
      new Request(CALLBACK, {
        method: "POST",
        headers: { ...s.headers, origin: ORIGIN, "content-type": "application/json", cookie },
        body: JSON.stringify({ challenge }),
      }),
    );
  }

  it("sets the cookie, lowercased and HttpOnly, for the wallet that signed its challenge, and uses the challenge up", async () => {
    const challenge = await issue();
    const response = await signedPost(await createIdentityFromPrivateKey(key), challenge);
    expect(response.status).toBe(200);
    expect(await response.json()).toEqual({ ok: true, address: wallet });
    expect(response.headers.getSetCookie()).toEqual([
      `dcl_wallet_verified=${wallet}; Path=/; Max-Age=2592000; SameSite=Lax; HttpOnly; Secure`,
      "dcl_wallet_challenge=; Path=/auth/callback; Max-Age=0; SameSite=Strict; HttpOnly; Secure",
    ]);
  });

  it("refuses a replay of a sign-in whose challenge was used, leaving the wallet cookie alone", async () => {
    const identity = await createIdentityFromPrivateKey(key);
    const challenge = await issue();
    const cookie = cookieHeader();
    applySetCookies(await signedPost(identity, challenge, challenge, cookie));
    expect(userId()).toBe(wallet);
    // The same signed request again, from the browser that used its challenge up.
    const replay = await signedPost(identity, challenge);
    expect(replay.status).toBe(401);
    expect(replay.headers.getSetCookie()).not.toContainEqual(expect.stringMatching(/^dcl_wallet_verified=/));
  });

  it("refuses a sign-in from a browser that never asked for the challenge", async () => {
    const challenge = await issue();
    const response = await signedPost(await createIdentityFromPrivateKey(key), challenge, challenge, "sid=another-browser");
    expect(response.status).toBe(401);
    expect(response.headers.getSetCookie()).not.toContainEqual(expect.stringMatching(/^dcl_wallet_verified=/));
  });

  it("refuses a sign-in with no challenge cookie, and one whose challenge is not this browser's", async () => {
    const identity = await createIdentityFromPrivateKey(key);
    await issue();
    const missing = await signedPost(identity, "b".repeat(43), "b".repeat(43), "sid=visitor-sid");
    expect(missing.status).toBe(401);
    const mismatched = await signedPost(identity, "b".repeat(43));
    expect(mismatched.status).toBe(401);
    for (const response of [missing, mismatched]) {
      expect(response.headers.getSetCookie()).toEqual(["dcl_wallet_challenge=; Path=/auth/callback; Max-Age=0; SameSite=Strict; HttpOnly; Secure"]);
    }
  });

  it("refuses a signature over another challenge than the one the body answers, clearing the wallet cookie", async () => {
    const challenge = await issue();
    const response = await signedPost(await createIdentityFromPrivateKey(key), challenge, "c".repeat(43));
    expect(response.status).toBe(401);
    expect(response.headers.getSetCookie()[0]).toMatch(/^dcl_wallet_verified=; .*Max-Age=0/);
  });

  it("refuses a signature made for another path, and one replayed after ten minutes", async () => {
    const identity = await createIdentityFromPrivateKey(key);
    let challenge = await issue();
    const elsewhere = await signRequest(identity, "POST", "/somewhere-else", { challenge });
    const moved = await callAction(
      new Request(CALLBACK, { method: "POST", headers: { ...elsewhere.headers, origin: ORIGIN, "content-type": "application/json", cookie: cookieHeader() }, body: JSON.stringify({ challenge }) }),
    );
    expect(moved.status).toBe(401);

    challenge = await issue();
    const signed = await signRequest(identity, "POST", "/auth/callback", { challenge });
    vi.useFakeTimers({ now: Number(signed.timestamp) + 11 * 60_000, toFake: ["Date"] });
    const stale = await callAction(
      new Request(CALLBACK, { method: "POST", headers: { ...signed.headers, origin: ORIGIN, "content-type": "application/json", cookie: cookieHeader() }, body: JSON.stringify({ challenge }) }),
    );
    expect(stale.status).toBe(401);
  });

  it("keeps the token's verdict when a post carries both a refused token and a valid signature", async () => {
    const challenge = await issue();
    const s = await signRequest(await createIdentityFromPrivateKey(key), "POST", "/auth/callback", { challenge });
    const response = await callAction(
      new Request(CALLBACK, {
        method: "POST",
        headers: { ...s.headers, origin: ORIGIN, "content-type": "application/json", cookie: cookieHeader() },
        body: JSON.stringify({ token: "tw-expired-example", challenge }),
      }),
    );
    expect(response.status).toBe(401);
  });
});

describe("a wallet switching the session to another account", () => {
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
    expect(answered).toEqual(["signout"]);
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

describe("cookie changes, one at a time: the last intent wins", () => {
  /** Holds the answer to the first post of `label` until released; the rest go through. */
  const held = new Map<string, Promise<void>>();
  function holdFirst(label: string) {
    let release!: () => void;
    held.set(label, new Promise<void>((resolve) => (release = resolve)));
    hold = async (seen) => {
      const wait = held.get(seen);
      held.delete(seen);
      await wait;
    };
    return () => release();
  }

  async function until(check: () => boolean) {
    await vi.waitFor(() => expect(check()).toBe(true));
  }

  it("ends signed out after a switch then a lock, with the switch's late answer naming nobody", async () => {
    // A browser wallet session on PREVIOUS; the wallet moves to another account, then locks.
    jar.set("dcl_wallet_verified", PREVIOUS);
    const release = holdFirst(`signed:${wallet}`);
    const identity = await createIdentityFromPrivateKey(key);
    const seen: Array<string | null> = [];
    const switched = switchSignedInAccount(identity);
    seen.push(pageViewUserId());
    suspendSignedInAccount();
    seen.push(pageViewUserId());
    await until(() => posts >= 2);
    // The lock's sign-out waits for the switch's answer: nothing else reaches the server meanwhile.
    expect(answered).toEqual([]);
    expect(posts).toBe(2);
    release();
    await switched;
    seen.push(pageViewUserId());
    await until(() => answered.length === 2);
    seen.push(pageViewUserId());
    expect(answered).toEqual([`signed:${wallet}`, "signout"]);
    expect(userId()).toBeNull();
    expect(seen).toEqual([null, null, null, null]);
  });

  it("ends on the returning account after a lock then its return, never naming it while locked", async () => {
    jar.set("dcl_wallet_verified", wallet);
    const releaseSignOut = holdFirst("signout");
    const releaseSignIn = holdFirst(`signed:${wallet}`);
    const identity = await createIdentityFromPrivateKey(key);
    const seen: Array<string | null> = [];
    suspendSignedInAccount();
    seen.push(pageViewUserId());
    const restored = switchSignedInAccount(identity);
    seen.push(pageViewUserId());
    await until(() => posts >= 1);
    releaseSignIn();
    seen.push(pageViewUserId());
    releaseSignOut();
    expect(await restored).toEqual({ ok: true, status: 200 });
    expect(answered).toEqual(["signout", `signed:${wallet}`]);
    expect(userId()).toBe(wallet);
    expect(pageViewUserId()).toBe(wallet);
    expect(seen).toEqual([null, null, null]);
  });

  it("does not clear the cookie for a refused switch that a newer one superseded", async () => {
    jar.set("dcl_wallet_verified", PREVIOUS);
    const other = privateKeyToAccount(generatePrivateKey());
    const forged = await createIdentityWith(PREVIOUS, (message) => other.signMessage({ message }));
    const identity = await createIdentityFromPrivateKey(key);
    const refused = switchSignedInAccount(forged);
    const latest = switchSignedInAccount(identity);
    expect(await refused).toEqual({ ok: false, status: 401 });
    expect(await latest).toEqual({ ok: true, status: 200 });
    // No sign-out between them: the refused one was no longer the latest intent.
    expect(answered).toEqual([`signed:${PREVIOUS}`, `signed:${wallet}`]);
    expect(userId()).toBe(wallet);
    expect(trackedUserDropped()).toBe(false);
  });

  it("keeps the account left out when a sign-in answers after a sign-out the visitor asked for since", async () => {
    const release = holdFirst(`token:${EMAIL_TOKEN}`);
    const signIn = postAuthCallback({ token: EMAIL_TOKEN });
    await until(() => posts >= 2);
    beginSignOut();
    const signOut = postAuthCallback({ signout: true });
    release();
    expect(await signIn).toEqual({ ok: true, status: 200 });
    // The sign-in answered while the sign-out was queued: it restores nothing.
    expect(trackedUserDropped()).toBe(true);
    await signOut;
    expect(answered).toEqual([`token:${EMAIL_TOKEN}`, "signout"]);
    expect(userId()).toBeNull();
    expect(pageViewUserId()).toBeNull();
  });
});
