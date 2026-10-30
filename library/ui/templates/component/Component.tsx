import type { ReactNode } from 'react'
import styles from './Component.module.css'

export type ComponentProps = {
  children: ReactNode
}

export function Component({ children }: ComponentProps) {
  return <div className={styles.root}>{children}</div>
}
