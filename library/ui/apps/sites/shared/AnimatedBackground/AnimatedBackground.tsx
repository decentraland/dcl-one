// Ported from the Shop's copy (apps/ui3/src/shop/discovery/components/animated-background), itself from
// decentraland/auth src/components/AnimatedBackground (Apache-2.0); the pattern and still image are
// decentraland-ui2's (see ../../assets/NOTICE).

import { useEffect, useRef } from 'react'
import { animateBackground, DEFAULT_PATTERN_TILING } from './animate'
import styles from './AnimatedBackground.module.css'

// Ported from decentraland/auth (src/components/AnimatedBackground): a WebGL purple vignette with the
// DCL logo pattern drifting across it. The static image is the fallback while WebGL boots — or the only
// thing shown when WebGL is unavailable or the user prefers reduced motion.
function AnimatedBackground({
  patternTiling = DEFAULT_PATTERN_TILING
}: {
  patternTiling?: number
}) {
  const rootRef = useRef<HTMLDivElement>(null)
  // Read by the render loop each frame, so tiling changes apply live without rebuilding the GL state.
  const tilingRef = useRef(patternTiling)
  tilingRef.current = patternTiling

  useEffect(() => {
    const root = rootRef.current
    if (!root) return
    return animateBackground(root, styles.canvas ?? '', () => tilingRef.current)
  }, [])

  return (
    <div className={styles.root} ref={rootRef} aria-hidden>
      <div className={styles.fallback} />
    </div>
  )
}

export { AnimatedBackground }
