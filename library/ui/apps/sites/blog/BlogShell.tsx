import type { ReactNode } from 'react'
import styles from './BlogShell.module.css'

/** The blog's main landmark: the page background with the purple glow along its top. */
export function BlogShell({ children }: { children: ReactNode }) {
  return <main className={styles.shell}>{children}</main>
}
