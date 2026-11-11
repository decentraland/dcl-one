import type { ReactNode } from 'react'
import styles from './JumpPage.module.css'

/** The /jump backdrop: one centred card or notice over the brand background. */
function JumpPage({ children }: { children: ReactNode }) {
  return <main className={styles.root}>{children}</main>
}

export { JumpPage }
