import { useId } from 'react'
import { Button } from '../../../components/Button/Button'
import { Skeleton } from '../../../components/Skeleton/Skeleton'
import { GeneralDownArrow, GeneralDownload, GeneralUpload } from '../../../foundations/icons'
import { linkHref, secureRel } from '../../../link.helpers'
import type { WalletNetworkDisplay } from './WalletsPanel'
import styles from './WalletTransactions.module.css'

export type WalletTransferRow = {
  id: string
  hash: string
  href: string | null
  direction: 'sent' | 'received'
  counterparty: string
  counterpartyLabel: string
  counterpartyHref: string | null
  amount: string
  at: number
}

type TransactionsProps = {
  display: WalletNetworkDisplay
  open: boolean
  onToggle: () => void
  rows?: readonly WalletTransferRow[]
  error?: string
  fetching: boolean
  onRetry: () => void
  more?: { pending: boolean; failed: boolean; onLoad: () => void }
}

const WHEN = new Intl.DateTimeFormat(undefined, { dateStyle: 'short', timeStyle: 'medium' })

function TransferRow({
  transfer,
  display: { Mana, accent }
}: {
  transfer: WalletTransferRow
  display: WalletNetworkDisplay
}) {
  const sent = transfer.direction === 'sent'
  const Direction = sent ? GeneralUpload : GeneralDownload
  const href = linkHref(transfer.href)
  const counterpartyHref = linkHref(transfer.counterpartyHref)
  const counterpartyPrefix = sent ? 'To' : 'From'
  const externalRel = secureRel('_blank', undefined)

  return (
    <li className={styles.row}>
      <div className={styles.kind}>
        <span className={styles.directionIcon}>
          <Direction size={16} aria-hidden="true" />
        </span>
        <div className={styles.party}>
          {sent ? 'Sent' : 'Received'}
          {counterpartyHref ? (
            <a
              className={styles.partyAddress}
              href={counterpartyHref}
              target="_blank"
              rel={externalRel}
              aria-label={`${counterpartyPrefix} ${transfer.counterparty}`}
            >
              {counterpartyPrefix} {transfer.counterpartyLabel}
            </a>
          ) : (
            <span className={styles.partyAddress}>
              {counterpartyPrefix} {transfer.counterpartyLabel}
            </span>
          )}
        </div>
      </div>
      <time className={styles.time} dateTime={new Date(transfer.at).toISOString()}>
        {WHEN.format(transfer.at)}
      </time>
      {href ? (
        <a
          className={styles.hash}
          href={href}
          target="_blank"
          rel={externalRel}
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
      <span className={styles.amount}>
        <Mana size={16} className={styles.mana} data-accent={accent} aria-hidden="true" />
        <span>
          {transfer.amount}
          <span className={styles.unit}> MANA</span>
        </span>
      </span>
    </li>
  )
}

export function WalletTransactions({
  display,
  open,
  onToggle,
  rows,
  error,
  fetching,
  onRetry,
  more
}: TransactionsProps) {
  const panelId = useId()
  const name = `Transactions, ${display.label}`
  const loading = open && rows === undefined && error === undefined

  return (
    <div className={styles.root}>
      <button
        className={styles.toggle}
        type="button"
        aria-label={name}
        aria-expanded={open}
        aria-controls={panelId}
        onClick={onToggle}
      >
        Transactions
        <GeneralDownArrow className={styles.chevron} size={16} aria-hidden="true" />
      </button>
      <div id={panelId} className={styles.panel} hidden={!open}>
        <div className={error ? styles.unavailable : styles.srOnly}>
          <p className={styles.note} role="status">
            {error ?? (loading ? 'Loading transactions…' : null)}
          </p>
          {error ? (
            <Button
              variant="secondary"
              size="sm"
              loading={fetching}
              aria-label={`Try again, ${display.label} transactions`}
              onClick={onRetry}
            >
              Try again
            </Button>
          ) : null}
        </div>
        {!open ? null : rows ? (
          rows.length === 0 ? (
            <p className={styles.note}>No transactions yet.</p>
          ) : (
            <>
              <p className={styles.listTitle}>Latest transactions</p>
              <ul className={styles.list} aria-label={name}>
                {rows.map((transfer) => (
                  <TransferRow key={transfer.id} transfer={transfer} display={display} />
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
        ) : loading ? (
          <div className={styles.loading} aria-hidden="true">
            {[0, 1, 2].map((row) => (
              <Skeleton key={row} tone="neutral" className={styles.skeleton} />
            ))}
          </div>
        ) : null}
      </div>
    </div>
  )
}
