// @vitest-environment jsdom
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import {
  QueryClient,
  QueryClientProvider,
  useQuery,
} from "@tanstack/react-query";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() => {
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshSig$: () => (type: unknown) => type,
    $RefreshReg$: () => {},
  });
});
import { usePrivateQueryLifecycle } from "./usePrivateQueryLifecycle";

// vitest globals are off in this app, so Testing Library cannot register its own cleanup.
afterEach(() => cleanup());

describe("when the Sites host changes authenticated identity", () => {
  let client: QueryClient;
  let release: (value: string) => void;
  let signal: AbortSignal | undefined;
  let wrapper: ({ children }: { children: ReactNode }) => ReactNode;
  beforeEach(() => {
    client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    release = () => undefined;
    signal = undefined;
    client.setQueryData(["blog", "categories"], ["Announcements"]);
    client.setQueryData(["private", "0xa", "cached"], "old private value");
    client.setQueryData(["legacy", "0xa"], "legacy owner retains this entry");
    wrapper = ({ children }) => (
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    );
  });
  it("should abort and remove the previous account while retaining public and current-account data", async () => {
    const { rerender, result } = renderHook(
      ({ account }: { account: string }) => {
        usePrivateQueryLifecycle(account);
        return useQuery({
          queryKey: ["private", account.toLowerCase(), "request"],
          queryFn: ({ signal: requestSignal }) => {
            if (account === "0xb") return Promise.resolve("new account");
            signal = requestSignal;
            return new Promise<string>((resolve) => {
              release = resolve;
            });
          },
        });
      },
      { initialProps: { account: "0xA" }, wrapper },
    );
    await waitFor(() => expect(signal).toBeDefined());
    rerender({ account: "0xb" });
    expect(signal?.aborted).toBe(true);
    await waitFor(() => expect(result.current.data).toBe("new account"));
    await act(async () => {
      release("late old account");
      await Promise.resolve();
    });
    expect(client.getQueriesData({ queryKey: ["private", "0xa"] })).toEqual([]);
    expect(client.getQueryData(["blog", "categories"])).toEqual([
      "Announcements",
    ]);
    expect(client.getQueryData(["private", "0xb", "request"])).toBe(
      "new account",
    );
    expect(client.getQueryData(["legacy", "0xa"])).toBe(
      "legacy owner retains this entry",
    );
  });
  it("should keep every entry when only the address's letter case changes", () => {
    const { rerender } = renderHook(
      ({ account }: { account: string | null }) =>
        usePrivateQueryLifecycle(account),
      { initialProps: { account: "0xa" as string | null }, wrapper },
    );
    rerender({ account: "0xA" });
    expect(client.getQueryData(["private", "0xa", "cached"])).toBe(
      "old private value",
    );
  });
  it("should keep every entry when a visitor signs in", () => {
    const { rerender } = renderHook(
      ({ account }: { account: string | null }) =>
        usePrivateQueryLifecycle(account),
      { initialProps: { account: null as string | null }, wrapper },
    );
    rerender({ account: "0xa" });
    expect(client.getQueryData(["private", "0xa", "cached"])).toBe(
      "old private value",
    );
    expect(client.getQueryData(["blog", "categories"])).toEqual([
      "Announcements",
    ]);
  });
  it("should treat an empty address as signed out", () => {
    const { rerender } = renderHook(
      ({ account }: { account: string | null }) =>
        usePrivateQueryLifecycle(account),
      { initialProps: { account: "0xa" as string | null }, wrapper },
    );
    rerender({ account: "" });
    expect(client.getQueriesData({ queryKey: ["private"] })).toEqual([]);
  });
  it("should remove every declared private account on sign-out and leave public data intact", () => {
    client.setQueryData(["private", "0xb", "cached"], "another private value");
    const { rerender } = renderHook(
      ({ account }: { account: string | null }) =>
        usePrivateQueryLifecycle(account),
      { initialProps: { account: "0xa" as string | null }, wrapper },
    );
    rerender({ account: null });
    expect(client.getQueriesData({ queryKey: ["private"] })).toEqual([]);
    expect(client.getQueryData(["blog", "categories"])).toEqual([
      "Announcements",
    ]);
  });
});
