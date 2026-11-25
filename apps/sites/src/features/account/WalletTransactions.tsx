import { useState } from "react";
import { WalletTransactions as WalletTransactionsView, type WalletNetworkDisplay } from "@dcl/one-ui";
import { explorerAddressUrl, explorerTxUrl } from "@ui/shop/checkout/chain/config";

import { formatManaAmount, isHistoryUnavailable, WALLET_NETWORKS, type WalletNetwork } from "@/data/lib/account/wallet";

import { shortAddress } from "@/data/lib/catalyst/format/address";

import { useManaTransfers } from "./useWallet";

type WalletTransactionsProps = {
  account: string;
  network: WalletNetwork;
  display: WalletNetworkDisplay;
};

export function WalletTransactions({ account, network, display }: WalletTransactionsProps) {
  const [open, setOpen] = useState(false);
  const transfers = useManaTransfers(account, network, open);
  const { chainId } = WALLET_NETWORKS[network];
  const rows = transfers.data?.pages.flatMap((page) =>
    page.transfers.map((transfer) => ({
      ...transfer,
      amount: formatManaAmount(transfer.amount),
      href: explorerTxUrl(chainId, transfer.hash),
      counterpartyLabel: shortAddress(transfer.counterparty),
      counterpartyHref: explorerAddressUrl(chainId, transfer.counterparty),
    })),
  );
  const error = transfers.isError && !transfers.isFetchNextPageError
    ? rows
      ? "Couldn't refresh transactions. The history shown may be out of date."
      : isHistoryUnavailable(transfers.error)
        ? "Transaction history is not available on this node."
        : "We couldn't load your transactions. Please try again later."
    : undefined;

  return (
    <WalletTransactionsView
      display={display}
      open={open}
      onToggle={() => setOpen((shown) => !shown)}
      rows={rows}
      error={error}
      fetching={transfers.isFetching}
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
