import { useId, type ReactNode } from 'react'
import { Button } from '../../../components/Button/Button'
import { Skeleton } from '../../../components/Skeleton/Skeleton'
import {
  GeneralEthereum,
  GeneralMana,
  GeneralPolygonMana,
  GeneralPolygonMatic
} from '../../../foundations/icons'
import styles from './WalletsPanel.module.css'

const NETWORKS = {
  ethereum: { label: 'Ethereum MANA', Icon: GeneralEthereum, Mana: GeneralMana },
  polygon: { label: 'Polygon MANA', Icon: GeneralPolygonMatic, Mana: GeneralPolygonMana }
} as const

export type WalletNetwork = keyof typeof NETWORKS

export function WalletsPanel({ label, children }: { label: string; children: ReactNode }) {
  return (
    <section className={styles.panel} aria-label={label}>
      {children}
    </section>
  )
}

type WalletBalanceProps = {
  network: WalletNetwork
  amount?: string
  pending: boolean
  unavailable: boolean
  onRetry: () => void
  children: ReactNode
}

export function WalletBalance({
  network,
  amount,
  pending,
  unavailable,
  onRetry,
  children
}: WalletBalanceProps) {
  const titleId = useId()
  const { label, Icon, Mana } = NETWORKS[network]

  return (
    <section className={styles.card} aria-labelledby={titleId} data-network={network}>
      <div className={styles.balance}>
        <h2 id={titleId} className={styles.network}>
          <span className={styles.networkIcon}>
            <Icon size={14} aria-hidden="true" />
          </span>
          {label}
        </h2>
        {amount !== undefined ? (
          <p className={styles.amount}>
            <Mana size={32} className={styles.mana} aria-hidden="true" />
            <span>{amount}</span>
          </p>
        ) : pending ? (
          <div aria-busy="true">
            <Skeleton tone="neutral" className={styles.skeleton} />
          </div>
        ) : null}
        {unavailable ? (
          <div className={styles.unavailable}>
            <p className={styles.note} role="status">
              Balance not available. This node cannot read this network right now.
            </p>
            <Button variant="secondary" size="sm" onClick={onRetry}>
              Try again
            </Button>
          </div>
        ) : null}
      </div>
      {children}
    </section>
  )
}

export function walletNetworkLabel(network: WalletNetwork) {
  return NETWORKS[network].label
}
