import { useEffect, useState, type RefObject } from 'react'
import { HEIGHT_MESSAGE } from './messages'

/** The height a canvas reports for its document (HEIGHT_MESSAGE), never under `minimum`. */
export function useCanvasHeight(
  frame: RefObject<HTMLIFrameElement | null>,
  minimum: number,
  initial = minimum
): number {
  const [height, setHeight] = useState(initial)
  useEffect(() => {
    const receive = (event: MessageEvent) => {
      if (event.source !== frame.current?.contentWindow) return
      const data = event.data as { type?: unknown; height?: unknown } | null
      if (data?.type === HEIGHT_MESSAGE && typeof data.height === 'number')
        setHeight(Math.max(minimum, Math.ceil(data.height)))
    }
    window.addEventListener('message', receive)
    return () => window.removeEventListener('message', receive)
  }, [frame, minimum])
  return height
}
