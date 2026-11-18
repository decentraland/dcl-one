import { useQuery } from '@tanstack/react-query'
import { fetchCampaign, type Campaign } from '~/lib/campaign'
import { FeatureFlag } from '~/lib/featureFlags'
import { useFeatureFlag } from './useFeatureFlag'

const STALE_TIME_MS = 15 * 60_000

/** The running campaign, or null while the flag is off, the CMS has none, or the request failed. */
export function useCampaign(): Campaign | null {
  const flag = useFeatureFlag(FeatureFlag.CAMPAIGN)
  const { data } = useQuery({
    queryKey: ['campaign'],
    queryFn: fetchCampaign,
    enabled: flag.enabled,
    staleTime: STALE_TIME_MS
  })
  return data ?? null
}
