// A rollout flag in React (the read itself: @explorer-core/lib/featureFlags).

import { useSyncExternalStore } from 'react'
import { featureFlag, featureFlagsSettled, subscribeFeatureFlags, type Flag } from '@explorer-core/lib/featureFlags'

/** Undefined until the session's read settles, then on only where the answer says `true`. */
export function useFeatureFlag(flag: Flag): boolean | undefined {
  return useSyncExternalStore(subscribeFeatureFlags, () => (featureFlagsSettled() ? featureFlag(flag) : undefined))
}
