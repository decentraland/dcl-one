import type { HTMLAttributes } from 'react'
import styles from './ConnectionStatus.module.css'

export type ConnectionStatusValue = 'online' | 'away'

export type ConnectionStatusProps = Omit<HTMLAttributes<HTMLSpanElement>, 'children' | 'role'> & {
  status: ConnectionStatusValue
  /** The text for assistive technology. Defaults to "Online" or "Away"; `""` hides the dot from it. */
  label?: string
}

const LABELS: Record<ConnectionStatusValue, string> = { online: 'Online', away: 'Away' }

/** The dot that says whether a member is connected. Its colour is never its only sign: it has a name. */
export function ConnectionStatus({ status, label = LABELS[status], className, ...rest }: ConnectionStatusProps) {
  return (
    <span
      {...rest}
      {...(label ? { role: 'img', 'aria-label': label } : { 'aria-hidden': true })}
      className={[styles.root, className].filter(Boolean).join(' ')}
      data-status={status}
    />
  )
}
