// Talks to the wearable-preview iframe directly: ui2's WearablePreview bakes its props into the iframe
// URL (a change reboots Unity, ~10s), so the mounted iframe keeps a frozen URL and every later change
// travels as an UPDATE message (~2s). The iframe replaces its overrides with each message, so the FULL
// option set is sent every time, and again after every boot (a reloaded iframe starts empty).
import { PreviewMessageType, type PreviewOptions } from '@dcl/schemas'

export const PREVIEW_UPDATE_DEBOUNCE_MS = 300

export type PreviewBridge = {
  /** Queues the full option set; coalesced and posted once the iframe is ready. */
  update(options: PreviewOptions): void
  /** How many times the iframe has announced itself ready. */
  readonly boots: number
  dispose(): void
}

type BridgeParams = {
  iframe: HTMLIFrameElement
  /** Called right before each post. */
  onPost?: (options: PreviewOptions) => void
  onBoot?: (boots: number) => void
  debounceMs?: number
  /** The window that receives the iframe's messages (tests pass a fake). */
  host?: Window
}

export function createPreviewBridge({
  iframe,
  onPost,
  onBoot,
  debounceMs = PREVIEW_UPDATE_DEBOUNCE_MS,
  host = window
}: BridgeParams): PreviewBridge {
  const origin = new URL(iframe.src, host.location?.href ?? 'http://localhost').origin
  let current: PreviewOptions | null = null
  let posted: string | null = null
  let ready = false
  let boots = 0
  let timer: ReturnType<typeof setTimeout> | null = null

  // The iframe rebuilds its scene for every update it receives, so an update that says nothing new is
  // a reload for nothing. A boot forces one through: the fresh scene knows none of it yet.
  function post(force = false) {
    timer = null
    if (!ready || !current || !iframe.contentWindow) return
    const serialized = JSON.stringify(current)
    if (!force && serialized === posted) return
    posted = serialized
    onPost?.(current)
    iframe.contentWindow.postMessage({ type: PreviewMessageType.UPDATE, payload: { options: current } }, origin)
  }

  function schedule() {
    if (timer !== null) clearTimeout(timer)
    timer = setTimeout(post, debounceMs)
  }

  function onMessage(event: MessageEvent<{ type?: string } | undefined>) {
    // Both checks: the window identifies the iframe, the origin rejects a page it may have navigated to.
    if (event.source !== iframe.contentWindow || event.origin !== origin) return
    if (event.data?.type !== PreviewMessageType.READY) return
    ready = true
    boots++
    onBoot?.(boots)
    // Nothing to coalesce on a boot: the scene is empty until it hears the current set.
    if (timer !== null) clearTimeout(timer)
    post(true)
  }

  host.addEventListener('message', onMessage as EventListener)

  return {
    update(options) {
      current = options
      if (ready) schedule()
    },
    get boots() {
      return boots
    },
    dispose() {
      host.removeEventListener('message', onMessage as EventListener)
      if (timer !== null) clearTimeout(timer)
      timer = null
    }
  }
}
