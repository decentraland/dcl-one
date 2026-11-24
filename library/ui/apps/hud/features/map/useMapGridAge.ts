// The parcel map's age label, shared by every map drawn from the map API's grid: the Explorer's map
// page (here) and ui3's LAND map.

import { useEffect, useState } from 'react'
import { fetchMapGridCheckedAt, mapGridAgeLabel, MAP_GRID_AGE_POLL_MS } from '@explorer-core/features/map/gridAge'

/**
 * "Updated 12 min ago" for a grid from the map API at `base` that was last found current over five
 * minutes ago; null while it is fresher, while its age is unknown and while the map is not shown
 * (`shown` false). It reads the age when the map is shown and every minute while it stays shown;
 * a read that fails keeps the last answer and its label goes on ageing.
 */
export function useMapGridAge(base: string, shown: boolean): string | null {
  const [checkedAt, setCheckedAt] = useState<number | null>(null)
  const [now, setNow] = useState(() => Date.now())
  useEffect(() => {
    if (!shown) return
    const controller = new AbortController()
    const read = (): void => {
      fetchMapGridCheckedAt(base, controller.signal).then(
        (at) => {
          if (controller.signal.aborted) return
          setCheckedAt(at)
          setNow(Date.now())
        },
        () => {
          if (!controller.signal.aborted) setNow(Date.now())
        }
      )
    }
    read()
    const timer = setInterval(read, MAP_GRID_AGE_POLL_MS)
    return () => {
      controller.abort()
      clearInterval(timer)
      // A map shown again waits for a new answer rather than showing the last one's label.
      setCheckedAt(null)
    }
  }, [base, shown])
  return shown ? mapGridAgeLabel(checkedAt, now) : null
}
