import type { ReactNode } from 'react'
import topBackground from '../assets/events/top_background.webp'
import styles from './EventsPage.module.css'

/**
 * The /events page frame: the purple backdrop with source's theatre photo behind the top, and a
 * polite status region for notices such as a failed reminder. The region stays in the page so
 * announcements are heard, and takes no space while it is empty.
 */
function EventsPage({
  title,
  notice,
  dialog,
  children
}: {
  title: string
  notice?: string
  /** An open detail dialog: outside the content's stacking context, so it covers the site header. */
  dialog?: ReactNode
  children: ReactNode
}) {
  return (
    <main className={styles.root}>
      <img
        className={styles.backdrop}
        src={topBackground}
        alt=""
        aria-hidden="true"
        decoding="async"
      />
      <h1 className={styles.visuallyHidden}>{title}</h1>
      <p role="status" className={notice ? styles.notice : styles.visuallyHidden}>
        {notice ?? ''}
      </p>
      <div className={styles.content}>{children}</div>
      {dialog}
    </main>
  )
}

export { EventsPage }
