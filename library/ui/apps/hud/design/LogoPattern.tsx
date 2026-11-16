// LogoPattern — the drifting Decentraland icon pattern over the purple vignette, behind the
// scene loading screen and the sign-in screens. It fills its
// positioned parent and paints the parent's vignette through itself (the parent sets
// `background: var(--loading-vignette)`), so the first frame is CSS only.
import styles from './LogoPattern.module.css'

export function LogoPattern(): React.JSX.Element {
  return (
    <div className={styles.pattern} aria-hidden="true">
      <div className={styles.patternX}>
        <div className={styles.patternY} />
      </div>
    </div>
  )
}
