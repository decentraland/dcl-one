import { createRoot } from 'react-dom/client'
import { JumpInvalid } from '../../apps/sites/jump/JumpInvalid'
import '../../foundations/foundations.css'
import styles from './jump-invalid-mobile.module.css'
createRoot(document.getElementById('root')!).render(
  <main className={styles.page}>
    <JumpInvalid kind="place" jumpInHref="#play" eventsHref="#events" homeHref="#home" />
    <footer className={styles.footer}>Footer</footer>
  </main>
)
