import { useCallback, useSyncExternalStore } from "react";

/** Whether `query` matches; `serverValue` on the server and in a browser without `matchMedia`. */
export function useMediaQuery(query: string, serverValue = false): boolean {
  const subscribe = useCallback(
    (onChange: () => void) => {
      const list = window.matchMedia?.(query);
      list?.addEventListener?.("change", onChange);
      return () => list?.removeEventListener?.("change", onChange);
    },
    [query],
  );
  return useSyncExternalStore(
    subscribe,
    () => window.matchMedia?.(query).matches ?? serverValue,
    () => serverValue,
  );
}
