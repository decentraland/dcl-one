type TtlMemo<A extends unknown[], T> = ((...args: A) => Promise<T>) & {
  reset: () => void;
  /** Drops the entry for these arguments, so the next call loads again. */
  forget: (...args: A) => void;
};

// Server memo for visitor-independent reads: one upstream call per key per ttl, concurrent
// callers share the in-flight promise; a null key bypasses the memo; rejections and values
// failing `keep` are never retained. `maxEntries` bounds the memo (least recently used goes
// first); `staleMs` serves an expired value for that much longer while one reload runs behind it.
export function ttlMemo<A extends unknown[], T>(opts: {
  ttlMs: number;
  load: (...args: A) => Promise<T>;
  keyOf?: (...args: A) => string | null;
  keep?: (value: T) => boolean;
  maxEntries?: number;
  staleMs?: number;
}): TtlMemo<A, T> {
  const entries = new Map<string, { at: number; value: T }>();
  const inflight = new Map<string, Promise<T>>();
  const keyFor = (args: A) => (opts.keyOf ? opts.keyOf(...args) : "");
  const reload = (key: string, args: A): Promise<T> => {
    const pending = inflight.get(key);
    if (pending) return pending;
    const p = opts
      .load(...args)
      .then((value) => {
        if (!opts.keep || opts.keep(value)) {
          entries.delete(key);
          entries.set(key, { at: Date.now(), value });
          if (opts.maxEntries !== undefined && entries.size > opts.maxEntries) entries.delete(entries.keys().next().value!);
        }
        return value;
      })
      .finally(() => {
        inflight.delete(key);
      });
    inflight.set(key, p);
    return p;
  };
  const memo = ((...args: A): Promise<T> => {
    const key = keyFor(args);
    if (key === null) return opts.load(...args);
    const hit = entries.get(key);
    const age = hit ? Date.now() - hit.at : Infinity;
    if (hit && age < opts.ttlMs) {
      // Most recently used last, so eviction takes the coldest key.
      entries.delete(key);
      entries.set(key, hit);
      return Promise.resolve(hit.value);
    }
    if (hit && age < opts.ttlMs + (opts.staleMs ?? 0)) {
      // A failed background reload keeps the stale value; the next call tries again.
      void reload(key, args).catch(() => {});
      return Promise.resolve(hit.value);
    }
    if (hit) entries.delete(key);
    return reload(key, args);
  }) as TtlMemo<A, T>;
  memo.reset = () => {
    entries.clear();
    inflight.clear();
  };
  memo.forget = (...args: A) => {
    const key = keyFor(args);
    if (key !== null) entries.delete(key);
  };
  return memo;
}
