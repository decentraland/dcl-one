import { useId } from "react";

import type { AuthIdentity } from "@/data/lib/auth/types";
import type { WalletNetwork } from "@/data/lib/account/wallet";

import { CopyAddress } from "../components/CopyAddress";

import { ACCOUNT_COPY, WALLETS_COPY } from "./copy";
import { WalletBalanceCard } from "./WalletBalanceCard";
import styles from "./WalletsSettings.module.css";

const NETWORKS: readonly WalletNetwork[] = ["ethereum", "polygon"];

/**
 * `/account/wallets`: the member's wallet address, and the MANA balance and transfers of each network.
 * View only: nothing here moves MANA or asks the wallet to sign. The money actions (buy, swap, send,
 * receive) come later and go in each balance card's `actions`.
 */
export function WalletsSettings({ identity }: { identity: AuthIdentity }) {
  const addressId = useId();

  return (
    <section className={styles.root} aria-label={ACCOUNT_COPY.sections.wallets}>
      <section className={styles.card} aria-labelledby={addressId}>
        <h2 id={addressId} className={styles.title}>
          {WALLETS_COPY.address}
        </h2>
        <div className={styles.addressRow}>
          <span className={styles.address}>{identity.signer}</span>
          <CopyAddress address={identity.signer} />
        </div>
      </section>
      {NETWORKS.map((network) => (
        <WalletBalanceCard key={network} identity={identity} network={network} />
      ))}
    </section>
  );
}
