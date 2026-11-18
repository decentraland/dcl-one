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
const photos = vi.hoisted(() => ({ fetch: vi.fn() }));
vi.mock("@/data/lib/catalyst/social/photos", () => ({ fetchUserPhotos: photos.fetch }));

import type { AuthIdentity } from "@/data/lib/auth/types";
import { profileKeys } from "@/data/lib/profile/queries";
import {
  useMyCommunities,
  useProfile,
  useProfileAssets,
  useProfileCreations,
  useProfilePhotos,
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

beforeEach(() => {
  client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  fetchMock = vi.fn();
  vi.stubGlobal("fetch", fetchMock);
});
afterEach(() => {
  client.clear();
  vi.unstubAllGlobals();
  communities.fetch.mockReset();
  photos.fetch.mockReset();
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
  it("reads one category a page at a time and loads the next page after the rows already shown", async () => {
    const item = (n: number) => ({ id: `c-${n}`, itemId: `${n}`, name: `Hat ${n}`, thumbnail: "", contractAddress: "0xabc", rarity: "rare", network: "MATIC", price: "0", minListingPrice: null });
    fetchMock.mockImplementation(async (input: string) => {
      const skip = Number(new URL(input).searchParams.get("skip"));
      return json({ data: Array.from({ length: skip === 0 ? 24 : 1 }, (_, n) => item(skip + n)), total: 25 });
    });

    const { result } = renderHook(() => useProfileCreations(ADDRESS, "emote"), { wrapper });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    const first = new URL(fetchMock.mock.calls[0]![0] as string);
    expect(first.pathname).toBe("/marketplace-api/v2/catalog");
    expect(Object.fromEntries(first.searchParams)).toMatchObject({ creator: ADDRESS, category: "emote", first: "24", skip: "0", sortBy: "newest" });
    expect(result.current.hasNextPage).toBe(true);

    await result.current.fetchNextPage();

    await waitFor(() => expect(result.current.data?.pages).toHaveLength(2));
    expect(new URL(fetchMock.mock.calls[1]![0] as string).searchParams.get("skip")).toBe("24");
    expect(result.current.hasNextPage).toBe(false);
    expect(client.getQueryData(profileKeys.creations(ADDRESS, "emote"))).toBeDefined();
    expect(client.getQueryData(profileKeys.creations(ADDRESS, "wearable"))).toBeUndefined();
  });

  it("requests nothing for a value that is not a wallet address", () => {
    const { result } = renderHook(() => useProfileCreations("alice", "wearable"), { wrapper });

    expect(result.current.fetchStatus).toBe("idle");
    expect(fetchMock).not.toHaveBeenCalled();
  });
});

describe("useProfileAssets", () => {
  it("reads every category without a filter, and exposes a failure instead of an empty list", async () => {
    fetchMock.mockResolvedValue(json({ error: "down" }, 503));

    const { result } = renderHook(() => useProfileAssets(ADDRESS, null), { wrapper });

    await waitFor(() => expect(result.current.isError).toBe(true));
    expect(result.current.data).toBeUndefined();
    const url = new URL(fetchMock.mock.calls[0]![0] as string);
    expect(url.pathname).toBe("/marketplace-api/v1/nfts");
    expect(url.searchParams.get("owner")).toBe(ADDRESS);
    expect(url.searchParams.has("category")).toBe(false);
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

describe("useProfilePhotos", () => {
  const OWNER = SIGNER.toLowerCase();
  const row = (id: string, isPublic = true) => ({ id, isPublic });
  const page = (rows: unknown[]) => ({ items: rows, received: rows.length, total: rows.length });

  it("keeps the public photos on screen while the owner's signed list first loads", async () => {
    let release!: (value: unknown) => void;
    photos.fetch.mockImplementation((_address: string, _params: unknown, opts: { identity?: AuthIdentity }) =>
      opts.identity ? new Promise((resolve) => (release = resolve)) : Promise.resolve(page([row("pub")])),
    );
    const { result, rerender } = renderHook(({ id }) => useProfilePhotos(OWNER, id), {
      wrapper,
      initialProps: { id: null as AuthIdentity | null },
    });
    await waitFor(() => expect(result.current.data?.pages[0]?.items).toEqual([row("pub")]));

    rerender({ id: identity });

    expect(result.current.isPlaceholderData).toBe(true);
    expect(result.current.data?.pages[0]?.items).toEqual([row("pub")]);
    release(page([row("pub"), row("mine", false)]));
    await waitFor(() => expect(result.current.isPlaceholderData).toBe(false));
    expect(result.current.data?.pages[0]?.items).toEqual([row("pub"), row("mine", false)]);
  });

  it("never keeps the owner's private photos once the list is no longer signed", async () => {
    photos.fetch.mockImplementation((_address: string, _params: unknown, opts: { identity?: AuthIdentity }) =>
      opts.identity ? Promise.resolve(page([row("mine", false)])) : new Promise(() => {}),
    );
    const { result, rerender } = renderHook(({ id }) => useProfilePhotos(OWNER, id), {
      wrapper,
      initialProps: { id: identity as AuthIdentity | null },
    });
    await waitFor(() => expect(result.current.data?.pages[0]?.items).toEqual([row("mine", false)]));

    rerender({ id: null });

    expect(result.current.data).toBeUndefined();
    expect(result.current.isPending).toBe(true);
  });

  it("does not carry one profile's photos over to another's", async () => {
    photos.fetch.mockImplementation((address: string) =>
      address === OWNER ? Promise.resolve(page([row("a")])) : new Promise(() => {}),
    );
    const { result, rerender } = renderHook(({ address }) => useProfilePhotos(address, null), {
      wrapper,
      initialProps: { address: OWNER },
    });
    await waitFor(() => expect(result.current.data).toBeDefined());

    rerender({ address: ADDRESS });

    expect(result.current.data).toBeUndefined();
  });

  it("requests nothing for a value that is not a wallet address", () => {
    const { result } = renderHook(() => useProfilePhotos("alice", null), { wrapper });

    expect(result.current.fetchStatus).toBe("idle");
    expect(photos.fetch).not.toHaveBeenCalled();
  });
});
