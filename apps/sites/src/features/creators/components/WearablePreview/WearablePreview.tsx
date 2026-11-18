import { useEffect, useMemo, useRef } from 'react'
import { PreviewMessageType, sendMessage } from '@dcl/schemas/dist/dapps/preview'
import { config } from '@/data/lib/creators/config'
import styles from './WearablePreview.module.css'
import type { WearablePreviewProps } from './types'

// How long option changes are coalesced before they are pushed to the iframe.
const UPDATE_DEBOUNCE_MS = 500

// The option each prop travels as in the iframe's URL, where its name differs.
const PARAM_NAMES: Record<string, string> = { urns: 'urn', unityMode: 'mode' }

function toUrl(baseUrl: string, options: Record<string, unknown>, dev: boolean): string {
  const params = new URLSearchParams()
  for (const [key, value] of Object.entries(options)) {
    if (key === 'blob' || value === undefined || value === null || value === '') continue
    for (const item of Array.isArray(value) ? value : [value]) params.append(PARAM_NAMES[key] ?? key, String(item))
  }
  if (dev) params.set('env', 'dev')
  return `${baseUrl}?${params}`
}

const blobIds = new WeakMap<object, number>()
let nextBlobId = 1

function blobId(blob: object): number {
  let id = blobIds.get(blob)
  if (!id) blobIds.set(blob, (id = nextBlobId++))
  return id
}

/** The wearable preview in an iframe: its options travel in the URL, and later changes (and a `blob`, always) as messages. */
export function WearablePreview({ id, dev = false, onLoad, onError, onUpdate, ...options }: WearablePreviewProps) {
  const baseUrl = config.get('WEARABLE_PREVIEW_URL')
  const frame = useRef<HTMLIFrameElement>(null)
  const callbacks = useRef({ onLoad, onError, onUpdate })
  callbacks.current = { onLoad, onError, onUpdate }

  // A blob holds files, which JSON drops: it is told apart by identity.
  const serialized = JSON.stringify({ ...options, blob: options.blob ? blobId(options.blob) : undefined })
  const url = useMemo(() => toUrl(baseUrl, options, dev), [baseUrl, serialized, dev])
  // The URL the iframe was mounted with: changing `src` would reload the preview, so it never changes.
  const src = useRef(url).current

  const ready = useRef(false)
  // What the iframe already has. The mount URL carries everything but a blob.
  const delivered = useRef(options.blob ? null : serialized)

  const push = useRef(() => undefined as void)
  push.current = () => {
    const target = frame.current?.contentWindow
    if (!ready.current || !target || delivered.current === serialized) return
    delivered.current = serialized
    sendMessage(target, PreviewMessageType.UPDATE, { options } as never)
    callbacks.current.onUpdate?.()
  }

  useEffect(() => {
    const onMessage = (event: MessageEvent) => {
      // Another preview on the page posts the same messages: only this iframe's count.
      if (event.source !== frame.current?.contentWindow || event.origin !== new URL(baseUrl).origin) return
      const { type, payload } = (event.data ?? {}) as { type?: string; payload?: { renderer?: never; message?: string } }
      if (type === PreviewMessageType.READY) {
        ready.current = true
        push.current()
      } else if (type === PreviewMessageType.LOAD) {
        callbacks.current.onLoad?.(payload?.renderer)
      } else if (type === PreviewMessageType.ERROR) {
        callbacks.current.onError?.(new Error(payload?.message))
      }
    }
    window.addEventListener('message', onMessage)
    return () => window.removeEventListener('message', onMessage)
  }, [baseUrl])

  useEffect(() => {
    const timeout = setTimeout(() => push.current(), UPDATE_DEBOUNCE_MS)
    return () => clearTimeout(timeout)
  }, [serialized])

  return <iframe id={id} ref={frame} className={styles.frame} src={src} allow="autoplay" title="" />
}
