import { useLayoutEffect, useRef } from "react";
import { useQueryClient } from "@tanstack/react-query";

/** The Sites host owns declared private caches; legacy key shapes retain their own lifecycle. */
export function usePrivateQueryLifecycle(account: string | null) {
  const client = useQueryClient();
  const current = account?.toLowerCase() || null;
  const previous = useRef(current);
  useLayoutEffect(() => {
    const old = previous.current;
    previous.current = current;
    if (old === current) return;
    const filters = { queryKey: current ? ["private", old] : ["private"] };
    void client.cancelQueries(filters);
    client.removeQueries(filters);
  }, [client, current]);
}
