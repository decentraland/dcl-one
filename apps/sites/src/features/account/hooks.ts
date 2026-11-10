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

/**
 * Writes into a cached entry only while it holds data: an updater returning `undefined` leaves the
 * cache alone. After an account switch the previous account's entries are removed, and a late
 * write must not bring them back.
 */
function updateIfCached<T>(client: QueryClient, key: QueryKey, update: (current: T) => T) {
  client.setQueryData<T>(key, (current) => (current === undefined ? current : update(current)));
}

export function useSubscription(identity: AuthIdentity | null) {
  const options = identity ? subscriptionQuery(identity) : null;
  return useQuery({
    queryKey: options?.queryKey ?? accountKeys.subscription(""),
    queryFn: options?.queryFn ?? skipToken,
  });
}

/*
 * Every write takes the signing identity as a variable, not from the hook's props. A pending
 * mutation picks up the latest options on re-render, so after an account switch a callback that
 * read the props would sign or write as the new account. The variables stay the ones it started with.
 */

export type SubscriptionUpdate = { identity: AuthIdentity; details: SubscriptionDetails };
const UPDATE_SUBSCRIPTION = ["account", "update-subscription"] as const;

/**
 * Applies the new details to the cache at once and rolls back to the value before them on failure.
 * Each PUT sends the whole object, so the writes run one at a time (`scope`): a later toggle cannot
 * land before an earlier one and lose it. The PUT reply is not read back, since the server can answer
 * with partial details; once the last pending write settles the subscription is read again.
 */
export function useUpdateSubscription() {
  const client = useQueryClient();
  return useMutation({
    mutationKey: UPDATE_SUBSCRIPTION,
    scope: { id: "account-subscription" },
    mutationFn: ({ identity, details }: SubscriptionUpdate) => putSubscription(identity, details),
    onMutate: async ({ identity, details }) => {
      const key = accountKeys.subscription(identity.signer);
      await client.cancelQueries({ queryKey: key });
      const previous = client.getQueryData<SubscriptionResponse>(key);
      updateIfCached<SubscriptionResponse>(client, key, (current) => ({ ...current, details }));
      return { previous };
    },
    onError: (_error, { identity }, context) => {
      const previous = context?.previous;
      if (!previous) return;
      updateIfCached<SubscriptionResponse>(client, accountKeys.subscription(identity.signer), () => previous);
    },
    onSettled: (_result, _error, { identity }) => {
      if (client.isMutating({ mutationKey: UPDATE_SUBSCRIPTION }) > 1) return;
      return client.invalidateQueries({ queryKey: accountKeys.subscription(identity.signer) });
    },
  });
}

/**
 * The server decides whether a confirmation is pending: the same address sends none, an empty one
 * clears it, and the input is trimmed. So the subscription is read again instead of guessed.
 */
export function useSetEmail() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: ({ identity, email }: { identity: AuthIdentity; email: string }) =>
      putEmail(identity, email),
    onSuccess: (_result, { identity }) =>
      client.invalidateQueries({ queryKey: accountKeys.subscription(identity.signer) }),
  });
}

export function useCreditsStatus(identity: AuthIdentity | null) {
  const options = identity ? creditsStatusQuery(identity) : null;
  return useQuery({
    queryKey: options?.queryKey ?? accountKeys.creditsStatus(""),
    queryFn: options?.queryFn ?? skipToken,
  });
}

export function useOptOutOfCredits() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: ({ identity }: { identity: AuthIdentity }) => optOutOfCredits(identity),
    onSuccess: (_result, { identity }) => {
      updateIfCached<CreditsStatus>(client, accountKeys.creditsStatus(identity.signer), () => ({
        status: "opted_out",
        optedOutAt: new Date().toISOString(),
      }));
    },
  });
}
