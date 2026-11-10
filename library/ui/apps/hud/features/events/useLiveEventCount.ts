// The live-events count the sidebar shows, refreshed while it is mounted.
import { useEffect, useState } from 'react'
import { fetchEvents, LIVE_COUNT_REFRESH_MS } from '@explorer-core/features/events/eventsApi'

/** Live events right now; 0 while loading or when the service is unreachable (as Unity). */
export function useLiveEventCount(): number {
  const [count, setCount] = useState(0)
  useEffect(() => {
    const ctrl = new AbortController()
    const load = (): void => {
      fetchEvents('live', ctrl.signal).then(
        (list) => setCount(list.length),
        () => !ctrl.signal.aborted && setCount(0)
      )
    }
    load()
    const id = setInterval(load, LIVE_COUNT_REFRESH_MS)
    return () => {
      ctrl.abort()
      clearInterval(id)
    }
  }, [])
  return count
}
