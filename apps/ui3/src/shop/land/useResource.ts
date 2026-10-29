import { useCallback, useEffect, useRef, useState } from "react";

export type Resource<T> =
  | { status: "idle"; data: null; error: null; reload(): void }
  | { status: "loading"; data: T | null; error: null; reload(): void }
  | { status: "ready"; data: T; error: null; reload(): void }
  | { status: "error"; data: T | null; error: unknown; reload(): void };

export function useResource<T>(load: ((signal: AbortSignal) => Promise<T>) | null, deps: readonly unknown[], initial?: T): Resource<T> {
  const [tick, setTick] = useState(0);
  const [state, setState] = useState<{ status: Resource<T>["status"]; data: T | null; error: unknown }>(() =>
    load && initial !== undefined ? { status: "ready", data: initial, error: null } : { status: load ? "loading" : "idle", data: null, error: null },
  );
  const skipFirst = useRef(!!load && initial !== undefined);
  const loadRef = useRef(load);
  loadRef.current = load;
  const reload = useCallback(() => setTick((t) => t + 1), []);

  useEffect(() => {
    const run = loadRef.current;
    if (skipFirst.current) {
      skipFirst.current = false;
      return;
    }
    if (!run) {
      setState((s) => (s.status === "idle" && s.data === null ? s : { status: "idle", data: null, error: null }));
      return;
    }
    const ctrl = new AbortController();
    setState((s) => ({ status: "loading", data: s.data, error: null }));
    run(ctrl.signal).then(
      (data) => {
        if (!ctrl.signal.aborted) setState({ status: "ready", data, error: null });
      },
      (error: unknown) => {
        if (ctrl.signal.aborted || (error as { name?: string } | null)?.name === "AbortError") return;
        setState((s) => ({ status: "error", data: s.data, error }));
      },
    );
    return () => ctrl.abort();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tick, !!load, ...deps]);

  return { ...state, reload } as Resource<T>;
}
