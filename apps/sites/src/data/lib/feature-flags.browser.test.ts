// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";

import { primeRemoteFlags, remoteFlag, remoteFlagsPrimed, REMOTE_FLAG } from "@ui/shop/discovery/lib/remoteFlags";

import { primeDappsFlags } from "./feature-flags.browser";

afterEach(() => {
  vi.unstubAllGlobals();
  primeRemoteFlags(null);
});

describe("rollout flags for a page the server did not render", () => {
  it("reads them once from this server and primes them", async () => {
    const fetchMock = vi.fn(async () => new Response(JSON.stringify({ "dapps-shop-creator-sales": true })));
    vi.stubGlobal("fetch", fetchMock);
    await primeDappsFlags();
    await primeDappsFlags();
    expect(fetchMock).toHaveBeenCalledOnce();
    expect(fetchMock).toHaveBeenCalledWith("/internal/dapps-flags", expect.anything());
    expect(remoteFlag(REMOTE_FLAG.creatorSales)).toBe(true);
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
