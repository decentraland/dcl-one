// from: shop@94584a5c:app/src/hooks/useInfiniteGrid.ts (the paged grid's states), app/src/main.tsx (the query defaults)

import { useCallback, useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";

type Entry = { data: unknown; at: number };

const cache = new Map<string, Entry>();
type Flight = { promise: Promise<unknown>; controller: AbortController; refs: number };
const inflight = new Map<string, Flight>();
let generation = 0;
const listeners = new Set<() => void>();

function notify() {
  generation++;
  for (const l of listeners) l();
}

export function invalidateShopQueries(prefix?: string): void {
  for (const key of [...cache.keys()]) if (prefix === undefined || key.startsWith(prefix)) cache.delete(key);
  for (const [key, flight] of [...inflight]) {
    if (prefix !== undefined && !key.startsWith(prefix)) continue;
    inflight.delete(key);
    flight.controller.abort();
  }
  notify();
}

function join<T>(id: string, fetcher: (signal: AbortSignal) => Promise<T>): { promise: Promise<T>; leave(): void } {
  let flight = inflight.get(id);
  if (!flight) {
    const controller = new AbortController();
    const promise = fetcher(controller.signal);
    const created: Flight = { promise, controller, refs: 0 };
    const clear = () => {
      if (inflight.get(id) === created) inflight.delete(id);
    };
    promise.then(clear, clear);
    inflight.set(id, created);
    flight = created;
  }
  const joined = flight;
  joined.refs++;
  return {
    promise: joined.promise as Promise<T>,
    leave() {
      joined.refs--;
      if (joined.refs <= 0 && inflight.get(id) === joined) {
        inflight.delete(id);
        joined.controller.abort();
      }
    },
  };
}

export function resetShopQueries(): void {
  cache.clear();
  for (const flight of inflight.values()) flight.controller.abort();
  inflight.clear();
}

export function primeShopQuery(key: readonly unknown[], data: unknown, at: number = Date.now()): void {
  cache.set(keyOf(key), { data, at });
  notify();
}

export function peekShopQuery<T>(key: readonly unknown[], staleMs: number = DEFAULT_STALE_MS): T | undefined {
  const entry = cache.get(keyOf(key));
  return entry && Date.now() - entry.at < staleMs ? (entry.data as T) : undefined;
}

export function keyOf(key: readonly unknown[]): string {
  return JSON.stringify(key);
}

function useGeneration(): number {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => generation,
    () => generation,
  );
}

export type QueryResult<T> = {
  data: T | undefined;
  error: unknown;
  isLoading: boolean;
  isPlaceholder: boolean;
  isFetching: boolean;
  refetch(): void;
};

export type QueryOptions = {
  enabled?: boolean;
  staleMs?: number;
  keepPrevious?: boolean;
};

const DEFAULT_STALE_MS = 30_000;

export function useShopQuery<T>(
  key: readonly unknown[] | null,
  fetcher: (signal: AbortSignal) => Promise<T>,
  { enabled = true, staleMs = DEFAULT_STALE_MS, keepPrevious = false }: QueryOptions = {},
): QueryResult<T> {
  const id = key && enabled ? keyOf(key) : null;
  const gen = useGeneration();
  const fetcherRef = useRef(fetcher);
  fetcherRef.current = fetcher;
  const [state, setState] = useState<{ id: string | null; error: unknown; fetching: boolean }>({ id, error: null, fetching: false });
  const [tick, setTick] = useState(0);
  const previous = useRef<{ data: T | undefined }>({ data: undefined });

  const entry = id ? cache.get(id) : undefined;

  useEffect(() => {
    if (!id) return;
    const cached = cache.get(id);
    if (cached && Date.now() - cached.at < staleMs && tick === 0) {
      setState({ id, error: null, fetching: false });
      return;
    }
    let live = true;
    setState({ id, error: null, fetching: true });
    const flight = join(id, (signal) => fetcherRef.current(signal));
    flight.promise.then(
      (data) => {
        cache.set(id, { data, at: Date.now() });
        if (live) setState({ id, error: null, fetching: false });
      },
      (error) => {
        if (live && (error as { name?: string } | null)?.name !== "AbortError") setState({ id, error, fetching: false });
      },
    );
    return () => {
      live = false;
      flight.leave();
    };
  }, [id, gen, tick, staleMs]);

  const data = entry?.data as T | undefined;
  if (data !== undefined) previous.current = { data };
  const sameKey = state.id === id;
  const error = sameKey ? state.error : null;
  const isLoading = !!id && data === undefined && !error;
  const isPlaceholder = isLoading && keepPrevious && previous.current.data !== undefined;

  const refetch = useCallback(() => {
    if (id) cache.delete(id);
    setTick((n) => n + 1);
  }, [id]);

  return {
    data: data !== undefined ? data : isPlaceholder ? previous.current.data : undefined,
    error,
    isLoading,
    isPlaceholder,
    isFetching: sameKey ? state.fetching : !!id,
    refetch,
  };
}

export type Page<T> = { items: T[]; total: number };

export type InfiniteResult<T, M> = {
  items: T[];
  total: number;
  meta: M | undefined;
  isLoading: boolean;
  isPlaceholder: boolean;
  error: unknown;
  hasNextPage: boolean;
  isFetchingNextPage: boolean;
  isFetchNextPageError: boolean;
  fetchNextPage(): void;
  refetch(): void;
};

export function useInfiniteShopQuery<T, M = undefined>(
  key: readonly unknown[] | null,
  fetchPage: (skip: number, signal: AbortSignal) => Promise<Page<T> & { meta?: M }>,
  { enabled = true, staleMs = DEFAULT_STALE_MS }: Omit<QueryOptions, "keepPrevious"> = {},
): InfiniteResult<T, M> {
  const first = useShopQuery<Page<T> & { meta?: M }>(key, (signal) => fetchPage(0, signal), { enabled, staleMs, keepPrevious: true });
  const id = key && enabled ? keyOf(key) : null;
  const [more, setMore] = useState<{ id: string | null; pages: T[][]; loading: boolean; failed: boolean }>({
    id,
    pages: [],
    loading: false,
    failed: false,
  });
  const fetchRef = useRef(fetchPage);
  fetchRef.current = fetchPage;
  const pending = useRef<AbortController | null>(null);

  useEffect(() => {
    pending.current?.abort();
    setMore({ id, pages: [], loading: false, failed: false });
  }, [id, first.data]);

  const firstItems = first.isPlaceholder ? [] : (first.data?.items ?? []);
  const extra = more.id === id ? more.pages.flat() : [];
  const items = useMemo(() => [...firstItems, ...extra], [firstItems, extra]);
  const total = first.data?.total ?? 0;
  const hasNextPage = !first.isLoading && !first.error && items.length < total && (more.pages.at(-1)?.length ?? 1) > 0;

  const fetchNextPage = useCallback(() => {
    if (!id || more.loading || !hasNextPage) return;
    const controller = new AbortController();
    pending.current = controller;
    setMore((m) => ({ ...m, loading: true, failed: false }));
    fetchRef.current(items.length, controller.signal).then(
      (page) => {
        if (controller.signal.aborted) return;
        setMore((m) => (m.id === id ? { id, pages: [...m.pages, page.items], loading: false, failed: false } : m));
      },
      (e) => {
        if (controller.signal.aborted || (e as { name?: string } | null)?.name === "AbortError") return;
        setMore((m) => ({ ...m, loading: false, failed: true }));
      },
    );
  }, [id, more.loading, hasNextPage, items.length]);

  return {
    items,
    total,
    meta: first.isPlaceholder ? undefined : first.data?.meta,
    isLoading: first.isLoading && !first.isPlaceholder,
    isPlaceholder: first.isPlaceholder,
    error: first.error,
    hasNextPage,
    isFetchingNextPage: more.loading,
    isFetchNextPageError: more.failed,
    fetchNextPage,
    refetch: first.refetch,
  };
}
