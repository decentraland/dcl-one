// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() => {
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshSig$: () => (type: unknown) => type,
    $RefreshReg$: () => {},
  });
});

import { useBlogSearchSuggestions } from "./useBlogSearchSuggestions";

afterEach(() => {
  vi.resetAllMocks();
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

const answer = (query: string) => ({ query, total: 1, posts: [{ slug: `${query}-post`, href: `/blog/x/${query}-post`, title: query, description: "", image: null, hue: 0 }] });

type FetchImpl = (url: string, init?: RequestInit) => Promise<Response>;

function setup(fetchImpl: FetchImpl) {
  const fetchMock = vi.fn((input: string, init?: RequestInit) => fetchImpl(input, init));
  vi.stubGlobal("fetch", fetchMock);
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  const hook = renderHook(({ input }: { input: string }) => useBlogSearchSuggestions(input, 10), { wrapper, initialProps: { input: "" } });
  return { fetchMock, hook };
}

const ok = (url: string) => Promise.resolve(Response.json(answer(new URL(url, "https://sites.test").searchParams.get("q") ?? "")));

describe("useBlogSearchSuggestions", () => {
  let fetchMock: ReturnType<typeof setup>["fetchMock"];
  let hook: ReturnType<typeof setup>["hook"];

  describe("when typing pauses on a padded query", () => {
    beforeEach(async () => {
      ({ fetchMock, hook } = setup(ok));
      hook.rerender({ input: "  decen " });
      await waitFor(() => expect(hook.result.current.suggestions?.query).toBe("decen"));
    });

    it("should ask once, with the trimmed query", () => {
      expect(fetchMock).toHaveBeenCalledTimes(1);
      expect(fetchMock.mock.calls[0]![0]).toBe("/api/blog/search?q=decen");
    });

    it("should return the trimmed query's suggestions, loaded and not failed", () => {
      expect(hook.result.current).toMatchObject({ query: "decen", loading: false, failed: false });
    });
  });

  describe("when the query is under 3 characters", () => {
    beforeEach(async () => {
      vi.useFakeTimers();
      ({ fetchMock, hook } = setup(ok));
      hook.rerender({ input: "ab" });
      await act(() => vi.advanceTimersByTimeAsync(10));
    });

    it("should ask nothing", () => {
      expect(fetchMock).not.toHaveBeenCalled();
    });

    it("should not report loading", () => {
      expect(hook.result.current.loading).toBe(false);
    });

    describe("and it grows to 3 characters", () => {
      beforeEach(async () => {
        vi.useRealTimers();
        hook.rerender({ input: "abc" });
        await waitFor(() => expect(hook.result.current.suggestions).not.toBeNull());
      });

      it("should ask only for the 3-character query", () => {
        expect(fetchMock.mock.calls.map(([url]) => url)).toEqual(["/api/blog/search?q=abc"]);
      });

      describe("and it shrinks back under 3 characters", () => {
        beforeEach(() => {
          hook.rerender({ input: "ab" });
        });

        it("should hide the earlier suggestions at once", () => {
          expect(hook.result.current).toEqual({ query: "", suggestions: null, loading: false, failed: false });
        });
      });
    });
  });

  describe("when an older query answers late, after a newer one", () => {
    let releaseFirst: () => void;
    let signals: AbortSignal[];

    beforeEach(async () => {
      releaseFirst = () => undefined;
      signals = [];
      ({ hook } = setup((url, init) => {
        signals.push(init!.signal!);
        if (url.endsWith("q=gen")) return new Promise((resolve) => (releaseFirst = () => resolve(Response.json(answer("gen")))));
        return ok(url);
      }));
      hook.rerender({ input: "gen" });
      await waitFor(() => expect(signals).toHaveLength(1));
      hook.rerender({ input: "genesis" });
      await waitFor(() => expect(hook.result.current.suggestions?.query).toBe("genesis"));
    });

    it("should abort the older request", () => {
      expect(signals[0]!.aborted).toBe(true);
    });

    describe("and the older answer arrives", () => {
      beforeEach(async () => {
        await act(async () => releaseFirst());
      });

      it("should keep showing the newer answer", () => {
        expect(hook.result.current.suggestions?.query).toBe("genesis");
      });
    });
  });

  describe("when the request fails", () => {
    beforeEach(async () => {
      ({ hook } = setup(() => Promise.resolve(new Response("{}", { status: 503 }))));
      hook.rerender({ input: "news" });
    });

    it("should report the failure without throwing and stop loading", async () => {
      await waitFor(() => expect(hook.result.current.failed).toBe(true));
      expect(hook.result.current.loading).toBe(false);
    });
  });

  describe("when the user types fast", () => {
    beforeEach(async () => {
      ({ fetchMock, hook } = setup(ok));
      for (const input of ["dec", "dece", "decen"]) hook.rerender({ input });
      await waitFor(() => expect(hook.result.current.suggestions?.query).toBe("decen"));
    });

    it("should send only the last query", () => {
      expect(fetchMock.mock.calls.map(([u]) => u)).toEqual(["/api/blog/search?q=decen"]);
    });
  });
});
