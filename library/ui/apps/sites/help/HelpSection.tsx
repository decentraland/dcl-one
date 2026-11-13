import type { ReactNode } from 'react'
import styles from './HelpSection.module.css'

type HelpSectionProps = {
  title: string
  description?: ReactNode
  /** The questions, in a list that fills the section's height and scrolls on its own. */
  children: ReactNode
  /** A line under the list, such as a support address. */
  footer?: ReactNode
}

/** One help center section: its heading, an optional intro and its scrolling list of questions. */
function HelpSection({ title, description, children, footer }: HelpSectionProps) {
  return (
    <section className={styles.root}>
      <div className={styles.texts}>
        <h2 className={styles.title}>{title}</h2>
        {description ? <p className={styles.description}>{description}</p> : null}
      </div>
      <div className={styles.scroller}>{children}</div>
      {footer ? <p className={[styles.description, styles.footer].join(' ')}>{footer}</p> : null}
    </section>
  )
}

export { HelpSection }
export type { HelpSectionProps }
