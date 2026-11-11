import { afterEach, describe, expect, it, vi } from "vitest";

import { ttlMemo } from "./ttl-memo";

afterEach(() => {
  vi.useRealTimers();
});

describe("ttlMemo", () => {
  it("serves one upstream call per key inside the ttl and shares the in-flight promise", async () => {
    vi.useFakeTimers();
    const load = vi.fn(async (k: string) => ({ k, n: load.mock.calls.length }));
    const memo = ttlMemo({ ttlMs: 1000, load, keyOf: (k) => k });
    const [a, b] = await Promise.all([memo("x"), memo("x")]);
    expect(load).toHaveBeenCalledTimes(1);
    expect(b).toBe(a);
    expect(await memo("x")).toBe(a);
    expect(await memo("y")).not.toBe(a);
    expect(load).toHaveBeenCalledTimes(2);
    vi.advanceTimersByTime(1001);
    expect(await memo("x")).not.toBe(a);
    expect(load).toHaveBeenCalledTimes(3);
  });

  it("never retains rejections or values failing keep, and a null key bypasses", async () => {
    let fail = true;
    const load = vi.fn(async (k: string | null) => {
      if (fail) throw new Error("down");
      return k;
    });
    const memo = ttlMemo({
      ttlMs: 1000,
      load,
      keyOf: (k) => k,
      keep: (v) => v !== "skip",
    });
    await expect(memo("x")).rejects.toThrow("down");
    fail = false;
    expect(await memo("x")).toBe("x");
    expect(load).toHaveBeenCalledTimes(2);
    await memo("skip");
    await memo("skip");
    expect(load).toHaveBeenCalledTimes(4);
    await memo(null);
    await memo(null);
    expect(load).toHaveBeenCalledTimes(6);
    memo.reset();
    await memo("x");
    expect(load).toHaveBeenCalledTimes(7);
  });

  it("evicts the least recently used key past maxEntries", async () => {
    const load = vi.fn(async (k: string) => k);
    const memo = ttlMemo({ ttlMs: 1000, load, keyOf: (k) => k, maxEntries: 2 });
    await memo("a");
    await memo("b");
    await memo("a");
    await memo("c");
    expect(load).toHaveBeenCalledTimes(3);
    await memo("a");
    expect(load).toHaveBeenCalledTimes(3);
    await memo("b");
    expect(load).toHaveBeenCalledTimes(4);
  });

  it("serves a stale value inside staleMs while one reload runs, and keeps it when the reload fails", async () => {
    vi.useFakeTimers({ toFake: ["Date"] });
    const settle = () => new Promise((resolve) => setImmediate(resolve));
    let n = 0;
    let fail = false;
    const load = vi.fn(async () => {
      if (fail) throw new Error("down");
      return ++n;
    });
    const memo = ttlMemo({ ttlMs: 1000, staleMs: 5000, load });
    expect(await memo()).toBe(1);
    vi.setSystemTime(Date.now() + 1500);
    fail = true;
    expect(await memo()).toBe(1);
    await settle();
    expect(load).toHaveBeenCalledTimes(2);
    fail = false;
    expect(await memo()).toBe(1);
    await settle();
    expect(await memo()).toBe(2);
    expect(load).toHaveBeenCalledTimes(3);
    vi.setSystemTime(Date.now() + 7000);
    fail = true;
    await expect(memo()).rejects.toThrow("down");
  });

  it("forget drops one key", async () => {
    const load = vi.fn(async (k: string) => k);
    const memo = ttlMemo({ ttlMs: 1000, load, keyOf: (k) => k });
    await memo("a");
    await memo("b");
    memo.forget("a");
    await memo("a");
    await memo("b");
    expect(load).toHaveBeenCalledTimes(3);
  });
});
