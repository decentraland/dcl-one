import {
  skipToken,
  useMutation,
  useQuery,
  useQueryClient,
  type QueryClient,
  type QueryKey,
} from "@tanstack/react-query";

import type { AuthIdentity } from "@/data/lib/auth/types";
import { optOutOfCredits, type CreditsStatus } from "@/data/lib/account/credits";
import {
  putEmail,
  putSubscription,
  type SubscriptionDetails,
  type SubscriptionResponse,
} from "@/data/lib/account/notifications";
import { accountKeys, creditsStatusQuery, subscriptionQuery } from "@/data/lib/account/queries";

const SIGNED_OUT = "Sign in to manage your account.";

/**
 * Writes into a cached entry only while it holds data: an updater returning `undefined` leaves the
 * cache alone. After an account switch the previous account's entries are removed, and a late
 * write must not bring them back.
 */
function updateIfCached<T>(client: QueryClient, key: QueryKey, update: (current: T) => T) {
  client.setQueryData<T>(key, (current) => (current === undefined ? current : update(current)));
}

function requireIdentity(identity: AuthIdentity | null): AuthIdentity {
  if (!identity) throw new Error(SIGNED_OUT);
  return identity;
}

export function useSubscription(identity: AuthIdentity | null) {
  const options = identity ? subscriptionQuery(identity) : null;
  return useQuery({
    queryKey: options?.queryKey ?? accountKeys.subscription(""),
    queryFn: options?.queryFn ?? skipToken,
  });
}

/**
 * Applies the new details to the cache at once so the switches stay where the member put them.
 * The PUT reply is not read back: the server can answer with partial details. On failure the
 * subscription is read again, since the last good state may include a later toggle.
 */
export function useUpdateSubscription(identity: AuthIdentity | null) {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (details: SubscriptionDetails) => putSubscription(requireIdentity(identity), details),
    onMutate: async (details) => {
      if (!identity) return;
      const key = accountKeys.subscription(identity.signer);
      await client.cancelQueries({ queryKey: key });
      updateIfCached<SubscriptionResponse>(client, key, (current) => ({ ...current, details }));
    },
    onError: () => {
      if (!identity) return;
      void client.invalidateQueries({ queryKey: accountKeys.subscription(identity.signer) });
    },
  });
}

/** The new address stays pending until the member follows the confirmation link. */
export function useSetEmail(identity: AuthIdentity | null) {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (email: string) => putEmail(requireIdentity(identity), email),
    onSuccess: (_result, email) => {
      if (!identity) return;
      updateIfCached<SubscriptionResponse>(
        client,
        accountKeys.subscription(identity.signer),
        (current) => ({ ...current, unconfirmedEmail: email }),
      );
    },
  });
}

export function useCreditsStatus(identity: AuthIdentity | null) {
  const options = identity ? creditsStatusQuery(identity) : null;
  return useQuery({
    queryKey: options?.queryKey ?? accountKeys.creditsStatus(""),
    queryFn: options?.queryFn ?? skipToken,
  });
}

export function useOptOutOfCredits(identity: AuthIdentity | null) {
  const client = useQueryClient();
  return useMutation({
    mutationFn: () => optOutOfCredits(requireIdentity(identity)),
    onSuccess: () => {
      if (!identity) return;
      updateIfCached<CreditsStatus>(client, accountKeys.creditsStatus(identity.signer), () => ({
        status: "opted_out",
        optedOutAt: new Date().toISOString(),
      }));
    },
  });
}
