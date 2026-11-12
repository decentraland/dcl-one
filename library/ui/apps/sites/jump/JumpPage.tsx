import type { ReactNode } from 'react'
import { AnimatedBackground } from '../shared/AnimatedBackground/AnimatedBackground'
import styles from './JumpPage.module.css'

type JumpPageProps = {
  children: ReactNode
  /** The invalid-link notice sits in a narrower, centred frame than a card. */
  invalid?: boolean
}

/** The /jump backdrop: one centred card or notice over the animated brand background, which fills the viewport. */
function JumpPage({ children, invalid = false }: JumpPageProps) {
  return (
    <main className={invalid ? `${styles.root} ${styles.invalid}` : styles.root}>
      <div className={styles.backdrop}>
        <AnimatedBackground />
      </div>
      <div className={styles.content}>{children}</div>
    </main>
  )
}

export { JumpPage }
export type { JumpPageProps }
