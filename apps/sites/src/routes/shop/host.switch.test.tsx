// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import type { ReactNode } from "react";
import { MemoryRouter } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() => {
  Object.assign(window, { __vite_plugin_react_preamble_installed__: true, $RefreshSig$: () => (type: unknown) => type, $RefreshReg$: () => {} });
});
const auth = vi.hoisted(() => ({ finish: null as null | (() => void), disconnect: vi.fn(), openSignIn: vi.fn() }));
vi.mock("@/data/lib/auth/index", () => ({
  useAuth: () => ({ identity: null, disconnect: auth.disconnect }),
}));
vi.mock("@/features/components/auth/signin-store", () => ({ openSignIn: auth.openSignIn }));

import { useSitesShopHost } from "./host";

afterEach(() => vi.clearAllMocks());

describe("switching accounts in the Shop", () => {
  it("sends the buyer to sign in only once the sign-out (Magic's included) has finished", async () => {
    auth.disconnect.mockImplementationOnce(() => new Promise<void>((resolve) => (auth.finish = resolve)));
    const wrapper = ({ children }: { children: ReactNode }) => <MemoryRouter initialEntries={["/shop/items"]}>{children}</MemoryRouter>;
    const { result } = renderHook(() => useSitesShopHost(), { wrapper });
    act(() => result.current.onSwitchAccount?.());
    expect(auth.disconnect).toHaveBeenCalledOnce();
    expect(auth.openSignIn).not.toHaveBeenCalled();
    await act(async () => auth.finish!());
    expect(auth.openSignIn).toHaveBeenCalledWith({ redirectTo: "/shop/items" });
  });
});
