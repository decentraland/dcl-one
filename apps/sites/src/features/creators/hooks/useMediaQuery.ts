import { useEffect, useState } from 'react'

// `media.*` strings (~/styles/theme) are CSS at-rules; matchMedia wants the bare condition.
const toCondition = (query: string) => query.replace(/^@media\s+/, '')

/** Whether a `media` query currently matches; false where matchMedia is unavailable (tests, SSR). */
export function useMediaQuery(query: string): boolean {
  const condition = toCondition(query)
  const [matches, setMatches] = useState(() => window.matchMedia?.(condition).matches ?? false)

  useEffect(() => {
    if (!window.matchMedia) return
    const media = window.matchMedia(condition)
    const onChange = (event: MediaQueryListEvent) => setMatches(event.matches)
    media.addEventListener('change', onChange)
    return () => media.removeEventListener('change', onChange)
  }, [condition])

  return matches
}
