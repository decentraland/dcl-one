import { WalletsPanel } from "@dcl/one-ui/apps/sites/account/WalletsPanel";

import type { AuthIdentity } from "@/data/lib/auth/types";
import { WALLET_NETWORKS, type WalletNetwork } from "@/data/lib/account/wallet";

import { ACCOUNT_COPY } from "./copy";
import { WalletBalanceCard } from "./WalletBalanceCard";

const NETWORKS = Object.keys(WALLET_NETWORKS) as WalletNetwork[];

export function WalletsSettings({ identity }: { identity: AuthIdentity }) {
  return (
    <WalletsPanel label={ACCOUNT_COPY.sections.wallets}>
      {NETWORKS.map((network) => (
        <WalletBalanceCard key={network} identity={identity} network={network} />
      ))}
    </WalletsPanel>
  );
}
