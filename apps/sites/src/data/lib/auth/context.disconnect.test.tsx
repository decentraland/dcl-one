// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() => {
  Object.assign(window, { __vite_plugin_react_preamble_installed__: true, $RefreshSig$: () => (type: unknown) => type, $RefreshReg$: () => {} });
});
const magic = vi.hoisted(() => ({ end: vi.fn(async () => {}) }));
vi.mock("./magic-session", () => ({ endMagicSession: magic.end }));

import { useAuth } from "./context";
import { getIdentity, setIdentity } from "./session";

const SIGNER = "0x4e9c4a2502fdf71e93ed8ed6ca9ddbd891d6f295";

beforeEach(() => {
  vi.stubGlobal("fetch", vi.fn(async () => new Response(null, { status: 204 })));
  setIdentity({
    signer: SIGNER,
    ephemeral: { address: `0x${"c".repeat(40)}`, privateKey: `0x${"12".repeat(32)}`, publicKey: `0x04${"ab".repeat(64)}` },
    expiration: new Date(Date.now() + 86_400_000).toISOString(),
    authChain: [{ type: "SIGNER", payload: SIGNER, signature: "" }],
  });
});
afterEach(() => {
  vi.unstubAllGlobals();
  vi.useRealTimers();
  vi.clearAllMocks();
  localStorage.clear();
});

describe("signing out", () => {
  it("ends this site's session at once and Magic's with it, resolving once Magic has logged out", async () => {
    let finishLogout!: () => void;
    magic.end.mockImplementationOnce(() => new Promise<void>((resolve) => (finishLogout = resolve)));
    const { result } = renderHook(() => useAuth());
    let done = false;
    let signedOut!: Promise<void>;
    act(() => {
      signedOut = result.current.disconnect().then(() => {
        done = true;
      });
    });
    expect(getIdentity()).toBeNull();
    await vi.waitFor(() => expect(magic.end).toHaveBeenCalledOnce());
    expect(done).toBe(false);
    finishLogout();
    await signedOut;
    expect(done).toBe(true);
  });

  it("never keeps a caller waiting on a Magic that does not answer", async () => {
    vi.useFakeTimers();
    magic.end.mockImplementationOnce(() => new Promise<void>(() => {}));
    const { result } = renderHook(() => useAuth());
    let done = false;
    act(() => {
      void result.current.disconnect().then(() => {
        done = true;
      });
    });
    await vi.advanceTimersByTimeAsync(2_000);
    expect(done).toBe(true);
    expect(getIdentity()).toBeNull();
  });
});
