import { useEffect } from 'react'

/** While active, reload / tab close gets the browser's native "leave page?" prompt (custom UI isn't allowed there). */
export function useBeforeUnloadGuard(active: boolean) {
  useEffect(() => {
    if (!active) return
    const handler = (event: BeforeUnloadEvent) => {
      event.preventDefault()
      event.returnValue = ''
    }
    window.addEventListener('beforeunload', handler)
    return () => window.removeEventListener('beforeunload', handler)
  }, [active])
}
