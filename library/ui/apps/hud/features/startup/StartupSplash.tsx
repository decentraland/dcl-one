// The splash: the animated logo on the purple vignette, until the first screen the
// player can use. The hosts' page paints the same thing before the HUD loads (react-web app.html,
// Sites play_.app[.]html.tsx), from the same file, so the hand-off doesn't jump.
import { publicUrl } from '@hud-host'
import styles from './StartupSplash.module.css'

export function StartupSplash(): React.JSX.Element {
  return (
    <div className={styles.root} role="status" aria-label="Loading">
      <img className={styles.logo} src={publicUrl('assets/splash-logo.webp')} alt="" draggable={false} />
    </div>
  )
}
