// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() => {
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshSig$: () => (type: unknown) => type,
    $RefreshReg$: () => {},
  });
});
const communities = vi.hoisted(() => ({ fetch: vi.fn() }));
vi.mock("@/data/lib/catalyst/social/profile-communities", () => ({
  fetchMyCommunities: communities.fetch,
}));

import type { AuthIdentity } from "@/data/lib/auth/types";
import { profileKeys } from "@/data/lib/profile/queries";
import {
  useMyCommunities,
  useProfile,
  useProfileAssets,
  useProfileCreations,
} from "./hooks";

const ADDRESS = "0x17a253c2ac0d5ba92cadbbf665e3390c9913dc5d";
const SIGNER = "0x4E9C4A2502FDF71E93ED8ED6CA9DDBD891D6F295";

const identity: AuthIdentity = {
  signer: SIGNER,
  ephemeral: { address: `0x${"c".repeat(40)}`, privateKey: `0x${"12".repeat(32)}` },
  expiration: new Date(Date.now() + 86_400_000).toISOString(),
  authChain: [],
};

let fetchMock: ReturnType<typeof vi.fn>;
let client: QueryClient;

function wrapper({ children }: { children: ReactNode }) {
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}

const json = (body: unknown, status = 200) => new Response(JSON.stringify(body), { status });
const assetsPage = (elements: unknown[]) =>
  json({ ok: true, data: { elements, total: elements.length, page: 0, pages: 1, limit: 48 } });

beforeEach(() => {
  client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  fetchMock = vi.fn();
  vi.stubGlobal("fetch", fetchMock);
});
afterEach(() => {
  client.clear();
  vi.unstubAllGlobals();
  communities.fetch.mockReset();
});

describe("useProfile", () => {
  it("requests nothing for a value that is not a wallet address", () => {
    const { result } = renderHook(() => useProfile("alice"), { wrapper });

    expect(result.current.fetchStatus).toBe("idle");
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("exposes the server error instead of an empty profile", async () => {
    fetchMock.mockResolvedValue(json({ error: "down" }, 503));

    const { result } = renderHook(() => useProfile(ADDRESS), { wrapper });

    await waitFor(() => expect(result.current.isError).toBe(true));
    expect(result.current.data).toBeUndefined();
  });
});

describe("useProfileCreations", () => {
  it("relabels every creation with the profile's name without changing the cached value", async () => {
    fetchMock.mockResolvedValue(json({ ok: true, data: [] }));

    const { result } = renderHook(() => useProfileCreations(ADDRESS, "Alice"), { wrapper });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(result.current.data).toEqual({ wearables: [], emotes: [] });
    expect(client.getQueryData(profileKeys.creations(ADDRESS))).toEqual({ wearables: [], emotes: [] });
  });
});

describe("useProfileAssets", () => {
  it("shows the names when the wearables fail, and reports the failure", async () => {
    fetchMock.mockImplementation(async (input: string) => {
      const path = new URL(input).pathname;
      if (path.endsWith("/wearables")) return json({ error: "down" }, 500);
      return assetsPage([{ tokenId: "1", name: "alice", contractAddress: "0x0" }]);
    });

    const { result } = renderHook(() => useProfileAssets(ADDRESS), { wrapper });

    await waitFor(() => expect(result.current.isPending).toBe(false));
    expect(result.current.isError).toBe(true);
    expect(result.current.data.wearables).toEqual([]);
    expect(result.current.data.names).toEqual([{ id: "1", stem: "alice" }]);
  });

  it("is empty and not pending for an address that cannot have assets", () => {
    const { result } = renderHook(() => useProfileAssets(""), { wrapper });

    expect(result.current).toEqual({
      data: { wearables: [], names: [] },
      isPending: false,
      isError: false,
    });
    expect(fetchMock).not.toHaveBeenCalled();
  });
});

describe("useMyCommunities", () => {
  it("requests nothing without an identity", () => {
    const { result } = renderHook(() => useMyCommunities(null), { wrapper });

    expect(result.current.fetchStatus).toBe("idle");
    expect(communities.fetch).not.toHaveBeenCalled();
  });

  it("requests nothing when the page is not the member's own", () => {
    const { result } = renderHook(() => useMyCommunities(identity, false), { wrapper });

    expect(result.current.fetchStatus).toBe("idle");
    expect(communities.fetch).not.toHaveBeenCalled();
  });

  it("caches the list under the signer's private prefix", async () => {
    const row = { id: "c1", name: "Builders", membersCount: 3, role: "owner", thumb: null };
    communities.fetch.mockResolvedValue([row]);

    const { result } = renderHook(() => useMyCommunities(identity), { wrapper });

    await waitFor(() => expect(result.current.data).toEqual([row]));
    expect(communities.fetch).toHaveBeenCalledWith(identity, { signal: expect.any(AbortSignal) });
    const key = ["private", SIGNER.toLowerCase(), "profile", "communities"];
    expect(client.getQueryData(key)).toEqual([row]);

    client.removeQueries({ queryKey: ["private", SIGNER.toLowerCase()] });
    expect(client.getQueryData(key)).toBeUndefined();
  });

  it("reports a failed load as an error with no rows", async () => {
    communities.fetch.mockRejectedValue(new Error("401"));

    const { result } = renderHook(() => useMyCommunities(identity), { wrapper });

    await waitFor(() => expect(result.current.isError).toBe(true));
    expect(result.current.data).toBeUndefined();
  });
});
