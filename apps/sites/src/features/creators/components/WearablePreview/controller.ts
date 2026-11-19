import mitt from 'mitt'
import { PreviewMessageType, sendMessage } from '@dcl/schemas/dist/dapps/preview'
import type { WearablePreviewController } from './types'

const REQUEST_TIMEOUT_MS = 10_000

type EmoteEvents = WearablePreviewController['emote']['events']
type Pending = {
  /** The preview window asked: only its answer settles the request. */
  source: Window
  resolve: (value: unknown) => void
  reject: (reason: Error) => void
  timeout: ReturnType<typeof setTimeout>
}

const pending = new Map<string, Pending>()
const emoteEvents = new WeakMap<object, EmoteEvents>()
let nonce = 0
let listening = false

function onMessage(event: MessageEvent): void {
  const { type, payload } = (event.data ?? {}) as { type?: string; payload?: Record<string, unknown> }
  if (!payload) return
  if (type === PreviewMessageType.CONTROLLER_RESPONSE) {
    const request = pending.get(String(payload.id))
    if (!request || event.source !== request.source) return
    pending.delete(String(payload.id))
    clearTimeout(request.timeout)
    if (payload.ok) request.resolve(payload.result)
    else request.reject(new Error(String(payload.error)))
  } else if (type === PreviewMessageType.EMOTE_EVENT && event.source && typeof payload.type === 'string') {
    emoteEvents.get(event.source)?.emit(payload.type as never, payload.payload as never)
  }
}

/** The controller of the preview iframe mounted under `id`: its calls are requests posted to the iframe. */
export function createWearablePreviewController(id: string): WearablePreviewController {
  const frame = () => (document.getElementById(id) as HTMLIFrameElement | null)?.contentWindow ?? null
  const target = frame()
  if (!target) throw new Error(`Could not find an iframe with id="${id}"`)
  if (!listening) {
    window.addEventListener('message', onMessage)
    listening = true
  }
  let events = emoteEvents.get(target)
  if (!events) emoteEvents.set(target, (events = mitt() as EmoteEvents))

  const request = <T>(namespace: string, method: string, params: unknown[] = []): Promise<T> =>
    new Promise<T>((resolve, reject) => {
      const window = frame()
      if (!window) return reject(new Error('iframe not found or not accessible'))
      const requestId = `${id}-${nonce++}`
      const timeout = setTimeout(() => {
        pending.delete(requestId)
        reject(new Error(`Request timeout for ${method}`))
      }, REQUEST_TIMEOUT_MS)
      pending.set(requestId, { source: window, resolve: resolve as (value: unknown) => void, reject, timeout })
      sendMessage(window, PreviewMessageType.CONTROLLER_REQUEST, { id: requestId, namespace, method, params } as never)
    })

  return {
    scene: {
      getScreenshot: (width, height) => request('scene', 'getScreenshot', [width, height]),
      getMetrics: () => request('scene', 'getMetrics'),
      changeZoom: zoom => request('scene', 'changeZoom', [zoom]),
      panCamera: offset => request('scene', 'panCamera', [offset])
    },
    emote: {
      getLength: () => request('emote', 'getLength'),
      goTo: seconds => request('emote', 'goTo', [seconds]),
      play: () => request('emote', 'play'),
      pause: () => request('emote', 'pause'),
      stop: () => request('emote', 'stop'),
      hasSound: () => request('emote', 'hasSound'),
      enableSound: () => request('emote', 'enableSound'),
      disableSound: () => request('emote', 'disableSound'),
      events
    },
    physics: {
      setSpringBonesParams: (itemHash, params) => request('physics', 'setSpringBonesParams', [itemHash, params])
    }
  }
}
