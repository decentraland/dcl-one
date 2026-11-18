import { useCallback, useEffect, useRef, useState } from 'react'
import { track } from '~/lib/analytics'
import { blobsAreEqual } from '~/lib/blobs'
import {
  LivePreviewError,
  fetchBridgeState,
  fetchModelBlob,
  isSameModelMetadata,
  queryLocalNetworkPermission,
  type BridgeState,
  type LivePreviewErrorCode
} from '~/lib/livePreview'
import { captureError } from '~/lib/monitoring'

export type LiveBridgeStatus = 'disconnected' | 'connecting' | 'connected' | 'error'
export type LiveBridgeErrorCode = LivePreviewErrorCode | 'unreachable' | 'permission_denied'

export type LiveBridge = {
  status: LiveBridgeStatus
  errorCode: LiveBridgeErrorCode | null
  /** Chromium's Local Network Access permission; null where no prompt applies. */
  permission: PermissionState | null
  state: BridgeState | null
  glb: Blob | null
  lastUpdateAt: number | null
  /** When a refresh (button or tab return) last confirmed the model on screen is the latest export. */
  lastCheckedAt: number | null
  /** How many models this session applied. */
  pushCount: number
  isRefreshing: boolean
  connect: () => void
  disconnect: () => void
  refresh: () => void
}

// Bridges that don't long-poll answer `/state?since=` at once; this is the interval between asks then.
const POLL_INTERVAL_MS = 2_000
// A `since` reply this fast with an unchanged version means the bridge ignored the param.
const LONG_POLL_FALLBACK_MS = 1_000
const ERROR_POLL_INTERVAL_MS = 15_000

/**
 * Watches the Blender bridge: long-polls `/state`, pulls `/model.glb` when the version moves and hands
 * the fresh blob over, skipping re-exports that changed nothing. Pauses while the tab is hidden and asks
 * again on return; a forced refresh cuts an in-flight long-poll short.
 */
export function useLiveBridge(bridgeUrl: string): LiveBridge {
  const [status, setStatus] = useState<LiveBridgeStatus>('disconnected')
  const [errorCode, setErrorCode] = useState<LiveBridgeErrorCode | null>(null)
  const [permission, setPermission] = useState<PermissionState | null>(null)
  const [state, setState] = useState<BridgeState | null>(null)
  const [glb, setGlb] = useState<Blob | null>(null)
  const [lastUpdateAt, setLastUpdateAt] = useState<number | null>(null)
  const [lastCheckedAt, setLastCheckedAt] = useState<number | null>(null)
  const [pushCount, setPushCount] = useState(0)
  const [isRefreshing, setRefreshing] = useState(false)

  // The loop reads these without re-subscribing.
  const connectedRef = useRef(false)
  const pollingRef = useRef(false)
  const versionRef = useRef<BridgeState['version'] | null>(null)
  const stateRef = useRef<BridgeState | null>(null)
  const glbRef = useRef<Blob | null>(null)
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const abortRef = useRef<AbortController | null>(null)
  // A refresh asks without `since`: a long-polling bridge would otherwise hold the request, and the
  // spinner with it, until the next export.
  const forcedRef = useRef(false)
  // Funnel events fire once per connection, and once per failure kind.
  const announcedRef = useRef(false)
  const lastErrorRef = useRef<LiveBridgeErrorCode | null>(null)
  const lastPushAtRef = useRef<number | null>(null)

  const clearTimer = () => {
    if (timerRef.current) clearTimeout(timerRef.current)
    timerRef.current = null
  }

  const poll = useCallback(async () => {
    // Re-entrant calls (a tab return raises visibilitychange and focus together) must not fork a second chain.
    if (!connectedRef.current || pollingRef.current) return
    pollingRef.current = true
    const controller = new AbortController()
    abortRef.current = controller
    const startedAt = Date.now()
    const previousVersion = versionRef.current
    const forced = forcedRef.current
    forcedRef.current = false
    let delay = POLL_INTERVAL_MS
    try {
      const next = await fetchBridgeState(bridgeUrl, {
        since: forced ? null : previousVersion,
        signal: controller.signal
      })
      // After every await: a disconnect mid-flight must not overwrite the disconnected status.
      if (!connectedRef.current) return
      setStatus('connected')
      setErrorCode(null)
      lastErrorRef.current = null
      const longPoll = !forced && Date.now() - startedAt >= LONG_POLL_FALLBACK_MS
      if (!announcedRef.current && !forced) {
        announcedRef.current = true
        track('Live preview connect', { long_poll: longPoll })
      }

      const changed = next.version !== previousVersion
      delay = changed || longPoll ? 0 : POLL_INTERVAL_MS
      if (!changed) {
        if (forced) setLastCheckedAt(Date.now())
        return
      }

      const model = await fetchModelBlob(bridgeUrl, controller.signal)
      if (!connectedRef.current) return
      const sameBytes = glbRef.current !== null && (await blobsAreEqual(glbRef.current, model))
      if (!connectedRef.current) return
      versionRef.current = next.version
      // A save or an exporter side effect re-exported an unchanged scene: nothing to redraw.
      if (sameBytes && isSameModelMetadata(stateRef.current, next)) {
        if (forced) setLastCheckedAt(Date.now())
        return
      }

      stateRef.current = next
      setState(next)
      if (!sameBytes) {
        glbRef.current = model
        setGlb(model)
      }
      const now = Date.now()
      setLastUpdateAt(now)
      setPushCount(count => count + 1)
      track('Live preview model', {
        item_type: next.type ?? (next.category ? 'wearable' : undefined),
        category: next.category || undefined,
        since_last_ms: lastPushAtRef.current === null ? undefined : now - lastPushAtRef.current
      })
      lastPushAtRef.current = now
    } catch (error) {
      if (controller.signal.aborted) {
        // A refresh cut the request short: the next tick runs at once.
        delay = 0
      } else {
        if (!connectedRef.current) return
        delay = ERROR_POLL_INTERVAL_MS
        let code: LiveBridgeErrorCode = error instanceof LivePreviewError ? error.code : 'unreachable'
        // A request the browser blocked rejects like an unreachable bridge; the permission tells them apart.
        if (code === 'unreachable') {
          const current = await queryLocalNetworkPermission()
          if (!connectedRef.current) return
          if (current) setPermission(current.state)
          if (current?.state === 'denied') code = 'permission_denied'
        } else {
          // The bridge is there but the pair no longer agree on the contract: schema drift to look into.
          captureError(error, { flow: 'live_preview', step: 'poll', bridge_status: (error as LivePreviewError).status })
        }
        setStatus('error')
        setErrorCode(code)
        if (lastErrorRef.current !== code) {
          lastErrorRef.current = code
          track('Live preview connect error', { error: code })
        }
      }
    } finally {
      pollingRef.current = false
      if (abortRef.current === controller) abortRef.current = null
      // An aborted tick is followed at once by the one the refresh asked for: keep the spinner until then.
      if (!controller.signal.aborted || !connectedRef.current) setRefreshing(false)
      // The visibility handler restarts the loop when the tab comes back.
      if (connectedRef.current && !document.hidden) {
        timerRef.current = setTimeout(() => void poll(), delay)
      }
    }
  }, [bridgeUrl])

  const stop = useCallback(() => {
    connectedRef.current = false
    clearTimer()
    abortRef.current?.abort()
  }, [])

  const disconnect = useCallback(() => {
    stop()
    versionRef.current = null
    setStatus('disconnected')
    setErrorCode(null)
    setRefreshing(false)
  }, [stop])

  const connect = useCallback(() => {
    stop()
    versionRef.current = null
    connectedRef.current = true
    setStatus('connecting')
    setErrorCode(null)
    void poll()
  }, [poll, stop])

  const refresh = useCallback(() => {
    if (!connectedRef.current) return
    setRefreshing(true)
    forcedRef.current = true
    clearTimer()
    if (abortRef.current) abortRef.current.abort()
    else void poll()
  }, [poll])

  // Mount-only on purpose: reconnecting is the creator's call through Connect, never a reaction to the
  // URL field changing.
  useEffect(() => {
    connect()
    return stop
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  useEffect(() => {
    let current: PermissionStatus | null = null
    let cancelled = false
    const onChange = () => setPermission(current?.state ?? null)
    void queryLocalNetworkPermission().then(result => {
      if (cancelled || !result) return
      current = result
      current.addEventListener('change', onChange)
      onChange()
    })
    return () => {
      cancelled = true
      current?.removeEventListener('change', onChange)
    }
  }, [])

  // Polling pauses while hidden but keeps running while merely unfocused: Blender side by side with the
  // browser is the main case. Either return asks the bridge right away.
  useEffect(() => {
    const onReturn = () => {
      if (!document.hidden) refresh()
    }
    document.addEventListener('visibilitychange', onReturn)
    window.addEventListener('focus', onReturn)
    return () => {
      document.removeEventListener('visibilitychange', onReturn)
      window.removeEventListener('focus', onReturn)
    }
  }, [refresh])

  return {
    status,
    errorCode,
    permission,
    state,
    glb,
    lastUpdateAt,
    lastCheckedAt,
    pushCount,
    isRefreshing,
    connect,
    disconnect,
    refresh
  }
}
