import { useId, type ComponentType, type ReactNode } from 'react'
import { Button } from '../../../components/Button/Button'
import { Skeleton } from '../../../components/Skeleton/Skeleton'
import type { IconProps } from '../../../foundations/icons'
import styles from './WalletsPanel.module.css'

export type WalletNetworkDisplay = {
  label: string
  Icon: ComponentType<IconProps>
  Mana: ComponentType<IconProps>
  tone: 'blue' | 'purple'
  accent: boolean
}

export function WalletsPanel({ label, children }: { label: string; children: ReactNode }) {
  const titleId = useId()
  return (
    <section className={styles.panel} aria-labelledby={titleId}>
      <h2 id={titleId} className={styles.srOnly}>
        {label}
      </h2>
      {children}
    </section>
  )
}

type WalletBalanceProps = {
  display: WalletNetworkDisplay
  amount?: string
  pending: boolean
  unavailable: boolean
  fetching: boolean
  onRetry: () => void
  children: ReactNode
}

export function WalletBalance({
  display: { label, Icon, Mana, tone, accent },
  amount,
  pending,
  unavailable,
  fetching,
  onRetry,
  children
}: WalletBalanceProps) {
  const titleId = useId()

  return (
    <section className={styles.card} aria-labelledby={titleId}>
      <div className={styles.balance}>
        <h3 id={titleId} className={styles.network}>
          <span className={styles.networkIcon} data-tone={tone}>
            <Icon size={14} aria-hidden="true" />
          </span>
          {label}
        </h3>
        {amount !== undefined ? (
          <p className={styles.amount}>
            <Mana size={32} className={styles.mana} data-accent={accent} aria-hidden="true" />
            <span>{amount}</span>
          </p>
        ) : pending ? (
          <div aria-hidden="true">
            <Skeleton tone="neutral" className={styles.skeleton} />
          </div>
        ) : null}
        <div className={unavailable ? styles.unavailable : styles.srOnly}>
          <p className={styles.note} role="status">
            {unavailable
              ? amount === undefined
                ? 'Balance not available. This node cannot read this network right now.'
                : "Couldn't refresh the balance. The balance shown may be out of date."
              : pending
                ? 'Loading balance…'
                : null}
          </p>
          {unavailable ? (
            <Button
              variant="secondary"
              size="sm"
              loading={fetching}
              aria-label={`Try again, ${label} balance`}
              onClick={onRetry}
            >
              Try again
            </Button>
          ) : null}
        </div>
      </div>
      {children}
    </section>
  )
}
