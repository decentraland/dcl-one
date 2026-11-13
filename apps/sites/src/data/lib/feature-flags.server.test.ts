import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { dappsFlags, dappsFlagsAnswer, resetDappsFlags } from "./feature-flags.server";

const fetchMock = vi.fn();

beforeEach(() => {
  vi.stubGlobal("fetch", fetchMock);
  vi.stubEnv("FEATURE_FLAGS_ORIGIN", "");
  resetDappsFlags();
});
afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
  vi.unstubAllEnvs();
  fetchMock.mockReset();
});

const answer = (body: unknown, status = 200) => fetchMock.mockResolvedValueOnce(new Response(JSON.stringify(body), { status }));

describe("decentraland.org's rollout flags, read by the server", () => {
  it("asks as decentraland.org, keeps only true as on, and reads once a minute", async () => {
    answer({ flags: { "dapps-shop-creator-sales": true, "dapps-shop-names": "yes", "dapps-off": false }, variants: {} });
    expect(await dappsFlags()).toEqual({ "dapps-shop-creator-sales": true, "dapps-shop-names": false, "dapps-off": false });
    await dappsFlags();
    expect(fetchMock).toHaveBeenCalledOnce();
    expect(fetchMock.mock.calls[0]![0]).toBe("https://feature-flags.decentraland.org/dapps.json");
    expect((fetchMock.mock.calls[0]![1] as RequestInit).headers).toEqual({ Referer: "https://decentraland.org/" });
  });

  it("keeps an enabled variant's string payload with the flags, from the same read", async () => {
    const list = "0x00000000000000000000000000000000000000aa";
    answer({
      flags: { "dapps-shop-my-store": true },
      variants: {
        "dapps-shop-my-store": { name: "testers", enabled: true, payload: { type: "string", value: list } },
        "dapps-off": { name: "x", enabled: false, payload: { type: "string", value: "no" } },
      },
    });
    expect(await dappsFlagsAnswer()).toEqual({ flags: { "dapps-shop-my-store": true }, variants: { "dapps-shop-my-store": list } });
    expect(await dappsFlags()).toEqual({ "dapps-shop-my-store": true });
    expect(fetchMock).toHaveBeenCalledOnce();
  });

  it("asks as the configured site on another domain", async () => {
    vi.stubEnv("FEATURE_FLAGS_ORIGIN", "https://decentraland.zone/");
    answer({ flags: {} });
    await dappsFlags();
    expect((fetchMock.mock.calls[0]![1] as RequestInit).headers).toEqual({ Referer: "https://decentraland.zone/" });
  });

  it("keeps the last good answer through a failure", async () => {
    vi.useFakeTimers({ toFake: ["Date"] });
    answer({ flags: { "dapps-shop-creator-sales": true } });
    expect(await dappsFlags()).toEqual({ "dapps-shop-creator-sales": true });
    vi.setSystemTime(Date.now() + 61_000);
    fetchMock.mockRejectedValueOnce(new TypeError("fetch failed"));
    expect(await dappsFlags()).toEqual({ "dapps-shop-creator-sales": true });
  });

  it("is all off for a minute when the service fails before any answer, releasing the failed answer", async () => {
    vi.useFakeTimers({ toFake: ["Date"] });
    const cancel = vi.fn(async () => {});
    fetchMock.mockResolvedValueOnce({ ok: false, status: 503, body: { cancel } } as unknown as Response);
    expect(await dappsFlags()).toEqual({});
    expect(cancel).toHaveBeenCalledOnce();
    expect(await dappsFlags()).toEqual({});
    expect(fetchMock).toHaveBeenCalledOnce();
    vi.setSystemTime(Date.now() + 61_000);
    answer({ flags: { "dapps-shop-creator-sales": true } });
    expect(await dappsFlags()).toEqual({ "dapps-shop-creator-sales": true });
  });
});
