import { useId, useState } from "react";

import { Button, Skeleton } from "@dcl/one-ui";
import { GeneralDownArrow, GeneralDownload, GeneralUpload } from "@dcl/one-ui/foundations/icons";

import {
  formatManaAmount,
  isHistoryUnavailable,
  type ManaTransfer,
  type WalletNetwork,
} from "@/data/lib/account/wallet";
import { shortAddress } from "@/data/lib/catalyst/format/address";

import { WALLETS_COPY } from "./copy";
import { useManaTransfers } from "./useWallet";
import styles from "./WalletTransactions.module.css";

const COPY = WALLETS_COPY.transactions;

/** The reader's own locale and time zone: the list renders in the browser only, after a click. */
const WHEN = new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" });

function TransferRow({ transfer }: { transfer: ManaTransfer }) {
  const Direction = transfer.direction === "sent" ? GeneralUpload : GeneralDownload;
  return (
    <li className={styles.row}>
      <span className={styles.kind}>
        <Direction size={16} aria-hidden="true" />
        {COPY.direction[transfer.direction]}
      </span>
      <span className={styles.amount}>
        {formatManaAmount(transfer.amount)} {WALLETS_COPY.unit}
      </span>
      <span className={styles.detail}>
        {COPY.counterparty[transfer.direction]}{" "}
        <span title={transfer.counterparty}>{shortAddress(transfer.counterparty)}</span>
      </span>
      <time className={styles.detail} dateTime={new Date(transfer.at).toISOString()}>
        {WHEN.format(transfer.at)}
      </time>
      <span className={styles.hash}>{transfer.hash}</span>
    </li>
  );
}

/**
 * A network's MANA transfers, closed until the member opens them, so a visit that only reads the
 * balances asks the node for no history. A history the node cannot serve says so, with a retry; it is
 * never shown as an empty list. Rows are confirmed transfers: there is no pending or bridge state here.
 */
export function WalletTransactions({ account, network }: { account: string; network: WalletNetwork }) {
  const panelId = useId();
  const [open, setOpen] = useState(false);
  const transfers = useManaTransfers(account, network, open);
  const rows = transfers.data?.pages.flatMap((page) => page.transfers);
  const name = `${COPY.title}, ${WALLETS_COPY.networks[network]}`;

  return (
    <div className={styles.root}>
      <div>
        <Button
          variant="text"
          size="sm"
          aria-label={name}
          aria-expanded={open}
          aria-controls={panelId}
          trailingIcon={<GeneralDownArrow className={open ? styles.chevronOpen : styles.chevron} size={16} aria-hidden="true" />}
          onClick={() => setOpen((shown) => !shown)}
        >
          {COPY.title}
        </Button>
      </div>
      <div id={panelId} className={styles.panel} hidden={!open}>
        {!open ? null : rows ? (
          rows.length === 0 ? (
            <p className={styles.note}>{COPY.empty}</p>
          ) : (
            <>
              <ul className={styles.list} aria-label={name}>
                {rows.map((transfer) => (
                  <TransferRow key={transfer.id} transfer={transfer} />
                ))}
              </ul>
              {transfers.isFetchNextPageError ? (
                <p className={styles.error} role="alert">
                  {COPY.moreError}
                </p>
              ) : null}
              {transfers.hasNextPage ? (
                <div>
                  <Button
                    variant="secondary"
                    size="sm"
                    loading={transfers.isFetchingNextPage}
                    onClick={() => void transfers.fetchNextPage()}
                  >
                    {COPY.loadMore}
                  </Button>
                </div>
              ) : null}
            </>
          )
        ) : transfers.isError ? (
          <div className={styles.unavailable}>
            <p className={styles.note} role="status">
              {isHistoryUnavailable(transfers.error) ? COPY.unavailable : COPY.loadError}
            </p>
            <Button variant="secondary" size="sm" onClick={() => void transfers.refetch()}>
              {WALLETS_COPY.retry}
            </Button>
          </div>
        ) : (
          <div className={styles.loading} aria-busy="true">
            <Skeleton tone="neutral" className={styles.skeleton} />
            <Skeleton tone="neutral" className={styles.skeleton} />
            <Skeleton tone="neutral" className={styles.skeleton} />
          </div>
        )}
      </div>
    </div>
  );
}
