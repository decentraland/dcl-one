import { useLayoutEffect, useRef } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { CREDITS_BALANCE_QUERY_KEY } from "@shop/credits/balance-query";

/** Clear Sites private queries and the shared balance key when the owner changes. */
export function usePrivateQueryLifecycle(account: string | null) {
  const client = useQueryClient();
  const current = account?.toLowerCase() || null;
  const previous = useRef(current);
  useLayoutEffect(() => {
    const old = previous.current;
    previous.current = current;
    if (old === current) return;
    for (const key of ["private", CREDITS_BALANCE_QUERY_KEY]) {
      const filters = { queryKey: current ? [key, old] : [key] };
      void client.cancelQueries(filters);
      client.removeQueries(filters);
    }
  }, [client, current]);
}
