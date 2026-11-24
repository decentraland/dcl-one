import { WalletBalance, WalletsPanel, type WalletNetworkDisplay } from "@dcl/one-ui/apps/sites/account/WalletsPanel";
import { GeneralEthereum, GeneralMana, GeneralPolygonMana, GeneralPolygonMatic } from "@dcl/one-ui";

import type { AuthIdentity } from "@/data/lib/auth/types";
import { formatManaAmount, type WalletNetwork } from "@/data/lib/account/wallet";

import { ACCOUNT_COPY } from "./copy";
import { useManaBalance } from "./useWallet";
import { WalletTransactions } from "./WalletTransactions";

const NETWORK_DISPLAY = {
  ethereum: { label: "Ethereum MANA", Icon: GeneralEthereum, Mana: GeneralMana, tone: "blue", accent: true },
  polygon: { label: "Polygon MANA", Icon: GeneralPolygonMatic, Mana: GeneralPolygonMana, tone: "purple", accent: false },
} as const satisfies Record<WalletNetwork, WalletNetworkDisplay>;

const NETWORKS = Object.keys(NETWORK_DISPLAY) as WalletNetwork[];

function WalletBalanceCard({ identity, network }: { identity: AuthIdentity; network: WalletNetwork }) {
  const balance = useManaBalance(identity, network);
  const display = NETWORK_DISPLAY[network];

  return (
    <WalletBalance
      display={display}
      amount={balance.data === undefined ? undefined : formatManaAmount(balance.data)}
      pending={balance.isPending}
      unavailable={balance.isError}
      fetching={balance.isFetching}
      onRetry={() => void balance.refetch()}
    >
      <WalletTransactions account={identity.signer} network={network} display={display} />
    </WalletBalance>
  );
}

export function WalletsSettings({ identity }: { identity: AuthIdentity }) {
  return (
    <WalletsPanel label={ACCOUNT_COPY.sections.wallets}>
      {NETWORKS.map((network) => (
        <WalletBalanceCard key={network} identity={identity} network={network} />
      ))}
    </WalletsPanel>
  );
}
