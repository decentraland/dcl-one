import { useQueryClient, type QueryKey } from "@tanstack/react-query";
import { useEffect } from "react";
import { useLocation, useNavigate } from "react-router";

const failedWithoutData = ({ state }: { state: { status: string; data: unknown } }) =>
  state.status === "error" && state.data === undefined;

/**
 * A route boundary's recovery for the required read that failed (web-app standards, "Required data on
 * client navigation"). Leaving the boundary releases that read only when it failed without data and
 * is idle and unobserved, so newer data or another owner's request stays. The returned retry resets
 * it and navigates to the same URL, which clears the router's error.
 */
export function useFailedReadRecovery(queryKey: QueryKey): () => Promise<void> {
  const client = useQueryClient();
  const navigate = useNavigate();
  const location = useLocation();
  const key = JSON.stringify(queryKey);
  useEffect(
    () => () => {
      void client.resetQueries({
        queryKey: JSON.parse(key) as QueryKey,
        exact: true,
        type: "inactive",
        predicate: (query) => failedWithoutData(query) && query.state.fetchStatus === "idle",
      });
    },
    [client, key],
  );
  return async () => {
    await client.resetQueries({ queryKey, exact: true, predicate: failedWithoutData });
    void navigate(`${location.pathname}${location.search}`, { replace: true });
  };
}
