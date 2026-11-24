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

beforeEach(async () => {
  vi.resetModules();
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

    it("should activate the selected account and update the wallet cookie", () => {
      wallet.emit([B.toUpperCase().replace("0X", "0x")]);
      expect(session.getIdentity()?.signer).toBe(B);
      expect(document.cookie).toContain(B);
      expect(onChange).toHaveBeenCalledWith("activated");
    });

    describe("and the reader returns to the previous account", () => {
      beforeEach(() => {
        wallet.emit([B]);
        onChange.mockClear();
      });

      it("should reuse the saved session and expose it as the Explorer's sign-in", async () => {
        wallet.emit([A]);
        expect(session.getIdentity()?.signer).toBe(A);
        expect(onChange).toHaveBeenCalledExactlyOnceWith("activated");
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
      wallet.emit([B]);
      expect(session.getIdentity()).toBeNull();
      expect(document.cookie).not.toContain(A);
      expect(onChange).toHaveBeenCalledWith("sign-in");
      vi.resetModules();
      expect((await import("./session")).getIdentity()).toBeNull();
    });
  });

  describe("and the provider stops exposing accounts", () => {
    it("should suspend the session until the wallet exposes the account again", () => {
      wallet.emit([]);
      expect(session.getIdentity()).toBeNull();
      expect(onChange).not.toHaveBeenCalled();
      wallet.emit([A]);
      expect(session.getIdentity()?.signer).toBe(A);
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
