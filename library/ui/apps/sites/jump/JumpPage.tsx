import type { ReactNode } from 'react'
import { AnimatedBackground } from '../shared/AnimatedBackground/AnimatedBackground'
import styles from './JumpPage.module.css'

/** The /jump backdrop: one centred card or notice over the animated brand background. */
function JumpPage({ children }: { children: ReactNode }) {
  return (
    <main className={styles.root}>
      <div className={styles.backdrop}>
        <AnimatedBackground />
      </div>
      <div className={styles.content}>{children}</div>
    </main>
  )
}

export { JumpPage }
