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
  let aborted: boolean | undefined;
  beforeEach(() => {
    client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    release = () => undefined;
    signal = undefined;
    aborted = undefined;
    client.setQueryData(["blog", "categories"], ["Announcements"]);
    client.setQueryData(["private", "0xa", "cached"], "old private value");
    client.setQueryData(["legacy", "0xa"], "legacy owner retains this entry");
    wrapper = ({ children }) => (
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    );
  });
  function renderLifecycle(account: string | null) {
    return renderHook(
      ({ account }: { account: string | null }) =>
        usePrivateQueryLifecycle(account),
      { initialProps: { account }, wrapper },
    );
  }

  describe("and another account replaces one with a request in flight", () => {
    beforeEach(async () => {
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
      aborted = signal?.aborted;
      await waitFor(() => expect(result.current.data).toBe("new account"));
      await act(async () => {
        release("late old account");
        await Promise.resolve();
      });
    });

    it("should abort the previous account's request", () => {
      expect(aborted).toBe(true);
    });

    it("should remove the previous account's private entries", () => {
      expect(client.getQueriesData({ queryKey: ["private", "0xa"] })).toEqual([]);
    });

    it("should retain public data", () => {
      expect(client.getQueryData(["blog", "categories"])).toEqual([
        "Announcements",
      ]);
    });

    it("should retain the current account's data", () => {
      expect(client.getQueryData(["private", "0xb", "request"])).toBe(
        "new account",
      );
    });

    it("should leave non-private entries to their owner", () => {
      expect(client.getQueryData(["legacy", "0xa"])).toBe(
        "legacy owner retains this entry",
      );
    });
  });

  describe("and only the address's letter case changes", () => {
    beforeEach(() => {
      renderLifecycle("0xa").rerender({ account: "0xA" });
    });

    it("should keep every entry", () => {
      expect(client.getQueryData(["private", "0xa", "cached"])).toBe(
        "old private value",
      );
    });
  });

  describe("and a visitor signs in", () => {
    beforeEach(() => {
      renderLifecycle(null).rerender({ account: "0xa" });
    });

    it("should keep the private entries", () => {
      expect(client.getQueryData(["private", "0xa", "cached"])).toBe(
        "old private value",
      );
    });

    it("should keep public data", () => {
      expect(client.getQueryData(["blog", "categories"])).toEqual([
        "Announcements",
      ]);
    });
  });

  describe("and the address becomes empty", () => {
    beforeEach(() => {
      renderLifecycle("0xa").rerender({ account: "" });
    });

    it("should treat it as signed out and remove every private entry", () => {
      expect(client.getQueriesData({ queryKey: ["private"] })).toEqual([]);
    });
  });

  describe("and the account signs out", () => {
    beforeEach(() => {
      client.setQueryData(["private", "0xb", "cached"], "another private value");
      renderLifecycle("0xa").rerender({ account: null });
    });

    it("should remove every declared private account", () => {
      expect(client.getQueriesData({ queryKey: ["private"] })).toEqual([]);
    });

    it("should leave public data intact", () => {
      expect(client.getQueryData(["blog", "categories"])).toEqual([
        "Announcements",
      ]);
    });
  });
});
