export interface BackoffPolicy {
  readonly initialMs: number;
  readonly maxMs: number;
  readonly factor?: number;
  readonly jitter?: number;
}

export const DEFAULT_BACKOFF = Object.freeze({ initialMs: 100, maxMs: 1000, factor: 2 });
const MAX_BACKOFF_EXPONENT = 32;
const MAX_TIMER_DELAY_MS = 2 ** 31 - 1;

export function backoffDelay(attempt: number, policy: BackoffPolicy = DEFAULT_BACKOFF, remainingMs = Infinity, random = Math.random): number {
  const exponent = Math.max(0, Math.min(attempt, MAX_BACKOFF_EXPONENT));
  const base = Math.min(policy.maxMs, Math.max(1, policy.initialMs) * (policy.factor ?? DEFAULT_BACKOFF.factor) ** exponent);
  const jitter = policy.jitter ?? 0;
  return Math.max(0, Math.min(remainingMs, base * (1 - jitter + (jitter ? random() * jitter : 0))));
}

export interface CancellationSignal {
  readonly aborted: boolean;
  readonly reason?: unknown;
  addEventListener(type: "abort", listener: () => void, options?: { once?: boolean }): void;
  removeEventListener(type: "abort", listener: () => void): void;
}

function abortReason(signal?: CancellationSignal): unknown {
  return signal?.reason ?? Object.assign(new Error("Aborted"), { name: "AbortError" });
}

function createSleep(ms: number, signal?: CancellationSignal): { promise: Promise<void>; cancel(): void } {
  let cancel = () => {};
  const promise = new Promise<void>((resolve, reject) => {
    const finish = (reason?: unknown) => {
      clearTimeout(timer);
      signal?.removeEventListener("abort", abort);
      if (reason === undefined) resolve();
      else reject(reason);
    };
    const abort = () => finish(abortReason(signal));
    const timer = setTimeout(() => finish(), ms);
    cancel = () => finish();
    signal?.addEventListener("abort", abort, { once: true });
    if (signal?.aborted) abort();
  });
  return { promise, cancel };
}

export function sleep(ms: number, signal?: CancellationSignal): Promise<void> {
  return createSleep(ms, signal).promise;
}

export interface WaitUntilOptions {
  timeoutMs: number;
  description: string;
  initialDelayMs?: number;
  maxDelayMs?: number;
  signal?: CancellationSignal;
  now?: () => number;
  sleep?: (ms: number) => Promise<void>;
}

function monotonicNow(): number {
  const clock = (globalThis as { performance?: { now(): number } }).performance;
  return clock ? clock.now() : Date.now();
}

export async function waitUntil<T>(check: (remainingMs: number) => T | false | undefined | Promise<T | false | undefined>, options: WaitUntilOptions): Promise<T> {
  const { timeoutMs, description, signal, now = monotonicNow } = options;
  if (!Number.isFinite(timeoutMs) || timeoutMs <= 0 || timeoutMs > MAX_TIMER_DELAY_MS) {
    throw new RangeError(`timeoutMs must be greater than 0 and at most ${MAX_TIMER_DELAY_MS}`);
  }
  const policy = { initialMs: options.initialDelayMs ?? DEFAULT_BACKOFF.initialMs, maxMs: options.maxDelayMs ?? DEFAULT_BACKOFF.maxMs };
  let cancelSleep: (() => void) | undefined;
  const pause = options.sleep ?? (async (ms: number) => {
    const sleeping = createSleep(ms, signal);
    cancelSleep = sleeping.cancel;
    try {
      await sleeping.promise;
    } finally {
      cancelSleep = undefined;
    }
  });
  const deadline = now() + timeoutMs;
  const timedOut = () => new Error(`Timed out: ${description}`);
  if (signal?.aborted) throw abortReason(signal);
  let finished = false;
  let abort: (() => void) | undefined;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const limit = new Promise<never>((_, reject) => {
    timer = setTimeout(() => reject(timedOut()), Math.max(0, timeoutMs));
    abort = () => reject(abortReason(signal));
    signal?.addEventListener("abort", abort, { once: true });
    if (signal?.aborted) abort();
  });
  const run = async (): Promise<T> => {
    let attempt = 0;
    for (;;) {
      if (finished) throw timedOut();
      const remaining = deadline - now();
      if (remaining <= 0) throw timedOut();
      const result = await check(remaining);
      if (finished) throw timedOut();
      if (signal?.aborted) throw abortReason(signal);
      if (now() >= deadline) throw timedOut();
      if (result !== undefined && result !== false) return result;
      await pause(backoffDelay(attempt++, policy, deadline - now()));
    }
  };
  try {
    return await Promise.race([run(), limit]);
  } finally {
    finished = true;
    cancelSleep?.();
    if (timer !== undefined) clearTimeout(timer);
    if (abort) signal?.removeEventListener("abort", abort);
  }
}
