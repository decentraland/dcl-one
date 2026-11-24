// From decentraland/auth src/components/AnimatedBackground (Apache-2.0); the pattern and still image are
// decentraland-ui2's (see ../../assets/NOTICE).

import { useEffect, useRef } from 'react'
import { animateBackground, DEFAULT_PATTERN_TILING } from './animate'
import styles from './AnimatedBackground.module.css'

export type AnimatedBackgroundProps = {
  /** Repeats of the logo pattern across the background's height (higher draws it smaller); default 1.66. A change applies on the next frame. */
  patternTiling?: number
}

// A WebGL purple vignette with the DCL logo pattern drifting across it, filling its positioned parent. The static
// image is the fallback while WebGL boots, and the only thing shown when WebGL is unavailable or the user prefers
// reduced motion.
export function AnimatedBackground({
  patternTiling = DEFAULT_PATTERN_TILING
}: AnimatedBackgroundProps) {
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
