import { useRef, useState } from 'react'
import type { ButtonHTMLAttributes, ComponentType, Ref } from 'react'
import {
  MenusAdd,
  MenusBlock,
  MenusFriends,
  MenusPending,
  MenusUnfriend
} from '../../foundations/icons/menus'
import type { IconProps } from '../../foundations/icons/types'
import styles from './FriendCTA.module.css'

export type FriendCTAStatus = 'add' | 'sent' | 'friend' | 'reject' | 'blocked'

export type FriendCTALabels = {
  add: string
  sent: string
  cancel: string
  friend: string
  unfriend: string
  reject: string
  blocked: string
  unblock: string
}

export type FriendCTAProps = Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'children' | 'type'> & {
  status: FriendCTAStatus
  /** The write the press started is in progress: the spinner shows, and a press does nothing. */
  loading?: boolean
  /** `desktop` is 40px high, `mobile` 48px. */
  viewport?: 'desktop' | 'mobile'
  labels?: Partial<FriendCTALabels>
  ref?: Ref<HTMLButtonElement>
}

const LABELS: FriendCTALabels = {
  add: 'Add friend',
  sent: 'Request sent',
  cancel: 'Cancel request',
  friend: 'Friend',
  unfriend: 'Unfriend',
  reject: 'Reject',
  blocked: 'Blocked',
  unblock: 'Unblock'
}

type Look = {
  /** The friendship as it is; absent when the button only names its action. */
  state?: keyof FriendCTALabels
  action: keyof FriendCTALabels
  icon?: ComponentType<IconProps>
  actionIcon?: ComponentType<IconProps>
}

const LOOKS: Record<FriendCTAStatus, Look> = {
  add: { action: 'add', actionIcon: MenusAdd },
  sent: { state: 'sent', action: 'cancel', icon: MenusPending, actionIcon: MenusUnfriend },
  friend: { state: 'friend', action: 'unfriend', icon: MenusFriends, actionIcon: MenusUnfriend },
  reject: { action: 'reject' },
  blocked: { state: 'blocked', action: 'unblock', icon: MenusBlock }
}

const ICON = { size: 20, 'aria-hidden': true, focusable: false } as const

/**
 * The friendship call to action: one native button for each friendship state. `sent`, `friend` and
 * `blocked` show the state, and show what a press does (cancel, unfriend, unblock) on hover and on
 * keyboard focus. A touch or pen press shows that action first, and the next press runs it. Both
 * texts are the button's name at all times.
 */
export function FriendCTA({
  status,
  loading = false,
  viewport = 'desktop',
  labels,
  className,
  disabled,
  onClick,
  onPointerDown,
  onBlur,
  ...rest
}: FriendCTAProps) {
  const text = { ...LABELS, ...labels }
  const look = LOOKS[status]
  const coarse = useRef(false)
  const [armed, setArmed] = useState(false)
  const StateIcon = look.icon
  const ActionIcon = look.actionIcon
  return (
    <button
      {...rest}
      type="button"
      disabled={disabled}
      aria-busy={loading || undefined}
      aria-disabled={loading || rest['aria-disabled'] || undefined}
      className={[styles.root, className].filter(Boolean).join(' ')}
      data-status={status}
      data-viewport={viewport}
      data-loading={loading || undefined}
      data-armed={armed || undefined}
      onPointerDown={(event) => {
        coarse.current = event.pointerType === 'touch' || event.pointerType === 'pen'
        onPointerDown?.(event)
      }}
      onBlur={(event) => {
        setArmed(false)
        onBlur?.(event)
      }}
      onClick={(event) => {
        const fromTouch = coarse.current
        coarse.current = false
        if (loading) return event.preventDefault()
        // Without hover, the first press only shows what the next one does.
        if (look.state && fromTouch && !armed) return setArmed(true)
        setArmed(false)
        onClick?.(event)
      }}
    >
      {loading ? (
        <span className={styles.spinner} aria-hidden="true" />
      ) : (
        <>
          {StateIcon && (
            <span className={styles.stateOnly}>
              <StateIcon {...ICON} />
            </span>
          )}
          {ActionIcon && (
            <span className={look.state ? styles.actionOnly : styles.icon}>
              <ActionIcon {...ICON} />
            </span>
          )}
        </>
      )}
      <span className={styles.labels}>
        {look.state && <span className={styles.stateText}>{text[look.state]}</span>}
        {look.state && ' '}
        <span className={look.state ? styles.actionText : undefined}>{text[look.action]}</span>
      </span>
    </button>
  )
}
