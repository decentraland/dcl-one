import { useInfiniteQuery, useQuery } from "@tanstack/react-query";

import type { AuthIdentity } from "@/data/lib/auth/types";
import { manaBalanceQuery, manaTransfersQuery } from "@/data/lib/account/queries";
import type { WalletNetwork } from "@/data/lib/account/wallet";

/** The account's MANA on `network`, in wei. A failed read has no amount: the card says so, never zero. */
export function useManaBalance(identity: AuthIdentity, network: WalletNetwork) {
  return useQuery(manaBalanceQuery(identity, network));
}

/** The account's MANA transfers on `network`, a page at a time; nothing is read until `enabled`. */
export function useManaTransfers(account: string, network: WalletNetwork, enabled: boolean) {
  return useInfiniteQuery({ ...manaTransfersQuery(account, network), enabled });
}
