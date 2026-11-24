import { useCallback, useSyncExternalStore } from 'react'

// `media.*` strings (~/styles/theme) are CSS at-rules; matchMedia wants the bare condition.
const toCondition = (query: string) => query.replace(/^@media\s+/, '')

/**
 * Whether a media query currently matches. `serverValue` is the answer for the server render and
 * hydration, and where matchMedia is unavailable (tests).
 */
export function useMediaQuery(query: string, serverValue = false): boolean {
  const condition = toCondition(query)
  const subscribe = useCallback(
    (onChange: () => void) => {
      const list = window.matchMedia?.(condition)
      list?.addEventListener?.('change', onChange)
      return () => list?.removeEventListener?.('change', onChange)
    },
    [condition]
  )
  return useSyncExternalStore(
    subscribe,
    () => window.matchMedia?.(condition).matches ?? serverValue,
    () => serverValue
  )
}
