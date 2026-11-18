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
  presentation = 'modal',
  state,
  rows,
  Link
}: {
  onClose: () => void
  presentation?: 'modal' | 'popover'
  state: 'loading' | 'error' | 'ready'
  rows: Row[]
  Link: ComponentType<NavBarLinkProps>
}) {
  const ref = useRef<HTMLDialogElement>(null)
  const titleId = useId()
  const closeRef = useRef(onClose)
  useEffect(() => {
    closeRef.current = onClose
  }, [onClose])
  const isPopover = presentation === 'popover'
  useEffect(() => {
    const dialog = ref.current
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null
    if (isPopover) {
      const control = dialog?.parentElement
      const onPointerDown = (event: PointerEvent) => {
        if (!control?.contains(event.target as Node)) closeRef.current()
      }
      const onEscape = (event: KeyboardEvent) => {
        if (
          event.key !== 'Escape' ||
          event.defaultPrevented ||
          !control?.contains(event.target as Node)
        )
          return
        event.preventDefault()
        closeRef.current()
        opener?.focus()
      }
      document.addEventListener('pointerdown', onPointerDown)
      control?.addEventListener('keydown', onEscape)
      return () => {
        document.removeEventListener('pointerdown', onPointerDown)
        control?.removeEventListener('keydown', onEscape)
      }
    }
    dialog?.showModal()
    return () => {
      if (dialog?.open) dialog.close()
      if (opener?.isConnected) opener.focus()
    }
  }, [isPopover])
  return (
    <dialog
      ref={ref}
      className={styles.dialog}
      open={isPopover || undefined}
      data-presentation={presentation}
      aria-labelledby={titleId}
      onKeyDown={(event) => {
        if (isPopover || event.key !== 'Escape' || event.defaultPrevented) return
        event.preventDefault()
        event.stopPropagation()
        onClose()
      }}
      onCancel={(event) => {
        event.preventDefault()
        onClose()
      }}
      onClick={(event) => {
        if (isPopover || event.target !== event.currentTarget) return
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
        <Button
          onClick={() => {
            if (isPopover) ref.current?.parentElement?.querySelector('button')?.focus()
            onClose()
          }}
        >
          Close
        </Button>
      </div>
      {state === 'loading' ? <p role="status">Loading notifications…</p> : null}
      {state === 'error' ? (
        <p role="alert">
          {rows.length > 0
            ? 'Couldn’t refresh notifications. Showing previous notifications.'
            : 'Couldn’t load notifications.'}
        </p>
      ) : null}
      {state === 'ready' && rows.length === 0 ? <p>No notifications yet</p> : null}
      {state !== 'loading' && rows.length > 0 ? (
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
