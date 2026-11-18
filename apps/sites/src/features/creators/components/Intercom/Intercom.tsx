import { useEffect, useMemo, useState } from 'react'
import { useLocation } from 'react-router-dom'
import { config } from '~/config'
import { useAfterLoadIdle } from '~/hooks/useAfterLoadIdle'
import { getAnonymousId, onAnalyticsReady } from '~/lib/analytics'
import { useWallet } from '~/store/wallet'

type IntercomFn = (command: string, payload?: Record<string, unknown>) => void
type IntercomWindow = Window & { Intercom?: IntercomFn }

const WIDGET_URL = 'https://widget.intercom.io/widget'
const DEFAULT_VERTICAL_PADDING = 20
// Lifts the launcher above the bottom controls of full-height pages: the item editor's Revert/Save footer
// and the preview's playback bar / validation badge row.
const RAISED_VERTICAL_PADDING = 84
const RAISED_PATHS = ['/collections/editor', '/live-preview']

function intercom(): IntercomFn | undefined {
  return typeof window === 'undefined' ? undefined : (window as IntercomWindow).Intercom
}

let injecting: Promise<void> | undefined

/** Loads the widget script once per page, whoever asks first. A failed load is retried on the next call. */
function injectWidget(appId: string): Promise<void> {
  if (intercom()) return Promise.resolve()
  injecting ??= new Promise<void>((resolve, reject) => {
    const script = document.createElement('script')
    script.async = true
    script.src = `${WIDGET_URL}/${encodeURIComponent(appId)}`
    script.addEventListener('load', () => resolve())
    script.addEventListener('error', () => {
      // Without this a single network blip would keep the rejected promise, and support would stay
      // unreachable for the rest of the visit.
      injecting = undefined
      script.remove()
      reject(new Error('Intercom widget failed to load'))
    })
    document.head.appendChild(script)
  })
  return injecting
}

/**
 * The support widget. Renders nothing itself — it boots Intercom and keeps it in sync with who is
 * signed in, plus Segment's anonymous id so a conversation can be matched to that visitor's events.
 */
export const Intercom = () => {
  const appId = config.get('INTERCOM_APP_ID', '')
  const session = useWallet(state => state.session)
  const address = session?.address
  const providerType = session?.providerType
  const [anonymousId, setAnonymousId] = useState<string>()
  const { pathname } = useLocation()
  const isRaised = RAISED_PATHS.some(path => pathname.startsWith(path))
  // The widget is ~300KB of third-party script nobody needs during the first paint.
  const idle = useAfterLoadIdle()

  useEffect(() => {
    // The id only exists once analytics.js has loaded; it never changes afterwards.
    onAnalyticsReady(() => setAnonymousId(current => current ?? getAnonymousId()))
  }, [])

  const data = useMemo(() => {
    const attributes: Record<string, unknown> = {
      vertical_padding: isRaised ? RAISED_VERTICAL_PADDING : DEFAULT_VERTICAL_PADDING
    }
    if (address) attributes['Wallet'] = address.toLowerCase()
    if (providerType) attributes['Wallet type'] = providerType
    if (anonymousId) attributes['anon_id'] = anonymousId
    return attributes
  }, [address, providerType, anonymousId, isRaised])

  useEffect(() => {
    if (!appId || !idle) return
    let cancelled = false
    void injectWidget(appId)
      .then(() => {
        if (cancelled) return
        const client = intercom()
        client?.('reattach_activator')
        client?.('update', { app_id: appId, ...data })
      })
      .catch((error: unknown) => console.error('Could not render Intercom', error))
    return () => {
      cancelled = true
    }
  }, [appId, data, idle])

  useEffect(() => {
    return () => {
      intercom()?.('shutdown')
      // A remount has to inject again: the cached promise would resolve to a widget that is now dead.
      injecting = undefined
    }
  }, [])

  return null
}
