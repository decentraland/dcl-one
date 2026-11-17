// @vitest-environment jsdom
import { renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() => {
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshSig$: () => (type: unknown) => type,
    $RefreshReg$: () => {},
  });
});
const state = vi.hoisted(() => ({ auth: { identity: null as unknown, status: "anonymous" }, openSignIn: vi.fn() }));
vi.mock("@/data/lib/auth/index", () => ({ useAuth: () => state.auth }));
vi.mock("@/features/components/auth/signin-store", () => ({ openSignIn: state.openSignIn }));

import { useAccountIdentity } from "./useAccountIdentity";

const identity = { signer: "0x4e9c4a2502fdf71e93ed8ed6ca9ddbd891d6f295", ephemeral: {}, expiration: "2999-01-01T00:00:00.000Z", authChain: [] };

afterEach(() => {
  state.auth = { identity: null, status: "anonymous" };
  vi.resetAllMocks();
});

describe("useAccountIdentity", () => {
  it("hands a signed-in session to the page without asking to sign in", () => {
    state.auth = { identity, status: "connected" };
    const { result } = renderHook(() => useAccountIdentity());

    expect(result.current).toEqual({ state: "signed-in", identity });
    expect(state.openSignIn).not.toHaveBeenCalled();
  });

  it("sends a browser without a session to sign in", () => {
    const { result } = renderHook(() => useAccountIdentity());

    expect(result.current.state).toBe("signed-out");
    expect(state.openSignIn).toHaveBeenCalledTimes(1);
  });

  it("waits while a sign-in is still connecting", () => {
    state.auth = { identity: null, status: "connecting" };
    const { result } = renderHook(() => useAccountIdentity());

    expect(result.current.state).toBe("checking");
    expect(state.openSignIn).not.toHaveBeenCalled();
  });
});
