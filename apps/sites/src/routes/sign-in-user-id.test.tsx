// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, renderHook, screen, waitFor } from "@testing-library/react";
import { generatePrivateKey, privateKeyToAccount, type PrivateKeyAccount } from "viem/accounts";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() =>
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshReg$: () => {},
    $RefreshSig$: () => (type: unknown) => type,
  }),
);

// Every sign-in method posts its proof to /auth/callback before the reader leaves, so the next page's
// views carry the account as their Segment `userId`. The browser cannot load the route's action, so
// the server here answers as it does for each proof (a token the in-app wallet knows, a request the
// session signed); routes/routes/auth.callback.sign-in.test.ts runs the same proofs through the real
// action and its verification.

const thirdweb = vi.hoisted(() => ({
  completeEmailLogin: vi.fn(),
  initiateEmailLogin: vi.fn(async () => {}),
}));
vi.mock("@/data/lib/auth/thirdweb/api", async (original) => ({
  ...(await original<typeof import("@/data/lib/auth/thirdweb/api")>()),
  ...thirdweb,
}));
const magic = vi.hoisted(() => ({ finish: vi.fn(), end: vi.fn(async () => {}) }));
vi.mock("@/features/auth/login/magic", async (original) => ({
  ...(await original<typeof import("@/features/auth/login/magic")>()),
  finishMagicSignIn: magic.finish,
}));
vi.mock("@/data/lib/auth/magic-session", () => ({ endMagicSession: magic.end }));
const walletConnect = vi.hoisted(() => ({ start: vi.fn() }));
vi.mock("@web-core/auth/walletconnect", () => ({
  hasWalletConnect: () => true,
  startWalletConnectSignIn: walletConnect.start,
}));

import { readVerifiedWallet } from "@/core/lib/telemetry/session";
import { useAuth } from "@/data/lib/auth/context";
import { createIdentityFromPrivateKey } from "@/data/lib/auth/identity";
import { getIdentity, setIdentity } from "@/data/lib/auth/session";
import { resetSessionChangeForTests } from "@/data/lib/auth/session-change";
import LoginPage from "@/features/auth/login/LoginPage";
import MagicCallback from "@/features/auth/login/MagicCallback";

const ORIGIN = "https://example.test";
const EMAIL_TOKEN = "tw-email-token-example";
const NONCE = "ab".repeat(16);
const EARLIER = `0x${"9".repeat(40)}`;

type Proof = { token: string } | { signout: true } | { signedBy: string };

let account: PrivateKeyAccount;
let key: `0x${string}`;
let wallet: string;
/** The cookies the browser sends on the next request. */
let jar: Map<string, string>;
let proofs: Proof[];
/** How the server answers the next sign-in post: as it does, a refusal, or the network failing. */
let server: "answer" | "refuse" | "offline";
/** The verified wallet when the page navigated away: what the next page's views name. */
let userIdAtNavigation: string | null | undefined;

const assign = vi.fn(() => {
  userIdAtNavigation = userId();
});
const replace = vi.fn();

/** Root's `analyticsUserId` for the next request, read as root.tsx's loader reads it. */
function userId(): string | null {
  const cookie = [...jar].map(([name, value]) => `${name}=${value}`).join("; ");
  return readVerifiedWallet(new Request(`${ORIGIN}/`, { headers: { cookie } }));
}

function hexToUtf8(hex: string): string {
  const bytes = new Uint8Array((hex.length - 2) / 2);
  for (let i = 0; i < bytes.length; i++) bytes[i] = parseInt(hex.slice(2 + i * 2, 4 + i * 2), 16);
  return new TextDecoder().decode(bytes);
}

const CHALLENGE = "a".repeat(43);

function proofOf(init: RequestInit): Proof {
  const body = JSON.parse(String(init.body)) as { token?: string; signout?: true; challenge?: string };
  if (body.signout) return { signout: true };
  // A sign-in answers the challenge it asked for; a signed one signs it too.
  expect(body.challenge).toBe(CHALLENGE);
  if (body.token) return { token: body.token };
  const headers = new Headers(init.headers);
  expect(JSON.parse(headers.get("x-identity-metadata") ?? "{}")).toEqual({ challenge: CHALLENGE });
  const signer = JSON.parse(headers.get("x-identity-auth-chain-0") ?? "null") as { payload?: string } | null;
  expect(headers.get("x-identity-auth-chain-1")).toBeTruthy();
  expect(headers.get("x-identity-timestamp")).toBeTruthy();
  return { signedBy: signer?.payload?.toLowerCase() ?? "" };
}

async function routeFetch(input: RequestInfo | URL, init: RequestInit = {}): Promise<Response> {
  const url = new URL(String(input), ORIGIN);
  if (url.pathname === "/auth/callback") {
    if (JSON.parse(String(init.body)).challenge === true) {
      if (server === "offline") throw new TypeError("Failed to fetch");
      return Response.json({ ok: true, challenge: CHALLENGE });
    }
    const proof = proofOf(init);
    proofs.push(proof);
    if ("signout" in proof) {
      jar.delete("dcl_wallet_verified");
      return Response.json({ ok: true });
    }
    if (server === "offline") throw new TypeError("Failed to fetch");
    const address = "token" in proof ? (proof.token === EMAIL_TOKEN ? wallet : null) : proof.signedBy;
    if (server === "refuse" || !address) {
      jar.delete("dcl_wallet_verified");
      return Response.json({ ok: false }, { status: 401 });
    }
    jar.set("dcl_wallet_verified", address);
    return Response.json({ ok: true, address });
  }
  if (url.pathname === "/internal/thirdweb-sign") {
    // The in-app wallet signs in its enclave; here the same account signs.
    const body = JSON.parse(String(init.body)) as { message: string };
    return Response.json({ signature: await account.signMessage({ message: body.message }) });
  }
  throw new TypeError(`unexpected fetch ${url.pathname}`);
}

beforeEach(() => {
  key = generatePrivateKey();
  account = privateKeyToAccount(key);
  wallet = account.address.toLowerCase();
  jar = new Map([["sid", "visitor-sid"]]);
  proofs = [];
  server = "answer";
  userIdAtNavigation = undefined;
  resetSessionChangeForTests();
  localStorage.clear();
  sessionStorage.clear();
  setIdentity(null);
  vi.stubGlobal("fetch", vi.fn(routeFetch));
  vi.stubGlobal("location", {
    href: `${ORIGIN}/auth/login`,
    origin: ORIGIN,
    hostname: "example.test",
    pathname: "/auth/login",
    search: "",
    hash: "",
    protocol: "https:",
    assign,
    replace,
  });
  thirdweb.completeEmailLogin.mockResolvedValue({ token: EMAIL_TOKEN, walletAddress: account.address });
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.clearAllMocks();
  delete (window as { ethereum?: unknown }).ethereum;
});

function returnFromMagic() {
  sessionStorage.setItem("dcl:auth:magic-return", NONCE);
  const state = btoa(JSON.stringify({ customData: JSON.stringify({ redirectTo: "/shop", nonce: NONCE }) }));
  const search = `?state=${encodeURIComponent(state)}`;
  vi.stubGlobal("location", { href: `${ORIGIN}/auth/callback${search}`, origin: ORIGIN, hostname: "example.test", search, protocol: "https:", assign, replace });
  render(<MagicCallback magicApiKey="pk_example123" />);
}

async function signInByEmail() {
  render(<LoginPage redirectTo="/shop" layoutVariant="two-options" magicApiKey={null} />);
  fireEvent.change(screen.getByPlaceholderText("Enter Your Email"), { target: { value: "jane.doe@example.com" } });
  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name: "NEXT" }));
  });
  const firstBox = await screen.findByTestId("otp-input-0");
  await act(async () => {
    fireEvent.paste(firstBox, { clipboardData: { getData: () => "123456" } });
  });
}

describe("the Segment userId after each sign-in method", () => {
  it("names the wallet after a Magic sign-in (Google, Apple, Discord, X), proven by a signed post before leaving", async () => {
    magic.finish.mockImplementation(() => createIdentityFromPrivateKey(key));
    returnFromMagic();
    await waitFor(() => expect(assign).toHaveBeenCalledWith("/shop"));
    expect(proofs).toEqual([{ signedBy: wallet }]);
    expect(userIdAtNavigation).toBe(wallet);
  });

  it("names the wallet after an e-mail sign-in, proven by its in-app token", async () => {
    await signInByEmail();
    await waitFor(() => expect(assign).toHaveBeenCalledWith("/shop"));
    expect(proofs).toEqual([{ token: EMAIL_TOKEN }]);
    expect(userIdAtNavigation).toBe(wallet);
  });

  it("names the wallet after a browser-wallet sign-in", async () => {
    (window as { ethereum?: unknown }).ethereum = {
      isMetaMask: true,
      request: vi.fn(async ({ method, params }: { method: string; params?: unknown[] }) => {
        if (method === "eth_accounts" || method === "eth_requestAccounts") return [wallet];
        if (method === "personal_sign") return account.signMessage({ message: hexToUtf8(String(params?.[0])) });
        return null;
      }),
    };
    render(<LoginPage redirectTo="/shop" layoutVariant="two-options" magicApiKey={null} />);
    const button = await screen.findByRole("button", { name: "Continue with MetaMask" });
    await act(async () => {
      fireEvent.click(button);
    });
    await waitFor(() => expect(assign).toHaveBeenCalledWith("/shop"));
    expect(proofs).toEqual([{ signedBy: wallet }]);
    expect(userIdAtNavigation).toBe(wallet);
  });

  it("names the wallet after a WalletConnect sign-in", async () => {
    const identity = await createIdentityFromPrivateKey(key);
    walletConnect.start.mockResolvedValue({ qrDataUrl: "data:image/png;base64,", uri: "wc:example", connected: Promise.resolve(identity), cancel: vi.fn() });
    render(<LoginPage redirectTo="/shop" layoutVariant="two-options" magicApiKey={null} />);
    fireEvent.click(screen.getByRole("button", { name: "More Options" }));
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "WalletConnect" }));
    });
    await waitFor(() => expect(assign).toHaveBeenCalledWith("/shop"));
    expect(proofs).toEqual([{ signedBy: wallet }]);
    expect(userIdAtNavigation).toBe(wallet);
  });
});

describe("a sign-in /auth/callback does not confirm", () => {
  it("names no account after a refused Magic sign-in, clearing an earlier one, and still finishes the sign-in", async () => {
    jar.set("dcl_wallet_verified", EARLIER);
    server = "refuse";
    magic.finish.mockImplementation(() => createIdentityFromPrivateKey(key));
    returnFromMagic();
    await waitFor(() => expect(assign).toHaveBeenCalledWith("/shop"));
    expect(userIdAtNavigation).toBeNull();
    expect(getIdentity()?.signer).toBe(wallet);
  });

  it("names no account after a refused e-mail sign-in, and still finishes the sign-in", async () => {
    server = "refuse";
    await signInByEmail();
    await waitFor(() => expect(assign).toHaveBeenCalledWith("/shop"));
    expect(userIdAtNavigation).toBeNull();
    expect(getIdentity()?.signer).toBe(wallet);
  });

  it("names no account when the Magic post never reaches the server, and still finishes the sign-in", async () => {
    server = "offline";
    magic.finish.mockImplementation(() => createIdentityFromPrivateKey(key));
    returnFromMagic();
    await waitFor(() => expect(assign).toHaveBeenCalledWith("/shop"));
    // The network failed on the challenge already: no proof left the browser.
    expect(proofs).toEqual([]);
    expect(userIdAtNavigation).toBeNull();
    expect(getIdentity()?.signer).toBe(wallet);
  });

  it("names no account when the e-mail post never reaches the server, and still finishes the sign-in", async () => {
    server = "offline";
    await signInByEmail();
    await waitFor(() => expect(assign).toHaveBeenCalledWith("/shop"));
    expect(userIdAtNavigation).toBeNull();
    expect(getIdentity()?.signer).toBe(wallet);
  });

  it("posts nothing for a Magic session that expired before it could sign", async () => {
    magic.finish.mockImplementation(() => createIdentityFromPrivateKey(key, { expirationMs: -1_000 }));
    returnFromMagic();
    await waitFor(() => expect(assign).toHaveBeenCalledWith("/shop"));
    expect(proofs).toEqual([]);
    expect(userIdAtNavigation).toBeNull();
  });
});

describe("signing out after a Magic sign-in", () => {
  it("names no account once the sign-out answers, and ends Magic's session", async () => {
    magic.finish.mockImplementation(() => createIdentityFromPrivateKey(key));
    returnFromMagic();
    await waitFor(() => expect(assign).toHaveBeenCalled());
    expect(userId()).toBe(wallet);
    cleanup();

    const { result } = renderHook(() => useAuth());
    await act(() => result.current.disconnect());
    await waitFor(() => expect(proofs).toEqual([{ signedBy: wallet }, { signout: true }]));
    expect(userId()).toBeNull();
    expect(getIdentity()).toBeNull();
    expect(magic.end).toHaveBeenCalledOnce();
  });
});
