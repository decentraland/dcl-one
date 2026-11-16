// TODO(figma-ds): goes with its last consumer: the provisional avatars and the Sites profile collectible card.
import { useCallback, useEffect, useRef, useState } from 'react'

/**
 * Whether an image failed to load. On a server-rendered page an image can fail before React
 * hydrates and attaches `onError`, so the mounted element is checked too.
 */
function useBrokenImage(src: string | undefined) {
  const [broken, setBroken] = useState(false)
  const ref = useRef<HTMLImageElement>(null)
  useEffect(() => {
    const img = ref.current
    setBroken(Boolean(img && img.complete && img.naturalWidth === 0 && src))
  }, [src])
  const onError = useCallback(() => setBroken(true), [])
  return { broken, ref, onError }
}

export { useBrokenImage }
