// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { toShared } from "./single-sign-on";
import type { AuthIdentity } from "./types";
import type { Eip1193Provider } from "./wallet";

const A = `0x${"ab".repeat(20)}`;
const B = `0x${"cd".repeat(20)}`;
const PUBLIC_KEY = `0x${"12".repeat(65)}` as const;

function identity(signer: string, lifetime = 2 * 86_400_000): AuthIdentity {
  return {
    signer,
    expiration: new Date(Date.now() + lifetime).toISOString(),
    ephemeral: { address: A, privateKey: `0x${"12".repeat(32)}`, publicKey: PUBLIC_KEY },
    authChain: [{ type: "SIGNER", payload: signer, signature: "" }],
  };
}

function provider() {
  const listeners = new Set<(...args: unknown[]) => void>();
  return {
    request: vi.fn<Eip1193Provider["request"]>().mockResolvedValue([A]),
    on: (_: string, listener: (...args: unknown[]) => void) => { listeners.add(listener); },
    removeListener: (_: string, listener: (...args: unknown[]) => void) => { listeners.delete(listener); },
    emit: (accounts: unknown) => { for (const listener of [...listeners]) listener(accounts); },
  };
}

let session: typeof import("./session");
let signIn: typeof import("./signin-kind");
let wallet: ReturnType<typeof provider>;
let watch: typeof import("./wallet-account-switch").watchBrowserWalletAccount;
let onChange: ReturnType<typeof vi.fn<Parameters<typeof watch>[1]>>;
let stop: (() => void) | undefined;

/** The verified-wallet cookie as the server keeps it; it starts naming the signed-in account A. */
let verified: string | null;
let callbacks: Array<{ signedBy?: string; signout?: true }>;
/** How the server answers a sign-in post: as it does, a refusal (which clears the cookie), or the network failing. */
let server: "answer" | "refuse" | "offline";

async function authCallback(_url: string, init: RequestInit = {}): Promise<Response> {
  const body = JSON.parse(String(init.body)) as { signout?: true };
  if (body.signout) {
    callbacks.push({ signout: true });
    if (server === "offline") throw new TypeError("Failed to fetch");
    verified = null;
    return Response.json({ ok: true });
  }
  const signedBy = (JSON.parse(new Headers(init.headers).get("x-identity-auth-chain-0") ?? "{}") as { payload?: string }).payload;
  callbacks.push({ signedBy });
  if (server === "offline") throw new TypeError("Failed to fetch");
  if (server === "refuse" || !signedBy) {
    verified = null;
    return Response.json({ ok: false }, { status: 401 });
  }
  verified = signedBy;
  return Response.json({ ok: true, address: signedBy });
}

beforeEach(async () => {
  vi.resetModules();
  verified = A;
  callbacks = [];
  server = "answer";
  vi.stubGlobal("fetch", vi.fn(authCallback));
  localStorage.clear();
  sessionStorage.clear();
  wallet = provider();
  vi.stubGlobal("ethereum", wallet);
  session = await import("./session");
  signIn = await import("./signin-kind");
  watch = (await import("./wallet-account-switch")).watchBrowserWalletAccount;
  onChange = vi.fn();
});

afterEach(() => {
  stop?.();
  stop = undefined;
  session.clearIdentity();
  vi.unstubAllGlobals();
});

describe("when the Sites session belongs to a browser wallet", () => {
  beforeEach(() => {
    signIn.recordSignIn(A, "web3", "injected");
    session.setIdentity(identity(A));
    stop = watch(window, onChange);
  });

  describe("and the selected account has a cached session", () => {
    beforeEach(() => {
      localStorage.setItem(`single-sign-on-${B}`, JSON.stringify(toShared(identity(B, 3 * 86_400_000), PUBLIC_KEY)));
    });

    it("should activate the selected account and update the wallet cookie", async () => {
      wallet.request.mockResolvedValue([B]);
      wallet.emit([B.toUpperCase().replace("0X", "0x")]);
      expect(session.getIdentity()?.signer).toBe(B);
      expect(document.cookie).toContain(B);
      expect(onChange).toHaveBeenCalledWith("activated");
      await vi.waitFor(() => expect(verified).toBe(B));
    });

    it("should name the new account to Segment once the server verified it, and none before", async () => {
      const { trackedUserDropped } = await import("@/core/lib/telemetry/tracked-user");
      // The wallet now exposes B, to the event and to the next probe alike.
      wallet.request.mockResolvedValue([B]);
      wallet.emit([B]);
      // Until the server answers, the cookie still names A: page views leave the account out.
      expect(trackedUserDropped()).toBe(true);
      await vi.waitFor(() => expect(trackedUserDropped()).toBe(false));
      expect(callbacks).toEqual([{ signedBy: B }]);
      expect(verified).toBe(B);
    });

    it("should leave no cookie naming the old account when the server refuses the new one", async () => {
      const { trackedUserDropped } = await import("@/core/lib/telemetry/tracked-user");
      server = "refuse";
      // The wallet now exposes B, to the event and to the next probe alike.
      wallet.request.mockResolvedValue([B]);
      wallet.emit([B]);
      await vi.waitFor(() => expect(callbacks).toEqual([{ signedBy: B }]));
      await vi.waitFor(() => expect(verified).toBeNull());
      expect(trackedUserDropped()).toBe(true);
      expect(session.getIdentity()?.signer).toBe(B);
    });

    it("should keep page views from naming any account when the post never reaches the server", async () => {
      const { trackedUserDropped } = await import("@/core/lib/telemetry/tracked-user");
      server = "offline";
      // The wallet now exposes B, to the event and to the next probe alike.
      wallet.request.mockResolvedValue([B]);
      wallet.emit([B]);
      // The proof failed on the way, so the old account's cookie is cleared too (that fails here as well).
      await vi.waitFor(() => expect(callbacks).toEqual([{ signedBy: B }, { signout: true }]));
      expect(trackedUserDropped()).toBe(true);
      expect(session.getIdentity()?.signer).toBe(B);
    });

    describe("and the reader returns to the previous account", () => {
      beforeEach(async () => {
        wallet.request.mockResolvedValue([B]);
        wallet.emit([B]);
        await vi.waitFor(() => expect(verified).toBe(B));
        onChange.mockClear();
        callbacks = [];
      });

      it("should reuse the saved session and expose it as the Explorer's sign-in", async () => {
        wallet.request.mockResolvedValue([A]);
        wallet.emit([A]);
        expect(session.getIdentity()?.signer).toBe(A);
        expect(onChange).toHaveBeenCalledExactlyOnceWith("activated");
        // Switching back is a switch too: the server names A again.
        await vi.waitFor(() => expect(verified).toBe(A));
        expect(callbacks).toEqual([{ signedBy: A }]);
        const { getStoredLogin } = await import("@explorer-core/features/auth/sso");
        expect(getStoredLogin()?.address).toBe(A);
      });
    });
  });

  describe("and the selected account has an expired session", () => {
    beforeEach(() => {
      localStorage.setItem(`single-sign-on-${B}`, JSON.stringify(toShared(identity(B, -60_000), PUBLIC_KEY)));
    });

    it("should request sign-in and keep the old account suspended across a reload", async () => {
      wallet.request.mockResolvedValue([B]);
      wallet.emit([B]);
      expect(session.getIdentity()).toBeNull();
      expect(document.cookie).not.toContain(A);
      expect(onChange).toHaveBeenCalledWith("sign-in");
      await vi.waitFor(() => expect(verified).toBeNull());
      vi.resetModules();
      expect((await import("./session")).getIdentity()).toBeNull();
    });

    it("should stop naming the old account to Segment at once, and clear its cookie", async () => {
      const { trackedUserDropped } = await import("@/core/lib/telemetry/tracked-user");
      wallet.request.mockResolvedValue([B]);
      wallet.emit([B]);
      expect(trackedUserDropped()).toBe(true);
      await vi.waitFor(() => expect(callbacks).toEqual([{ signout: true }]));
      expect(verified).toBeNull();
    });

    it("should name the old account again once the wallet returns to it with its session", async () => {
      const { trackedUserDropped } = await import("@/core/lib/telemetry/tracked-user");
      wallet.request.mockResolvedValue([B]);
      wallet.emit([B]);
      await vi.waitFor(() => expect(verified).toBeNull());
      wallet.request.mockResolvedValue([A]);
      wallet.emit([A]);
      expect(session.getIdentity()?.signer).toBe(A);
      await vi.waitFor(() => expect(verified).toBe(A));
      expect(callbacks).toEqual([{ signout: true }, { signedBy: A }]);
      expect(trackedUserDropped()).toBe(false);
    });

    it("should keep page views from naming the old account when the clearing post never arrives", async () => {
      const { trackedUserDropped } = await import("@/core/lib/telemetry/tracked-user");
      server = "offline";
      wallet.request.mockResolvedValue([B]);
      wallet.emit([B]);
      await vi.waitFor(() => expect(callbacks).toEqual([{ signout: true }]));
      // The cookie could not be cleared; this document's page views leave the account out.
      expect(verified).toBe(A);
      expect(trackedUserDropped()).toBe(true);
    });
  });

  describe("and the provider stops exposing accounts", () => {
    it("should suspend the session until the wallet exposes the account again", async () => {
      wallet.emit([]);
      expect(session.getIdentity()).toBeNull();
      expect(onChange).not.toHaveBeenCalled();
      wallet.emit([A]);
      expect(session.getIdentity()?.signer).toBe(A);
      // Suspending cleared the cookie; the account that came back is proven again.
      await vi.waitFor(() => expect(verified).toBe(A));
      expect(callbacks).toEqual([{ signout: true }, { signedBy: A }]);
    });
  });

  describe("and the first account probe finds a locked wallet", () => {
    beforeEach(async () => {
      wallet.request.mockResolvedValueOnce([]);
      await vi.waitFor(() => expect(wallet.request).toHaveBeenCalledWith({ method: "eth_accounts" }));
    });

    it("should keep the valid session until the wallet emits a change", () => {
      expect(session.getIdentity()?.signer).toBe(A);
      expect(onChange).not.toHaveBeenCalled();
    });
  });

  describe("and the browser blocks session storage", () => {
    beforeEach(() => {
      vi.spyOn(Storage.prototype, "setItem").mockImplementationOnce(() => { throw new Error("Storage denied"); });
    });

    afterEach(() => vi.restoreAllMocks());

    it("should request sign-in without reactivating the old session", () => {
      wallet.emit([B]);
      expect(onChange).toHaveBeenCalledWith("sign-in");
      expect(session.getIdentity()).toBeNull();
    });
  });

  describe.each([
    { name: "a null payload", payload: null },
    { name: "a non-string account", payload: [42] },
    { name: "an invalid address", payload: ["0x01"] },
  ])("and the provider emits $name", ({ payload }) => {
    it("should leave the session alone", () => {
      wallet.emit(payload);
      expect(session.getIdentity()?.signer).toBe(A);
      expect(onChange).not.toHaveBeenCalled();
    });
  });

  describe("and Sites stops watching the wallet", () => {
    beforeEach(() => {
      stop?.();
      stop = undefined;
    });

    it("should ignore events from the previous subscription", () => {
      wallet.emit([B]);
      expect(session.getIdentity()?.signer).toBe(A);
      expect(onChange).not.toHaveBeenCalled();
    });
  });
});

describe("when a WalletConnect session is active", () => {
  beforeEach(() => {
    signIn.recordSignIn(A, "web3");
    session.setIdentity(identity(A));
    stop = watch(window, onChange);
  });

  it("should ignore the injected wallet", () => {
    wallet.emit([B]);
    expect(session.getIdentity()?.signer).toBe(A);
    expect(onChange).not.toHaveBeenCalled();
  });
});

describe("when another wallet is selected through EIP-6963", () => {
  let selected: ReturnType<typeof provider>;

  beforeEach(() => {
    selected = provider();
    signIn.recordSignIn(A, "web3", "io.test.wallet");
    session.setIdentity(identity(A));
    stop = watch(window, onChange);
    window.dispatchEvent(new CustomEvent("eip6963:announceProvider", {
      detail: { info: { rdns: "io.test.wallet" }, provider: selected },
    }));
  });

  it("should follow only the provider that signed in", () => {
    wallet.emit([B]);
    expect(session.getIdentity()?.signer).toBe(A);
    selected.emit([B]);
    expect(onChange).toHaveBeenCalledWith("sign-in");
  });
});

describe("when the sign-in predates browser wallet tracking", () => {
  beforeEach(() => {
    localStorage.setItem("dcl:auth:signin-kind:v1", JSON.stringify({ signer: A, kind: "web3" }));
    session.setIdentity(identity(A));
  });

  describe("and the injected wallet initially exposes the signed-in account", () => {
    beforeEach(async () => {
      stop = watch(window, onChange);
      await vi.waitFor(() => expect(wallet.request).toHaveBeenCalledWith({ method: "eth_accounts" }));
    });

    it("should follow a subsequent account change", () => {
      wallet.emit([B]);
      expect(onChange).toHaveBeenCalledWith("sign-in");
    });
  });

  describe("and the injected wallet exposes a different account", () => {
    beforeEach(async () => {
      wallet.request.mockResolvedValue([B]);
      stop = watch(window, onChange);
      await vi.waitFor(() => expect(wallet.request).toHaveBeenCalledWith({ method: "eth_accounts" }));
    });

    it("should leave the active session alone", () => {
      wallet.emit([B]);
      expect(session.getIdentity()?.signer).toBe(A);
      expect(onChange).not.toHaveBeenCalled();
    });
  });
});
