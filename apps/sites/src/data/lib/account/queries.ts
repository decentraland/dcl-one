import { infiniteQueryOptions, queryOptions } from "@tanstack/react-query";

import type { AuthIdentity } from "../auth/types";
import { fetchCreditsStatus } from "./credits";
import { fetchSubscription } from "./notifications";
import {
  fetchManaBalance,
  fetchManaTransfers,
  ownWalletOf,
  type ManaTransfersCursor,
  type WalletNetwork,
} from "./wallet";

/** The Sites root default, declared here so a loader's dehydrated data and the page agree on freshness. */
export const ACCOUNT_STALE_MS = 30_000;

/** Every account read is the signed-in member's own, so all keys sit under `["private", account, "account", ...]`. */
export const accountKeys = {
  all: (account: string) => ["private", account.toLowerCase(), "account"] as const,
  subscription: (account: string) =>
    ["private", account.toLowerCase(), "account", "subscription"] as const,
  creditsStatus: (account: string) =>
    ["private", account.toLowerCase(), "account", "credits-status"] as const,
  manaBalance: (account: string, network: WalletNetwork) =>
    ["private", account.toLowerCase(), "account", "mana-balance", network] as const,
  manaTransfers: (account: string, network: WalletNetwork) =>
    ["private", account.toLowerCase(), "account", "mana-transfers", network] as const,
};

/** The identity signs the request and is never cached; the key names only its account. */
export const subscriptionQuery = (identity: AuthIdentity) =>
  queryOptions({
    queryKey: accountKeys.subscription(identity.signer),
    staleTime: ACCOUNT_STALE_MS,
    queryFn: ({ signal }) => fetchSubscription(identity, { signal }),
  });

export const creditsStatusQuery = (identity: AuthIdentity) =>
  queryOptions({
    queryKey: accountKeys.creditsStatus(identity.signer),
    staleTime: ACCOUNT_STALE_MS,
    queryFn: ({ signal }) => fetchCreditsStatus(identity, { signal }),
  });

/** The member's wallet is looked up at read time and never cached: only the amount enters the cache. */
export const manaBalanceQuery = (identity: AuthIdentity, network: WalletNetwork) =>
  queryOptions({
    queryKey: accountKeys.manaBalance(identity.signer, network),
    staleTime: ACCOUNT_STALE_MS,
    queryFn: ({ signal }) =>
      fetchManaBalance(identity.signer, network, { signal, ownWallet: () => ownWalletOf(identity) }),
  });

/** One entry holds every page read so far; the cursor is the page parameter, not part of the key. */
export const manaTransfersQuery = (account: string, network: WalletNetwork) =>
  infiniteQueryOptions({
    queryKey: accountKeys.manaTransfers(account, network),
    staleTime: ACCOUNT_STALE_MS,
    initialPageParam: null as ManaTransfersCursor | null,
    queryFn: ({ pageParam, signal }) => fetchManaTransfers(account, network, { cursor: pageParam, signal }),
    getNextPageParam: (last) => last.next,
  });
