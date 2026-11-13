import type { AnchorHTMLAttributes } from 'react'
import styles from './Tabs.module.css'

export type TabsProps = Omit<
  AnchorHTMLAttributes<HTMLAnchorElement>,
  'children' | 'aria-current' | 'dangerouslySetInnerHTML'
> & {
  href: string
  label: string
  selected?: boolean
}

/** One route-navigation item. Panel tabs need their own tablist/tabpanel contract. */
export function Tabs({ label, selected = false, className, ...anchor }: TabsProps) {
  return (
    <a
      {...anchor}
      dangerouslySetInnerHTML={undefined}
      className={[styles.root, className].filter(Boolean).join(' ')}
      aria-current={selected ? 'page' : undefined}
      data-label={label}
    >
      <span className={styles.label}>{label}</span>
    </a>
  )
}
