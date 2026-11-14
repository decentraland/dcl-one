import { useQueries } from "@tanstack/react-query";
import { useMemo } from "react";

import {
  creatorBatches,
  creatorProfilesQuery,
  type CreatorProfiles,
} from "@/data/lib/catalyst/social/creator-profiles";

/**
 * The faces behind the page's by-lines: one batched profile read per group of cards, each asking only
 * for wallets no earlier group asked for. Cards show the initial while a read runs or if it fails.
 */
export function useCreatorProfiles(groups: readonly (readonly (string | null | undefined)[])[]): CreatorProfiles {
  const batches = useMemo(() => creatorBatches(groups), [groups]);
  return useQueries({
    queries: batches.map((addresses) => creatorProfilesQuery(addresses)),
    combine: (results) => Object.assign({}, ...results.map((result) => result.data ?? {})) as CreatorProfiles,
  });
}
