import { queryOptions } from "@tanstack/react-query";

import type { AuthIdentity } from "../auth/types";
import { fetchCreditsStatus } from "./credits";
import { fetchSubscription } from "./notifications";

/** The Sites root default, declared here so a loader's dehydrated data and the page agree on freshness. */
export const ACCOUNT_STALE_MS = 30_000;

/** Every account read is the signed-in member's own, so all keys sit under `["private", account, "account", ...]`. */
export const accountKeys = {
  all: (account: string) => ["private", account.toLowerCase(), "account"] as const,
  subscription: (account: string) =>
    ["private", account.toLowerCase(), "account", "subscription"] as const,
  creditsStatus: (account: string) =>
    ["private", account.toLowerCase(), "account", "credits-status"] as const,
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
