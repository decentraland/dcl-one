import { useQuery } from '@tanstack/react-query'
import { type FeatureFlag, getIsFeatureEnabled } from '~/lib/featureFlags'

const STALE_TIME_MS = 60_000

export type FeatureFlagState = {
  /** Fails closed: a flag still being read, an unreachable service or an absent flag is off. */
  enabled: boolean
  /** The flag is still being read, so `enabled` is the fallback rather than an answer. */
  isLoading: boolean
}

type Options = {
  /** Skip the read entirely (the flag's answer can't change anything): reports `off`, settled. */
  enabled?: boolean
}

/**
 * Whether a Decentraland feature flag is on. A surface that would flicker (a payment method, a
 * renderer) waits on `isLoading` instead of rendering the fallback first.
 */
export function useFeatureFlag(flag: FeatureFlag, { enabled = true }: Options = {}): FeatureFlagState {
  const query = useQuery({
    queryKey: ['feature-flag', flag],
    queryFn: () => getIsFeatureEnabled(flag),
    enabled,
    staleTime: STALE_TIME_MS
  })
  return { enabled: query.data === true, isLoading: query.isLoading }
}
