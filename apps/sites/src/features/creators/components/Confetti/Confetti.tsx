import { Suspense, lazy, useState } from 'react'
import styles from './Confetti.module.css'

// The shop's purchase burst, played once. Loaded on demand: it only matters on the one dialog that
// celebrates a purchase.
const LottieBurst = lazy(() => import('./LottieBurst'))

export function Confetti() {
  // Decided once at mount, before anything is fetched: a burst of motion is what prefers-reduced-motion
  // exists for, and CSS cannot stop a JS-driven animation.
  const [play] = useState(() => !window.matchMedia?.('(prefers-reduced-motion: reduce)').matches)
  if (!play) return null

  return (
    <div className={styles.layer} aria-hidden data-testid="confetti">
      <Suspense fallback={null}>
        <LottieBurst />
      </Suspense>
    </div>
  )
}
