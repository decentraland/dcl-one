// @vitest-environment jsdom
import { renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() => {
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshSig$: () => (type: unknown) => type,
    $RefreshReg$: () => {},
  });
});
const state = vi.hoisted(() => ({ auth: { identity: null as unknown, status: "anonymous" }, openSignIn: vi.fn() }));
vi.mock("@/data/lib/auth/index", () => ({ useAuth: () => state.auth }));
vi.mock("../components/auth/signin-store", () => ({ openSignIn: state.openSignIn }));

import { useAccountIdentity } from "./useAccountIdentity";

afterEach(() => {
  state.auth = { identity: null, status: "anonymous" };
  vi.resetAllMocks();
});

describe("useAccountIdentity", () => {
  let result: { current: ReturnType<typeof useAccountIdentity> };

  describe("when the session is signed in", () => {
    let identity: { signer: string; ephemeral: object; expiration: string; authChain: unknown[] };

    beforeEach(() => {
      identity = { signer: "0x4e9c4a2502fdf71e93ed8ed6ca9ddbd891d6f295", ephemeral: {}, expiration: "2999-01-01T00:00:00.000Z", authChain: [] };
      state.auth = { identity, status: "connected" };
      ({ result } = renderHook(() => useAccountIdentity()));
    });

    it("should hand the signed-in identity to the page", () => {
      expect(result.current).toEqual({ state: "signed-in", identity });
    });

    it("should not ask to sign in", () => {
      expect(state.openSignIn).not.toHaveBeenCalled();
    });
  });

  describe("when the browser has no session", () => {
    beforeEach(() => {
      ({ result } = renderHook(() => useAccountIdentity()));
    });

    it("should report the signed-out state", () => {
      expect(result.current.state).toBe("signed-out");
    });

    it("should open the sign-in once", () => {
      expect(state.openSignIn).toHaveBeenCalledTimes(1);
    });
  });

  describe("when a sign-in is still connecting", () => {
    beforeEach(() => {
      state.auth = { identity: null, status: "connecting" };
      ({ result } = renderHook(() => useAccountIdentity()));
    });

    it("should report the checking state", () => {
      expect(result.current.state).toBe("checking");
    });

    it("should not ask to sign in", () => {
      expect(state.openSignIn).not.toHaveBeenCalled();
    });
  });
});
