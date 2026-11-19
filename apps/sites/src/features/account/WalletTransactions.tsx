import { useState } from "react";
import { WalletTransactions as WalletTransactionsView } from "@dcl/one-ui/apps/sites/account/WalletTransactions";
import { explorerTxUrl } from "@ui/shop/checkout/chain/config";

import { formatManaAmount, isHistoryUnavailable, WALLET_NETWORKS, type WalletNetwork } from "@/data/lib/account/wallet";

import { useManaTransfers } from "./useWallet";

export function WalletTransactions({ account, network }: { account: string; network: WalletNetwork }) {
  const [open, setOpen] = useState(false);
  const transfers = useManaTransfers(account, network, open);
  const rows = transfers.data?.pages.flatMap((page) =>
    page.transfers.map((transfer) => ({
      ...transfer,
      amount: formatManaAmount(transfer.amount),
      href: explorerTxUrl(WALLET_NETWORKS[network].chainId, transfer.hash),
    })),
  );
  const error = transfers.isError
    ? isHistoryUnavailable(transfers.error)
      ? "Transaction history is not available on this node."
      : "We couldn't load your transactions. Please try again later."
    : undefined;

  return (
    <WalletTransactionsView
      network={network}
      open={open}
      onToggle={() => setOpen((shown) => !shown)}
      rows={rows}
      error={error}
      onRetry={() => void transfers.refetch()}
      more={
        transfers.hasNextPage
          ? {
              pending: transfers.isFetchingNextPage,
              failed: transfers.isFetchNextPageError,
              onLoad: () => void transfers.fetchNextPage(),
            }
          : undefined
      }
    />
  );
}
