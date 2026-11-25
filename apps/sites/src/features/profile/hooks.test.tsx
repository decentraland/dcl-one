// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, type Mock, vi } from "vitest";

vi.hoisted(() => {
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshSig$: () => (type: unknown) => type,
    $RefreshReg$: () => {},
  });
});
const communities = vi.hoisted(() => ({ mine: vi.fn(), member: vi.fn() }));
vi.mock("@/data/lib/catalyst/social/profile-communities", () => ({
  fetchMyCommunities: communities.mine,
  fetchMemberCommunities: communities.member,
}));
const photos = vi.hoisted(() => ({ fetch: vi.fn() }));
vi.mock("@/data/lib/catalyst/social/photos", () => ({ fetchUserPhotos: photos.fetch }));

import type { AuthIdentity } from "@/data/lib/auth/types";
import { profileKeys } from "@/data/lib/profile/queries";
import {
  useProfile,
  useProfileAssets,
  useProfileCommunities,
  useProfileCreations,
  useProfilePhotos,
} from "./hooks";

const ADDRESS = "0x17a253c2ac0d5ba92cadbbf665e3390c9913dc5d";
const SIGNER = "0x4E9C4A2502FDF71E93ED8ED6CA9DDBD891D6F295";

let identity: AuthIdentity;
let fetchMock: Mock;
let client: QueryClient;

function wrapper({ children }: { children: ReactNode }) {
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}

const json = (body: unknown, status = 200) => new Response(JSON.stringify(body), { status });

beforeEach(() => {
  identity = {
    signer: SIGNER,
    ephemeral: { address: `0x${"c".repeat(40)}`, privateKey: `0x${"12".repeat(32)}` },
    expiration: new Date(Date.now() + 86_400_000).toISOString(),
    authChain: [],
  };
  client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  fetchMock = vi.fn();
  vi.stubGlobal("fetch", fetchMock);
});

afterEach(() => {
  cleanup();
  client.clear();
  vi.unstubAllGlobals();
  vi.resetAllMocks();
});

describe("useProfile", () => {
  describe("when the value is not a wallet address", () => {
    let result: { current: ReturnType<typeof useProfile> };

    beforeEach(() => {
      ({ result } = renderHook(() => useProfile("alice"), { wrapper }));
    });

    it("should stay idle", () => {
      expect(result.current.fetchStatus).toBe("idle");
    });

    it("should request nothing", () => {
      expect(fetchMock).not.toHaveBeenCalled();
    });
  });

  describe("when the server fails", () => {
    let result: { current: ReturnType<typeof useProfile> };

    beforeEach(async () => {
      fetchMock.mockResolvedValueOnce(json({ error: "down" }, 503));
      ({ result } = renderHook(() => useProfile(ADDRESS), { wrapper }));
      await waitFor(() => expect(result.current.isError).toBe(true));
    });

    it("should expose the server error instead of an empty profile", () => {
      expect(result.current.data).toBeUndefined();
    });
  });
});

describe("useProfileCreations", () => {
  describe("when a category has more rows than a page", () => {
    let result: { current: ReturnType<typeof useProfileCreations> };

    beforeEach(async () => {
      const item = (n: number) => ({ id: `c-${n}`, itemId: `${n}`, name: `Hat ${n}`, thumbnail: "", contractAddress: "0xabc", rarity: "rare", network: "MATIC", price: "0", minListingPrice: null });
      fetchMock.mockImplementation(async (input: string) => {
        const skip = Number(new URL(input).searchParams.get("skip"));
        return json({ data: Array.from({ length: skip === 0 ? 24 : 1 }, (_, n) => item(skip + n)), total: 25 });
      });
      ({ result } = renderHook(() => useProfileCreations(ADDRESS, "emote"), { wrapper }));
      await waitFor(() => expect(result.current.isSuccess).toBe(true));
    });

    it("should read the catalog", () => {
      expect(new URL(fetchMock.mock.calls[0]![0] as string).pathname).toBe("/marketplace-api/v2/catalog");
    });

    it("should read one category a page at a time, newest first", () => {
      expect(Object.fromEntries(new URL(fetchMock.mock.calls[0]![0] as string).searchParams)).toMatchObject({ creator: ADDRESS, category: "emote", first: "24", skip: "0", sortBy: "newest" });
    });

    it("should report a next page", () => {
      expect(result.current.hasNextPage).toBe(true);
    });

    describe("and the next page is loaded", () => {
      beforeEach(async () => {
        // The hook re-renders only for the result fields read so far, so read the pages before asking for more.
        void result.current.data;
        await result.current.fetchNextPage();
        await waitFor(() => expect(result.current.data?.pages).toHaveLength(2));
      });

      it("should load it after the rows already shown", () => {
        expect(new URL(fetchMock.mock.calls[1]![0] as string).searchParams.get("skip")).toBe("24");
      });

      it("should report no further page", () => {
        expect(result.current.hasNextPage).toBe(false);
      });

      it("should cache the rows under the requested category", () => {
        expect(client.getQueryData(profileKeys.creations(ADDRESS, "emote"))).toBeDefined();
      });

      it("should not cache rows under another category", () => {
        expect(client.getQueryData(profileKeys.creations(ADDRESS, "wearable"))).toBeUndefined();
      });
    });
  });

  describe("when the value is not a wallet address", () => {
    let result: { current: ReturnType<typeof useProfileCreations> };

    beforeEach(() => {
      ({ result } = renderHook(() => useProfileCreations("alice", "wearable"), { wrapper }));
    });

    it("should stay idle", () => {
      expect(result.current.fetchStatus).toBe("idle");
    });

    it("should request nothing", () => {
      expect(fetchMock).not.toHaveBeenCalled();
    });
  });
});

describe("useProfileAssets", () => {
  describe("when no category filter is given and the server fails", () => {
    let result: { current: ReturnType<typeof useProfileAssets> };
    let url: URL;

    beforeEach(async () => {
      fetchMock.mockResolvedValueOnce(json({ error: "down" }, 503));
      ({ result } = renderHook(() => useProfileAssets(ADDRESS, null), { wrapper }));
      await waitFor(() => expect(result.current.isError).toBe(true));
      url = new URL(fetchMock.mock.calls[0]![0] as string);
    });

    it("should expose a failure instead of an empty list", () => {
      expect(result.current.data).toBeUndefined();
    });

    it("should read the owner's NFTs", () => {
      expect(url.pathname).toBe("/marketplace-api/v1/nfts");
      expect(url.searchParams.get("owner")).toBe(ADDRESS);
    });

    it("should read every category", () => {
      expect(url.searchParams.has("category")).toBe(false);
    });
  });
});

describe("useProfileCommunities", () => {
  const OWNER = SIGNER.toLowerCase();
  const row = (id: string) => ({ id, name: "Builders", role: "owner", membersCount: null, privacy: null, thumbnailUrl: null });
  const page = (rows: unknown[]) => ({ communities: rows, total: rows.length });

  describe("when the value is not a wallet address", () => {
    let result: { current: ReturnType<typeof useProfileCommunities> };

    beforeEach(() => {
      ({ result } = renderHook(() => useProfileCommunities("alice", null), { wrapper }));
    });

    it("should stay idle", () => {
      expect(result.current.fetchStatus).toBe("idle");
    });

    it("should request nothing", () => {
      expect(communities.member).not.toHaveBeenCalled();
    });
  });

  describe("when a visitor with an identity reads another member's list", () => {
    let result: { current: ReturnType<typeof useProfileCommunities> };

    beforeEach(async () => {
      communities.member.mockResolvedValueOnce(page([row("c1")]));
      ({ result } = renderHook(() => useProfileCommunities(ADDRESS, identity), { wrapper }));
      await waitFor(() => expect(result.current.data).toEqual(page([row("c1")])));
    });

    it("should read the public list unsigned", () => {
      expect(communities.member).toHaveBeenCalledWith(ADDRESS, { signal: expect.any(AbortSignal) });
    });

    it("should not read the signed list", () => {
      expect(communities.mine).not.toHaveBeenCalled();
    });

    it("should cache the list under the public key", () => {
      expect(client.getQueryData(profileKeys.communities(ADDRESS))).toEqual(page([row("c1")]));
    });
  });

  describe("when the member reads their own list", () => {
    let result: { current: ReturnType<typeof useProfileCommunities> };
    let key: string[];

    beforeEach(async () => {
      key = ["private", OWNER, "profile", "communities"];
      communities.mine.mockResolvedValueOnce(page([row("c1")]));
      ({ result } = renderHook(() => useProfileCommunities(OWNER, identity), { wrapper }));
      await waitFor(() => expect(result.current.data).toEqual(page([row("c1")])));
    });

    it("should read it signed", () => {
      expect(communities.mine).toHaveBeenCalledWith(identity, { signal: expect.any(AbortSignal) });
    });

    it("should not read the public list", () => {
      expect(communities.member).not.toHaveBeenCalled();
    });

    it("should cache it under the signer's private prefix", () => {
      expect(client.getQueryData(key)).toEqual(page([row("c1")]));
    });

    describe("and the signer's private entries are removed", () => {
      beforeEach(() => {
        client.removeQueries({ queryKey: ["private", OWNER] });
      });

      it("should drop the cached list", () => {
        expect(client.getQueryData(key)).toBeUndefined();
      });
    });
  });

  describe("when the member signs in on their own profile", () => {
    let result: { current: ReturnType<typeof useProfileCommunities> };
    let rerender: (props: { who: AuthIdentity | null }) => void;
    let settle: (value: unknown) => void;

    beforeEach(async () => {
      communities.member.mockResolvedValue(page([row("public")]));
      settle = () => {};
      communities.mine.mockReturnValueOnce(new Promise((resolve) => (settle = resolve)));
      ({ result, rerender } = renderHook(({ who }: { who: AuthIdentity | null }) => useProfileCommunities(OWNER, who), {
        wrapper,
        initialProps: { who: null as AuthIdentity | null },
      }));
      await waitFor(() => expect(result.current.data).toEqual(page([row("public")])));
      rerender({ who: identity });
    });

    describe("and the signed list is still loading", () => {
      it("should keep the public list on screen", () => {
        expect(result.current.data).toEqual(page([row("public")]));
      });
    });

    describe("and the signed list loads and the member signs out", () => {
      beforeEach(async () => {
        settle(page([row("public"), row("private")]));
        await waitFor(() => expect(result.current.data?.communities).toHaveLength(2));
        rerender({ who: null });
      });

      it("should drop the private rows", () => {
        expect(result.current.data).toEqual(page([row("public")]));
      });
    });
  });

  describe("when the load fails", () => {
    let result: { current: ReturnType<typeof useProfileCommunities> };

    beforeEach(async () => {
      communities.member.mockRejectedValueOnce(new Error("503"));
      ({ result } = renderHook(() => useProfileCommunities(ADDRESS, null), { wrapper }));
      await waitFor(() => expect(result.current.isError).toBe(true));
    });

    it("should report it as an error with no rows", () => {
      expect(result.current.data).toBeUndefined();
    });
  });
});

describe("useProfilePhotos", () => {
  const OWNER = SIGNER.toLowerCase();
  const row = (id: string, isPublic = true) => ({ id, isPublic });
  const page = (rows: unknown[]) => ({ items: rows, received: rows.length, total: rows.length });

  describe("when the owner signs in while the public photos are shown", () => {
    let result: { current: ReturnType<typeof useProfilePhotos> };
    let rerender: (props: { id: AuthIdentity | null }) => void;
    let release: (value: unknown) => void;

    beforeEach(async () => {
      photos.fetch.mockImplementation((_address: string, _params: unknown, opts: { identity?: AuthIdentity }) =>
        opts.identity ? new Promise((resolve) => (release = resolve)) : Promise.resolve(page([row("pub")])),
      );
      ({ result, rerender } = renderHook(({ id }) => useProfilePhotos(OWNER, id), {
        wrapper,
        initialProps: { id: null as AuthIdentity | null },
      }));
      await waitFor(() => expect(result.current.data?.pages[0]?.items).toEqual([row("pub")]));
      rerender({ id: identity });
    });

    describe("and the signed list is still loading", () => {
      it("should mark the shown data as a placeholder", () => {
        expect(result.current.isPlaceholderData).toBe(true);
      });

      it("should keep the public photos on screen", () => {
        expect(result.current.data?.pages[0]?.items).toEqual([row("pub")]);
      });
    });

    describe("and the signed list loads", () => {
      beforeEach(async () => {
        release(page([row("pub"), row("mine", false)]));
        await waitFor(() => expect(result.current.isPlaceholderData).toBe(false));
      });

      it("should show the owner's full list", () => {
        expect(result.current.data?.pages[0]?.items).toEqual([row("pub"), row("mine", false)]);
      });
    });
  });

  describe("when the list stops being signed", () => {
    let result: { current: ReturnType<typeof useProfilePhotos> };

    beforeEach(async () => {
      photos.fetch.mockImplementation((_address: string, _params: unknown, opts: { identity?: AuthIdentity }) =>
        opts.identity ? Promise.resolve(page([row("mine", false)])) : new Promise(() => {}),
      );
      const hook = renderHook(({ id }) => useProfilePhotos(OWNER, id), {
        wrapper,
        initialProps: { id: identity as AuthIdentity | null },
      });
      result = hook.result;
      await waitFor(() => expect(result.current.data?.pages[0]?.items).toEqual([row("mine", false)]));
      hook.rerender({ id: null });
    });

    it("should not keep the owner's private photos", () => {
      expect(result.current.data).toBeUndefined();
    });

    it("should be pending the public list", () => {
      expect(result.current.isPending).toBe(true);
    });
  });

  describe("when the profile changes to another one", () => {
    let result: { current: ReturnType<typeof useProfilePhotos> };

    beforeEach(async () => {
      photos.fetch.mockImplementation((address: string) =>
        address === OWNER ? Promise.resolve(page([row("a")])) : new Promise(() => {}),
      );
      const hook = renderHook(({ address }) => useProfilePhotos(address, null), {
        wrapper,
        initialProps: { address: OWNER },
      });
      result = hook.result;
      await waitFor(() => expect(result.current.data).toBeDefined());
      hook.rerender({ address: ADDRESS });
    });

    it("should not carry the first profile's photos over", () => {
      expect(result.current.data).toBeUndefined();
    });
  });

  describe("when the value is not a wallet address", () => {
    let result: { current: ReturnType<typeof useProfilePhotos> };

    beforeEach(() => {
      ({ result } = renderHook(() => useProfilePhotos("alice", null), { wrapper }));
    });

    it("should stay idle", () => {
      expect(result.current.fetchStatus).toBe("idle");
    });

    it("should request nothing", () => {
      expect(photos.fetch).not.toHaveBeenCalled();
    });
  });
});
