import { useEffect, useRef } from 'react'
import { useMediaQuery } from '~/hooks/useMediaQuery'
import { OVERVIEW_MOBILE_QUERY, trackSectionViewed, type OverviewSection } from '~/lib/overviewAnalytics'

/** Fires `Section Viewed` once, the first time `inView` becomes true for the section. */
export function useSectionViewed(section: OverviewSection, inView: boolean): void {
  const mobile = useMediaQuery(OVERVIEW_MOBILE_QUERY)
  const tracked = useRef(false)

  useEffect(() => {
    if (!inView || tracked.current) return
    tracked.current = true
    trackSectionViewed(section, mobile)
  }, [section, inView, mobile])
}
