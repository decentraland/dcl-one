import { useEffect, useState } from 'react'
import { PreviewMessageType } from '@dcl/schemas/dist/dapps/preview'
import { createWearablePreviewController } from './controller'
import type { WearablePreviewController } from './types'

// The iframes that have announced their scene, so a control mounted afterwards knows it can ask.
const loaded = new WeakSet<object>()
if (typeof window !== 'undefined') {
  window.addEventListener('message', event => {
    if (event.data?.type === PreviewMessageType.LOAD && event.source) loaded.add(event.source)
  })
}

const frameOf = (previewId: string) => (document.getElementById(previewId) as HTMLIFrameElement | null)?.contentWindow

/** The controller a control talks to (the page's own when it shares one) and whether its preview has loaded. */
export function usePreviewController(previewId: string, shared?: WearablePreviewController) {
  const [controller, setController] = useState<WearablePreviewController | undefined>(shared)
  const [isReady, setIsReady] = useState(false)

  useEffect(() => {
    setController(shared ?? createWearablePreviewController(previewId))
    const frame = frameOf(previewId)
    if (frame && loaded.has(frame)) {
      setIsReady(true)
      return
    }
    const onMessage = (event: MessageEvent) => {
      if (event.data?.type === PreviewMessageType.LOAD && event.source === frameOf(previewId)) setIsReady(true)
    }
    window.addEventListener('message', onMessage)
    return () => {
      window.removeEventListener('message', onMessage)
      setIsReady(false)
    }
  }, [previewId, shared])

  return { controller, isReady }
}
