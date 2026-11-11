// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() => {
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshSig$: () => (type: unknown) => type,
    $RefreshReg$: () => {},
  });
});

import { useBlogSearchSuggestions } from "./useBlogSearchSuggestions";

afterEach(() => {
  vi.unstubAllGlobals();
});

const answer = (query: string) => ({ query, total: 1, posts: [{ slug: `${query}-post`, href: `/blog/x/${query}-post`, title: query, description: "", image: null, hue: 0 }] });

function setup(fetchImpl: (url: string, init?: RequestInit) => Promise<Response>) {
  const fetchMock = vi.fn((input: string, init?: RequestInit) => fetchImpl(input, init));
  vi.stubGlobal("fetch", fetchMock);
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  const hook = renderHook(({ input }: { input: string }) => useBlogSearchSuggestions(input, 10), { wrapper, initialProps: { input: "" } });
  return { fetchMock, hook };
}

const ok = (url: string) => Promise.resolve(Response.json(answer(new URL(url, "https://sites.test").searchParams.get("q") ?? "")));

describe("useBlogSearchSuggestions", () => {
  it("asks once typing pauses, with the trimmed query, and returns its suggestions", async () => {
    const { fetchMock, hook } = setup(ok);
    hook.rerender({ input: "  decen " });
    await waitFor(() => expect(hook.result.current.suggestions?.query).toBe("decen"));
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(fetchMock.mock.calls[0]![0]).toBe("/api/blog/search?q=decen");
    expect(hook.result.current).toMatchObject({ query: "decen", loading: false, failed: false });
  });

  it("asks nothing under 3 characters and hides earlier suggestions at once", async () => {
    const { fetchMock, hook } = setup(ok);
    hook.rerender({ input: "ab" });
    await new Promise((r) => setTimeout(r, 30));
    expect(fetchMock).not.toHaveBeenCalled();
    hook.rerender({ input: "abc" });
    await waitFor(() => expect(hook.result.current.suggestions).not.toBeNull());
    hook.rerender({ input: "ab" });
    expect(hook.result.current).toEqual({ query: "", suggestions: null, loading: false, failed: false });
  });

  it("never shows an older query's late answer over the newer one, and aborts it", async () => {
    let releaseFirst!: () => void;
    const signals: AbortSignal[] = [];
    const { hook } = setup((url, init) => {
      signals.push(init!.signal!);
      if (url.endsWith("q=gen")) return new Promise((resolve) => (releaseFirst = () => resolve(Response.json(answer("gen")))));
      return ok(url);
    });
    hook.rerender({ input: "gen" });
    await waitFor(() => expect(signals).toHaveLength(1));
    hook.rerender({ input: "genesis" });
    await waitFor(() => expect(hook.result.current.suggestions?.query).toBe("genesis"));
    await act(async () => releaseFirst());
    expect(hook.result.current.suggestions?.query).toBe("genesis");
  });

  it("reports a failed request without throwing", async () => {
    const { hook } = setup(() => Promise.resolve(new Response("{}", { status: 503 })));
    hook.rerender({ input: "news" });
    await waitFor(() => expect(hook.result.current.failed).toBe(true));
    expect(hook.result.current.loading).toBe(false);
  });

  it("debounces: fast typing sends only the last query", async () => {
    const { fetchMock, hook } = setup(ok);
    for (const input of ["dec", "dece", "decen"]) hook.rerender({ input });
    await waitFor(() => expect(hook.result.current.suggestions?.query).toBe("decen"));
    expect(fetchMock.mock.calls.map(([u]) => u)).toEqual(["/api/blog/search?q=decen"]);
  });
});
