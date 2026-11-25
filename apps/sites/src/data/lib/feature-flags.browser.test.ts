// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";

import { primeRemoteFlags, refreshRemoteFlags, remoteFlag, remoteFlagsPrimed, remoteFlagState, remoteFlagVariant, REMOTE_FLAG } from "@ui/shop/discovery/lib/remoteFlags";

import { primeDappsFlags, readDappsFlags } from "./feature-flags.browser";

afterEach(() => {
  vi.unstubAllGlobals();
  primeRemoteFlags(null);
});

describe("rollout flags for a page the server did not render", () => {
  it("reads them once from this server and primes them, with their variants' payloads", async () => {
    const list = "0x00000000000000000000000000000000000000aa";
    const fetchMock = vi.fn(
      async () => new Response(JSON.stringify({ flags: { "dapps-shop-creator-sales": true, "dapps-shop-my-store": true }, variants: { "dapps-shop-my-store": list } })),
    );
    vi.stubGlobal("fetch", fetchMock);
    await primeDappsFlags();
    await primeDappsFlags();
    expect(fetchMock).toHaveBeenCalledOnce();
    expect(fetchMock).toHaveBeenCalledWith("/internal/feature-flags?variants=1", expect.anything());
    expect(remoteFlag(REMOTE_FLAG.creatorSales)).toBe(true);
    expect(remoteFlagVariant(REMOTE_FLAG.myStore)).toBe(list);
  });

  it("still reads a bare flags map, as a server from before the variants answered", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => new Response(JSON.stringify({ "dapps-shop-creator-sales": true }))));
    await primeDappsFlags();
    expect(remoteFlag(REMOTE_FLAG.creatorSales)).toBe(true);
    expect(remoteFlagVariant(REMOTE_FLAG.myStore)).toBeNull();
  });

  it("gives the library a reader of this server, never of the service (it refuses this site's origin)", async () => {
    const fetchMock = vi.fn(async (_url: string) => new Response(JSON.stringify({ flags: { "dapps-shop-creator-sales": true }, variants: {} })));
    vi.stubGlobal("fetch", fetchMock);
    const now = Date.now();
    await refreshRemoteFlags(now, readDappsFlags);
    await refreshRemoteFlags(now + 59_000, readDappsFlags);
    expect(fetchMock.mock.calls.map(([url]) => url)).toEqual(["/internal/feature-flags?variants=1"]);
    expect(remoteFlag(REMOTE_FLAG.creatorSales)).toBe(true);
    expect(remoteFlagsPrimed()).toBe(false);
  });

  it("lets the library give up and back off when this server cannot answer", async () => {
    const fetchMock = vi.fn(async () => new Response("", { status: 503 }));
    vi.stubGlobal("fetch", fetchMock);
    const now = Date.now();
    await refreshRemoteFlags(now, readDappsFlags);
    expect(remoteFlagState(REMOTE_FLAG.creatorSales)).toBe(false);
    await refreshRemoteFlags(now + 5_000, readDappsFlags);
    expect(fetchMock).toHaveBeenCalledTimes(2);
    await refreshRemoteFlags(now + 11_000, readDappsFlags);
    expect(fetchMock).toHaveBeenCalledTimes(4);
  });

  it("asks nothing when a server render already primed them", async () => {
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);
    primeRemoteFlags({ "dapps-shop-creator-sales": false });
    await primeDappsFlags();
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("leaves them off, and releases the body, when this server cannot answer", async () => {
    const cancel = vi.fn(async () => {});
    vi.stubGlobal("fetch", vi.fn(async () => ({ ok: false, status: 503, body: { cancel } }) as unknown as Response));
    await primeDappsFlags();
    expect(cancel).toHaveBeenCalledOnce();
    expect(remoteFlagsPrimed()).toBe(false);
    vi.stubGlobal("fetch", vi.fn(async () => {
      throw new TypeError("offline");
    }));
    await primeDappsFlags();
    expect(remoteFlag(REMOTE_FLAG.creatorSales)).toBe(false);
  });
});
