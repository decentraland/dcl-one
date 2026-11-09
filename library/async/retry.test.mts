import assert from "node:assert/strict";
import { test } from "node:test";
import { backoffDelay, DEFAULT_BACKOFF, waitUntil, sleep } from "./index.ts";
import { fetchGeneratesNoServerError } from "./http.ts";

test("backoff preserves jitter and clamps large attempts to the cap and remaining budget", () => {
  const policy = { initialMs: 500, maxMs: 15_000, factor: 1.5, jitter: 0.2 };
  assert.equal(backoffDelay(0, policy, Infinity, () => 0), 400);
  assert.equal(backoffDelay(1, policy, Infinity, () => 1), 750);
  assert.equal(backoffDelay(1000, policy, 123), 123);
  assert.equal(backoffDelay(0, policy, -1), 0);
  assert.deepEqual([0, 1, 2, 3, 4].map((n) => backoffDelay(n)), [100, 200, 400, 800, 1000]);
});

test("waitUntil checks immediately and does not invoke checks at or after its deadline", async () => {
  let now = 0;
  const calls: number[] = [];
  const sleeps: number[] = [];
  await assert.rejects(waitUntil(() => { calls.push(now); }, {
    timeoutMs: 35, description: "fixture", initialDelayMs: 10, maxDelayMs: 20, now: () => now,
    sleep: async (ms) => { sleeps.push(ms); now += ms; },
  }), /Timed out: fixture/);
  assert.deepEqual(calls, [0, 10, 30]);
  assert.deepEqual(sleeps, [10, 20, 5]);
});

test("a successful waitUntil returns immediately without sleeping", async () => {
  let slept = false;
  assert.equal(await waitUntil(() => 0, { timeoutMs: 1000, description: "zero is a value", sleep: async () => { slept = true; } }), 0);
  assert.equal(slept, false);
});

test("waitUntil times out even when its condition never resolves", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const pending = waitUntil(() => new Promise<never>(() => {}), { timeoutMs: 35, description: "stalled condition" });
  const rejected = assert.rejects(pending, /Timed out: stalled condition/);
  t.mock.timers.tick(35);
  await rejected;
});

test("waitUntil propagates hard failures without retries", async () => {
  let attempts = 0;
  await assert.rejects(waitUntil(() => { attempts++; throw new Error("hard failure"); }, { timeoutMs: 1000, description: "fixture" }), /hard failure/);
  assert.equal(attempts, 1);
});

test("aborting a waitUntil stops retrying and aborting sleep releases its timer", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const controller = new AbortController();
  let attempts = 0;
  const pending = waitUntil(() => { attempts++; }, { timeoutMs: 1000, description: "fixture", signal: controller.signal });
  const rejected = assert.rejects(pending, { name: "AbortError" });
  await Promise.resolve();
  await Promise.resolve();
  controller.abort();
  await rejected;
  t.mock.timers.tick(1000);
  assert.equal(attempts, 1);
  await assert.rejects(sleep(1000, controller.signal), { name: "AbortError" });
  assert.equal(DEFAULT_BACKOFF.maxMs, 1000);
});

test("HTTP probes accept client errors, reject server errors and cancel their bodies", async (t) => {
  let cancelled = 0;
  let status = 200;
  t.mock.method(globalThis, "fetch", async () => new Response(new ReadableStream({ cancel: () => { cancelled++; } }), { status }));
  assert.equal(await fetchGeneratesNoServerError("http://fixture"), true);
  status = 404;
  assert.equal(await fetchGeneratesNoServerError("http://fixture"), true);
  status = 503;
  assert.equal(await fetchGeneratesNoServerError("http://fixture"), false);
  assert.equal(cancelled, 3);
});

test("HTTP probes use the caller's timeout instead of the default", async (t) => {
  const controller = new AbortController();
  let timeout = 0;
  t.mock.method(AbortSignal, "timeout", (ms: number) => { timeout = ms; return controller.signal; });
  t.mock.method(globalThis, "fetch", async (_url, options) => {
    assert.equal(options?.signal, controller.signal);
    return new Response(null, { status: 204 });
  });
  assert.equal(await fetchGeneratesNoServerError("http://fixture", 17), true);
  assert.equal(timeout, 17);
});

test("Boolean conditions keep waiting while false", async () => {
  let now = 0;
  let checks = 0;
  const sleeps: number[] = [];
  assert.equal(await waitUntil(() => ++checks >= 3, {
    timeoutMs: 1000, description: "Boolean predicate", initialDelayMs: 10, maxDelayMs: 20, now: () => now,
    sleep: async (ms) => { sleeps.push(ms); now += ms; },
  }), true);
  assert.deepEqual(sleeps, [10, 20]);
});


test("a successful condition cannot extend the deadline", async () => {
  let now = 0;
  await assert.rejects(waitUntil(() => { now = 10; return true; }, {
    timeoutMs: 10, description: "strict deadline", now: () => now,
  }), /Timed out: strict deadline/);
  now = 0;
  assert.equal(await waitUntil(() => { now = 9; return 0; }, {
    timeoutMs: 10, description: "within budget", now: () => now,
  }), 0);
});

test("timing out cancels the owned backoff timer even with an injected clock", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const timers = new Set<ReturnType<typeof setTimeout>>();
  const schedule = globalThis.setTimeout;
  const clear = globalThis.clearTimeout;
  t.mock.method(globalThis, "setTimeout", (callback: () => void, ms?: number) => {
    const id = schedule(() => { timers.delete(id); callback(); }, ms);
    timers.add(id);
    return id;
  });
  t.mock.method(globalThis, "clearTimeout", (id: ReturnType<typeof setTimeout>) => { timers.delete(id); clear(id); });
  const pending = waitUntil(async () => {
    await new Promise<void>((resolve) => setTimeout(resolve, 80));
    return false;
  }, { timeoutMs: 100, initialDelayMs: 100, description: "owned timer", now: () => 0 });
  const rejected = assert.rejects(pending, /Timed out: owned timer/);
  t.mock.timers.tick(80);
  await Promise.resolve();
  await Promise.resolve();
  assert.equal(timers.size, 2);
  t.mock.timers.tick(20);
  await rejected;
  assert.equal(timers.size, 0);
});


test("waitUntil rejects zero, overflowing or non-finite timeout budgets before checking", async () => {
  let checks = 0;
  for (const timeoutMs of [0, 2 ** 31, Infinity, NaN, -1]) {
    await assert.rejects(waitUntil(() => { checks++; return true; }, { timeoutMs, description: "invalid budget" }), RangeError);
  }
  assert.equal(checks, 0);
});

test("the largest supported timeout stays pending and can be cancelled", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const controller = new AbortController();
  let finished = false;
  const pending = waitUntil(() => new Promise<never>(() => {}), {
    timeoutMs: 2 ** 31 - 1, description: "maximum budget", signal: controller.signal,
  }).finally(() => { finished = true; });
  const rejected = assert.rejects(pending, { name: "AbortError" });
  t.mock.timers.tick(1);
  await Promise.resolve();
  assert.equal(finished, false);
  controller.abort();
  await rejected;
});
