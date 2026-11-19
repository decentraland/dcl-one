import { WalletBalance } from "@dcl/one-ui/apps/sites/account/WalletsPanel";

import type { AuthIdentity } from "@/data/lib/auth/types";
import { formatManaAmount, type WalletNetwork } from "@/data/lib/account/wallet";

import { useManaBalance } from "./useWallet";
import { WalletTransactions } from "./WalletTransactions";

export function WalletBalanceCard({ identity, network }: { identity: AuthIdentity; network: WalletNetwork }) {
  const balance = useManaBalance(identity, network);

  return (
    <WalletBalance
      network={network}
      amount={balance.data === undefined ? undefined : formatManaAmount(balance.data)}
      pending={balance.isPending}
      unavailable={balance.isError}
      onRetry={() => void balance.refetch()}
    >
      <WalletTransactions account={identity.signer} network={network} />
    </WalletBalance>
  );
}
