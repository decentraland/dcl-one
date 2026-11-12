import { useEffect, useId, useRef, type ComponentType } from 'react'
import { Button } from '../../../components/Button/Button'
import type { NavBarLinkProps } from '../../../components/NavBar/NavBar'
import styles from './SitesNotificationsDialog.module.css'

type Row = {
  id: string
  title: string
  description: string
  href?: string | undefined
  time: string
  unread: boolean
}
export default function SitesNotificationsDialog({
  onClose,
  state,
  rows,
  Link
}: {
  onClose: () => void
  state: 'loading' | 'error' | 'ready'
  rows: Row[]
  Link: ComponentType<NavBarLinkProps>
}) {
  const ref = useRef<HTMLDialogElement>(null)
  const titleId = useId()
  useEffect(() => {
    const dialog = ref.current
    dialog?.showModal()
    return () => dialog?.close()
  }, [])
  return (
    <dialog ref={ref} className={styles.dialog} aria-labelledby={titleId} onClose={onClose}>
      <div className={styles.heading}>
        <h2 id={titleId}>Notifications</h2>
        <Button onClick={onClose}>Close</Button>
      </div>
      {state === 'loading' ? <p role="status">Loading notifications…</p> : null}
      {state === 'error' ? <p role="alert">Couldn’t load notifications.</p> : null}
      {state === 'ready' && rows.length === 0 ? <p>No notifications yet</p> : null}
      {state === 'ready' && rows.length > 0 ? (
        <ul className={styles.list}>
          {rows.map((row) => {
            const body = (
              <>
                <strong>{row.title}</strong>
                {row.description ? <span>{row.description}</span> : null}
                <small>{row.time}</small>
              </>
            )
            return (
              <li key={row.id} data-unread={row.unread || undefined}>
                {row.href ? (
                  <Link className={styles.row} href={row.href} onClick={onClose}>
                    {body}
                  </Link>
                ) : (
                  <div className={styles.row}>{body}</div>
                )}
              </li>
            )
          })}
        </ul>
      ) : null}
    </dialog>
  )
}
