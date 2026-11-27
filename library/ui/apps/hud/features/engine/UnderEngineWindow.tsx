// Everything the HUD draws under its popups. While a popup shows something the engine draws under the page
// (lib/engineWindow: the passport's avatar), that spot is cut out of all of it, so the engine's picture is
// not behind the chat, a full-screen page or the lobby. The wrapper has no box of its own otherwise
// (`display: contents`), so it changes nothing about where its children are laid out.

import { useLayoutEffect, useRef, useState, type ReactNode } from 'react'
import { useEngineWindow, windowClip } from '../../lib/engineWindow'

export function UnderEngineWindow({ children }: { children: ReactNode }): React.JSX.Element {
  const engineWindow = useEngineWindow()
  const ref = useRef<HTMLDivElement>(null)
  const [clipPath, setClipPath] = useState<string>()
  useLayoutEffect(() => {
    const el = ref.current
    if (engineWindow == null || el == null) {
      setClipPath(undefined)
      return
    }
    // The clip is in the wrapper's own pixels, from wherever its (empty) box starts.
    el.style.display = 'block'
    const box = el.getBoundingClientRect()
    setClipPath(windowClip(engineWindow, { x: box.left, y: box.top }))
  }, [engineWindow])
  return (
    <div ref={ref} style={clipPath != null ? { display: 'block', clipPath } : { display: 'contents' }}>
      {children}
    </div>
  )
}
