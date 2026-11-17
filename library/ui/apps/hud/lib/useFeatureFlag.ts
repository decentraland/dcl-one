// A rollout flag in React (the read itself: @explorer-core/lib/featureFlags).

import { useSyncExternalStore } from 'react'
import { featureFlag, featureFlagOverride, featureFlagsSettled, subscribeFeatureFlags, type Flag } from '@explorer-core/lib/featureFlags'

/** A flag `?ff=` forces at once; otherwise undefined until the session's read settles, then on only
 *  where the answer says `true`. */
export function useFeatureFlag(flag: Flag): boolean | undefined {
  return useSyncExternalStore(subscribeFeatureFlags, () => featureFlagOverride(flag) ?? (featureFlagsSettled() ? featureFlag(flag) : undefined))
}
