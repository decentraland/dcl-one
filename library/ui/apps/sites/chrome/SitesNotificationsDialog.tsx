import { useEffect, useId, useRef, type ComponentType } from 'react'
import {
  NotificationsFriend,
  NotificationsDefault,
  NotificationsCommunity,
  NotificationsGift,
  NotificationsReward,
  NotificationsItemAirdropped,
  NotificationsEventStarted
} from '../../../foundations/icons/notifications'
import type { NavBarLinkProps } from '../../../components/NavBar/NavBar'
import styles from './SitesNotificationsDialog.module.css'

import emptyBell from '../assets/notifications-empty.webp'
import defaultThumbnail from '../assets/dcl-logo.svg'

const badges: Record<string, typeof NotificationsDefault> = {
  friends: NotificationsFriend,
  community: NotificationsCommunity,
  gift: NotificationsGift,
  badge: NotificationsReward,
  marketplace: NotificationsItemAirdropped,
  event: NotificationsEventStarted
}
type Row = {
  category?: string
  id: string
  title: string
  description: string
  href?: string | undefined
  image?: string | undefined
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
  const openerRef = useRef<HTMLElement | null>(null)
  const titleId = useId()
  const closeRef = useRef(onClose)
  useEffect(() => {
    closeRef.current = onClose
  }, [onClose])
  const isPopover = presentation === 'popover'
  useEffect(() => {
    const dialog = ref.current
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null
    openerRef.current = opener
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
        <button
          type="button"
          className={styles.close}
          aria-label="Close"
          onClick={() => {
            if (isPopover && openerRef.current?.isConnected) openerRef.current.focus()
            onClose()
          }}
        >
          <span aria-hidden="true">×</span>
        </button>
      </div>
      {state === 'loading' ? <p role="status">Loading notifications…</p> : null}
      {state === 'error' ? (
        <p role="alert">
          {rows.length > 0
            ? 'Couldn’t refresh notifications. Showing previous notifications.'
            : 'Couldn’t load notifications.'}
        </p>
      ) : null}
      {state === 'ready' && rows.length === 0 ? (
        <div className={styles.empty}>
          <img src={emptyBell} width={120} height={120} alt="" />
          <h3>No Notifications Yet</h3>
          <p>
            Say hi 👋, join the{' '}
            <Link href="/social/communities/e99471aa-31c4-4952-abf6-99905445f43b" onClick={onClose}>
              Decentraland Foundation Community
            </Link>
            , or drop into an event—this space will come to life.
          </p>
        </div>
      ) : null}
      {state !== 'loading' && rows.length > 0 ? (
        <ul className={styles.list}>
          {rows.map((row) => {
            const Badge = badges[row.category ?? ''] ?? NotificationsDefault
            const body = (
              <>
                <span className={styles.media} aria-hidden="true">
                  <img
                    className={styles.thumbnail}
                    src={row.image || defaultThumbnail}
                    alt=""
                    onError={(event) => {
                      if (event.currentTarget.getAttribute('src') !== defaultThumbnail)
                        event.currentTarget.src = defaultThumbnail
                    }}
                  />
                  <Badge size={22} />
                </span>
                <span className={styles.copy}>
                  <strong>{row.title}</strong>
                  {row.description ? <span>{row.description}</span> : null}
                  <small>{row.time}</small>
                </span>
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
