import { useCallback, useEffect, useRef, useState } from 'react'
import type { LoginDriver } from '../../engine/driver'
import type { ConnectivitySnapshot } from '../../engine/generated'

export interface ConnectivityState {
  connectionLost: boolean
  probing: boolean
  rooms: Array<{ scope: string; state: string; retryAt?: number | null }>
}

/** Subscribe to engine policy through the trusted scene bridge on native and web.
 * The HUD neither probes the network nor decides when a connection should retry. */
export function useConnectivity(driver: LoginDriver | null, active: boolean) {
  const [state, setState] = useState<ConnectivitySnapshot | null>(null)
  const pending = useRef<{ requestId: string; promise: Promise<void>; finish: (error?: Error) => void } | null>(null)
  useEffect(() => {
    setState(null)
    if (!active || driver == null) return
    const off = driver.on(message => {
      if (message.kind === 'connectivity') {
        setState(previous => JSON.stringify(previous) === JSON.stringify(message.state) ? previous : message.state)
      } else if (message.kind === 'connectivityRetry' && message.requestId === pending.current?.requestId) {
        pending.current.finish(message.error == null ? undefined : new Error(message.error))
      }
    })
    driver.send({ kind: 'getConnectivity' })
    return () => {
      off()
      pending.current?.finish(new Error('Connectivity session ended'))
    }
  }, [driver, active])
  const retry = useCallback(() => {
    if (!active || driver == null) return
    if (pending.current != null) return pending.current.promise
    const requestId = crypto.randomUUID()
    let resolve!: () => void
    let reject!: (error: Error) => void
    const promise = new Promise<void>((accept, fail) => { resolve = accept; reject = fail })
    const finish = (error?: Error): void => {
      if (pending.current?.requestId !== requestId) return
      clearTimeout(timer)
      pending.current = null
      if (error == null) resolve()
      else reject(error)
    }
    const timer = setTimeout(() => finish(new Error('Connectivity retry timed out')), 10_000)
    pending.current = { requestId, promise, finish }
    try { driver.send({ kind: 'retryConnectivity', requestId }) }
    catch (error) { finish(error instanceof Error ? error : new Error(String(error))) }
    return promise
  }, [driver, active])
  return { state: active ? state : null, retry }
}
