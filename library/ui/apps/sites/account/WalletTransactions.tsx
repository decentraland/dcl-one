import { useId } from 'react'
import { Button } from '../../../components/Button/Button'
import { Skeleton } from '../../../components/Skeleton/Skeleton'
import {
  GeneralDownArrow,
  GeneralDownload,
  GeneralMana,
  GeneralPolygonMana,
  GeneralUpload
} from '../../../foundations/icons'
import { linkHref, secureRel } from '../../../link.helpers'
import { walletNetworkLabel, type WalletNetwork } from './WalletsPanel'
import styles from './WalletTransactions.module.css'

export type WalletTransferRow = {
  id: string
  hash: string
  href: string | null
  direction: 'sent' | 'received'
  counterparty: string
  amount: string
  at: number
}

type TransactionsProps = {
  network: WalletNetwork
  open: boolean
  onToggle: () => void
  rows?: readonly WalletTransferRow[]
  error?: string
  onRetry: () => void
  more?: { pending: boolean; failed: boolean; onLoad: () => void }
}

const WHEN = new Intl.DateTimeFormat(undefined, { dateStyle: 'short', timeStyle: 'medium' })

function TransferRow({
  transfer,
  network
}: {
  transfer: WalletTransferRow
  network: WalletNetwork
}) {
  const sent = transfer.direction === 'sent'
  const Direction = sent ? GeneralUpload : GeneralDownload
  const Mana = network === 'ethereum' ? GeneralMana : GeneralPolygonMana
  const href = linkHref(transfer.href)

  return (
    <li className={styles.row}>
      <span className={styles.kind} title={`${sent ? 'To' : 'From'} ${transfer.counterparty}`}>
        <span className={styles.directionIcon}>
          <Direction size={16} aria-hidden="true" />
        </span>
        {sent ? 'Sent' : 'Received'}
      </span>
      <time className={styles.time} dateTime={new Date(transfer.at).toISOString()}>
        {WHEN.format(transfer.at)}
      </time>
      {href ? (
        <a
          className={styles.hash}
          href={href}
          target="_blank"
          rel={secureRel('_blank', undefined)}
          title={transfer.hash}
        >
          {transfer.hash}
        </a>
      ) : (
        <span className={styles.hash} title={transfer.hash}>
          {transfer.hash}
        </span>
      )}
      <span className={styles.confirmed}>Confirmed</span>
      <span className={styles.amount} data-network={network}>
        <Mana size={16} className={styles.mana} aria-hidden="true" />
        <span>
          {transfer.amount}
          <span className={styles.unit}> MANA</span>
        </span>
      </span>
    </li>
  )
}

export function WalletTransactions({
  network,
  open,
  onToggle,
  rows,
  error,
  onRetry,
  more
}: TransactionsProps) {
  const panelId = useId()
  const name = `Transactions, ${walletNetworkLabel(network)}`

  return (
    <div className={styles.root}>
      <Button
        className={styles.toggle}
        variant="text"
        size="sm"
        contentLayout="fill"
        aria-label={name}
        aria-expanded={open}
        aria-controls={panelId}
        trailingIcon={<GeneralDownArrow className={styles.chevron} size={16} aria-hidden="true" />}
        onClick={onToggle}
      >
        Transactions
      </Button>
      <div id={panelId} className={styles.panel} hidden={!open}>
        {!open ? null : rows ? (
          rows.length === 0 ? (
            <p className={styles.note}>No transactions yet.</p>
          ) : (
            <>
              <p className={styles.listTitle}>Latest transactions</p>
              <ul className={styles.list} aria-label={name}>
                {rows.map((transfer) => (
                  <TransferRow key={transfer.id} transfer={transfer} network={network} />
                ))}
              </ul>
              {more?.failed ? (
                <p className={styles.error} role="alert">
                  We couldn&apos;t load more transactions. Please try again.
                </p>
              ) : null}
              {more ? (
                <div>
                  <Button
                    variant="secondary"
                    size="sm"
                    loading={more.pending}
                    onClick={more.onLoad}
                  >
                    Load more
                  </Button>
                </div>
              ) : null}
            </>
          )
        ) : error ? (
          <div className={styles.unavailable}>
            <p className={styles.note} role="status">
              {error}
            </p>
            <Button variant="secondary" size="sm" onClick={onRetry}>
              Try again
            </Button>
          </div>
        ) : (
          <div className={styles.loading} aria-busy="true">
            {[0, 1, 2].map((row) => (
              <Skeleton key={row} tone="neutral" className={styles.skeleton} />
            ))}
          </div>
        )}
      </div>
    </div>
  )
}
