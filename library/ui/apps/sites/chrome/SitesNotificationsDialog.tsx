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
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null
    dialog?.showModal()
    return () => {
      if (dialog?.open) dialog.close()
      if (opener?.isConnected) opener.focus()
    }
  }, [])
  return (
    <dialog
      ref={ref}
      className={styles.dialog}
      aria-labelledby={titleId}
      onKeyDown={(event) => {
        if (event.key !== 'Escape' || event.defaultPrevented) return
        event.preventDefault()
        event.stopPropagation()
        onClose()
      }}
      onCancel={(event) => {
        event.preventDefault()
        onClose()
      }}
      onClick={(event) => {
        if (event.target !== event.currentTarget) return
        const box = event.currentTarget.getBoundingClientRect()
        if (
          event.clientX < box.left ||
          event.clientX > box.right ||
          event.clientY < box.top ||
          event.clientY > box.bottom
        )
          onClose()
      }}
    >
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
