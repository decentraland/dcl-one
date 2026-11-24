// The parcel map's age label, shared by every map drawn from the map API's grid: the Explorer's map
// page (here) and the Shop's LAND pages (ui3's `MapGridAge`).

import { useEffect, useState, useSyncExternalStore } from 'react'
import { fetchMapGridAge, mapGridAgeLabel, MAP_GRID_AGE_POLL_MS, type MapGridAge } from '@explorer-core/features/map/gridAge'

function subscribeVisibility(onChange: () => void): () => void {
  document.addEventListener('visibilitychange', onChange)
  return () => document.removeEventListener('visibilitychange', onChange)
}

const pageVisible = (): boolean => document.visibilityState !== 'hidden'

/**
 * "Updated 12 min ago" for a grid from the map API at `base` that was last found current over five
 * minutes ago; null while it is fresher, while its age is unknown and while the map is not shown
 * (`shown` false, or the page hidden). It reads the age when the map is shown and every minute
 * while it stays shown, giving up a read still pending when the next starts, so an older answer
 * never lands after a newer one. A read that fails keeps the last answer and its label goes on
 * ageing. The age is measured on the server's clock (`MapGridAge.skew`).
 */
export function useMapGridAge(base: string, shown: boolean): string | null {
  const visible = useSyncExternalStore(subscribeVisibility, pageVisible, () => true)
  const active = shown && visible
  const [age, setAge] = useState<MapGridAge | null>(null)
  const [now, setNow] = useState(() => Date.now())
  useEffect(() => {
    if (!active) return
    let pending: AbortController | null = null
    const read = (): void => {
      pending?.abort()
      const controller = new AbortController()
      pending = controller
      fetchMapGridAge(base, controller.signal).then(
        (answer) => {
          if (controller.signal.aborted) return
          setAge(answer)
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
      pending?.abort()
      clearInterval(timer)
      // A map shown again waits for a new answer rather than showing the last one's label.
      setAge(null)
    }
  }, [base, active])
  return active && age != null ? mapGridAgeLabel(age.checkedAt, now + age.skew) : null
}
