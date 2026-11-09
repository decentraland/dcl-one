import { useCallback, useEffect, useRef, useState } from 'react'
import type { LoginDriver } from '../../engine/driver'

export interface ConnectivityState {
  connectionLost: boolean
  probing: boolean
  rooms: Array<{ scope: string; state: string; retryAt?: number | null }>
}

/** Read the engine policy through the same trusted console seam on native and web.
 * The HUD neither probes the network nor decides when a connection should retry. */
export function useConnectivity(driver: LoginDriver | null, active: boolean) {
  const [state, setState] = useState<ConnectivityState | null>(null)
  const refresh = useRef<(() => Promise<void>) | null>(null)
  useEffect(() => {
    setState(null)
    if (!active || !driver?.command) return
    let cancelled = false
    let pending = false
    const read = async () => {
      if (pending || cancelled) return
      pending = true
      try {
        const next = JSON.parse(await driver.command!('/connectivity_state')) as ConnectivityState
        if (!cancelled && typeof next.connectionLost === 'boolean' && typeof next.probing === 'boolean' && Array.isArray(next.rooms)) setState(next)
      } catch {
        // Console/engine startup errors are handled by the existing session crash path.
      } finally { pending = false }
    }
    refresh.current = read
    void read()
    const timer = setInterval(() => void read(), 1000)
    return () => { cancelled = true; clearInterval(timer); refresh.current = null }
  }, [driver, active])
  const retry = useCallback(() => {
    if (!active) return
    return driver?.command?.('/connectivity_retry').then(() => { void refresh.current?.() })
  }, [driver, active])
  return { state: active ? state : null, retry }
}
