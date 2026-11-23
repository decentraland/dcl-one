import { useId, type ReactNode } from 'react'
import ChromeLink from './ChromeLink'
import styles from './ChromeBody.module.css'

type ChromeBodyProps = {
  className?: string
  ariaLabel?: string
  tools?: ReactNode
  children?: ReactNode
  footer?: ReactNode
}

export default function ChromeBody({ className, ariaLabel, tools, children, footer }: ChromeBodyProps) {
  const mainId = useId()
  return (
    <div
      className={className ? `${styles.root} cs ui2 ${className}` : `${styles.root} cs ui2`}
      data-label={ariaLabel}
      data-site-layout-fill=""
    >
      <ChromeLink className={styles.skip} href={`#${mainId}`}>
        Skip to content
      </ChromeLink>
      {tools ? <div className={`${styles.tools} cs__tools`}>{tools}</div> : null}
      <main className={`${styles.body} cs__body`} id={mainId} tabIndex={-1}>
        {children}
        {footer}
      </main>
    </div>
  )
}
