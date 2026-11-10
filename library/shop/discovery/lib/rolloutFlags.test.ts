// The rollout switches without a host: reading the file through the transport given, and evaluating an answer.
import { describe, expect, it } from "vitest";
import {
  declaredOr,
  parseRemoteFlags,
  readFlagsFile,
  REMOTE_FLAG,
  remoteFlagOn,
  remoteFlagVariantIn,
  resolveShopFlags,
  type FlagsTransport,
} from "./rolloutFlags";

function transport(status: number, body: string): FlagsTransport & { calls: [string, { signal?: AbortSignal }][] } {
  const calls: [string, { signal?: AbortSignal }][] = [];
  return Object.assign(async (url: string, init: { signal?: AbortSignal }) => (calls.push([url, init]), { status, body }), { calls });
}

describe("reading the flags file", () => {
  it("asks the service it is given for the dapps file, with the caller's own signal", async () => {
    const t = transport(200, JSON.stringify({ flags: { "dapps-shop-creator-sales": true, "dapps-other": "yes" } }));
    const signal = new AbortController().signal;
    const answer = await readFlagsFile("https://flags.test", t, signal);
    expect(t.calls[0]![0]).toBe("https://flags.test/dapps.json");
    expect(t.calls[0]![1].signal).toBe(signal);
    expect(answer).toEqual({ flags: { "dapps-shop-creator-sales": true, "dapps-other": false }, variants: {} });
  });

  it("refuses an answer that is not a 2xx, or not JSON", async () => {
    await expect(readFlagsFile("https://flags.test", transport(503, "{}"))).rejects.toThrow("feature flags: HTTP 503");
    await expect(readFlagsFile("https://flags.test", transport(200, "<html>"))).rejects.toThrow();
  });
});

describe("evaluating an answer", () => {
  it("is off without an answer, and on only where it says yes", () => {
    expect(remoteFlagOn(null, REMOTE_FLAG.creatorSales)).toBe(false);
    expect(remoteFlagOn({ flags: { "dapps-shop-creator-sales": true } }, REMOTE_FLAG.creatorSales)).toBe(true);
    expect(remoteFlagOn({ flags: { "shop-creator-sales": true } }, REMOTE_FLAG.creatorSales)).toBe(false);
    const variants = parseRemoteFlags({ variants: { "dapps-shop-my-store": { enabled: true, payload: { value: "0xabc" } } } }).variants;
    expect(remoteFlagVariantIn({ variants }, REMOTE_FLAG.myStore)).toBe("0xabc");
    expect(remoteFlagVariantIn(undefined, REMOTE_FLAG.myStore)).toBeNull();
  });

  it("takes a host's declared switch over the file, and the file's while it declares none", () => {
    expect(declaredOr({}, "creatorSales", null)).toBeNull();
    expect(declaredOr({ creatorSales: false }, "creatorSales", true)).toBe(false);
    const remote = (on: boolean) => () => on;
    expect(resolveShopFlags({}, remote(true))).toEqual({ creatorSales: true });
    expect(resolveShopFlags({ creatorSales: false }, remote(true))).toEqual({ creatorSales: false });
    expect(resolveShopFlags({ creatorSales: true }, remote(false))).toEqual({ creatorSales: true });
  });
});
