// The Shop's rollout flags as the web Shop reads them: the feature-flags service's `dapps.json`, under the
// same keys (`dapps-<feature>`), so a flag turned on upstream is on in both Shops. Through TanStack, the
// HUD's only server cache (not the web Shop's module snapshot): read for a minute, each read given 3s and
// one retry straight away, and the last answer kept while a refetch runs or after one fails, so a flag
// doesn't switch itself off mid-session.
//
// The web Shop's fetchers this page reuses (discovery/api, through discovery/flags) read creator sales
// from that module's snapshot rather than from an argument, and drop a sale's prices without it. Each
// answer is handed to them before it reaches the cache, so the listings they map agree with the keys the
// answer changes. Transitional: it goes when those fetchers take the flag as an input.

import { queryOptions, useQuery } from '@tanstack/react-query'
import { primeRemoteFlags, readRemoteFlags, type RemoteFlag } from '../../../shop/discovery/lib/remoteFlags'

const READ_TIMEOUT_MS = 3_000

export const flagQueries = {
  dapps: () =>
    queryOptions({
      queryKey: ['featureFlags', 'dapps'] as const,
      queryFn: async ({ signal }) => {
        // The variants go with the flags: priming without them would drop an address list (My Store's) for
        // the rest of the document, the web Shop's pages included.
        const { flags, variants } = await readRemoteFlags(AbortSignal.any([signal, AbortSignal.timeout(READ_TIMEOUT_MS)]))
        primeRemoteFlags(flags, variants)
        return flags
      },
      staleTime: 60_000,
      retry: 1,
      retryDelay: 0
    })
}

/** One rollout flag as the service answers it: undefined until it does, off when the file can't be read. */
export function useShopFlag(flag: RemoteFlag): boolean | undefined {
  const { data, isError } = useQuery({ ...flagQueries.dapps(), select: (flags) => flags[`dapps-${flag}`] === true })
  return data ?? (isError ? false : undefined)
}
