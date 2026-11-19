import { useId, type ReactNode } from "react";

import { Button, Skeleton } from "@dcl/one-ui";
import {
  GeneralEthereum,
  GeneralMana,
  GeneralPolygonMana,
  GeneralPolygonMatic,
} from "@dcl/one-ui/foundations/icons";

import type { AuthIdentity } from "@/data/lib/auth/types";
import { formatManaAmount, type WalletNetwork } from "@/data/lib/account/wallet";

import { WALLETS_COPY } from "./copy";
import { useManaBalance } from "./useWallet";
import styles from "./WalletBalanceCard.module.css";
import { WalletTransactions } from "./WalletTransactions";

const COPY = WALLETS_COPY;

const ICONS = {
  ethereum: { Network: GeneralEthereum, Mana: GeneralMana },
  polygon: { Network: GeneralPolygonMatic, Mana: GeneralPolygonMana },
} as const;

type WalletBalanceCardProps = {
  identity: AuthIdentity;
  network: WalletNetwork;
  /**
   * The card's money actions (buy, swap, send, receive), shown beside the balance. The view-only
   * section passes none, so the card renders no control that moves MANA or asks for a signature.
   */
  actions?: ReactNode;
};

/**
 * One network's MANA: the balance and, below it, the transfers. A balance that cannot be read is shown
 * as not available with a way to retry, never as zero; an amount already shown stays while a re-read fails.
 */
export function WalletBalanceCard({ identity, network, actions }: WalletBalanceCardProps) {
  const titleId = useId();
  const balance = useManaBalance(identity, network);
  const { Network, Mana } = ICONS[network];

  return (
    <section className={styles.root} aria-labelledby={titleId}>
      <div className={styles.top}>
        <div className={styles.balance}>
          <h2 id={titleId} className={styles.network}>
            <Network size={20} aria-hidden="true" />
            {COPY.networks[network]}
          </h2>
          {balance.data !== undefined ? (
            <p className={styles.amount}>
              <Mana size={24} aria-hidden="true" />
              <span>{formatManaAmount(balance.data)}</span>
              <span className={styles.unit}>{COPY.unit}</span>
            </p>
          ) : balance.isPending ? (
            <div aria-busy="true">
              <Skeleton tone="neutral" className={styles.skeleton} />
            </div>
          ) : null}
          {balance.isError ? (
            <div className={styles.unavailable}>
              <p className={styles.note} role="status">
                {COPY.balanceUnavailable}
              </p>
              <Button variant="secondary" size="sm" onClick={() => void balance.refetch()}>
                {COPY.retry}
              </Button>
            </div>
          ) : null}
        </div>
        {actions ? <div className={styles.actions}>{actions}</div> : null}
      </div>
      <WalletTransactions account={identity.signer} network={network} />
    </section>
  );
}
