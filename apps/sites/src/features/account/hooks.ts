import {
  skipToken,
  useMutation,
  useQuery,
  useQueryClient,
  type Mutation,
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
import {
  ACCOUNT_STALE_MS,
  accountKeys,
  creditsStatusQuery,
  subscriptionQuery,
} from "@/data/lib/account/queries";

/**
 * Writes into a cached entry only while it holds data: an updater returning `undefined` leaves the
 * cache alone. After an account switch the previous account's entries are removed, and a late
 * write must not bring them back.
 */
function updateIfCached<T>(client: QueryClient, key: QueryKey, update: (current: T) => T) {
  client.setQueryData<T>(key, (current) => (current === undefined ? current : update(current)));
}

export function useSubscription(identity: AuthIdentity | null) {
  return useQuery(
    identity
      ? subscriptionQuery(identity)
      : { queryKey: accountKeys.subscription(""), queryFn: skipToken, staleTime: ACCOUNT_STALE_MS },
  );
}

/*
 * Every write takes the signing identity as a variable, not from the hook's props. A pending
 * mutation picks up the latest options on re-render, so after an account switch a callback that
 * read the props would sign or write as the new account. The variables stay the ones it started with.
 */

export type SubscriptionUpdate = { identity: AuthIdentity; details: SubscriptionDetails };
const UPDATE_SUBSCRIPTION = ["account", "update-subscription"] as const;

/**
 * The subscription the server last accepted, per client and account, while a chain of writes is
 * pending. A failed write restores it, never the optimistic value an earlier queued write left.
 */
const confirmedSubscriptions = new WeakMap<QueryClient, Map<string, SubscriptionResponse>>();

function confirmedFor(client: QueryClient) {
  let confirmed = confirmedSubscriptions.get(client);
  if (!confirmed) {
    confirmed = new Map();
    confirmedSubscriptions.set(client, confirmed);
  }
  return confirmed;
}

/** Pending subscription writes of one account, so another account's writes never count. */
function pendingWritesOf(client: QueryClient, account: string) {
  return client.isMutating({
    mutationKey: UPDATE_SUBSCRIPTION,
    predicate: (mutation: Mutation<unknown, unknown, unknown>) =>
      (mutation.state.variables as SubscriptionUpdate | undefined)?.identity.signer.toLowerCase() === account,
  });
}

/**
 * Applies the new details to the cache at once. Each PUT sends the whole object, so the writes run
 * one at a time (`scope`): a later toggle cannot land before an earlier one and lose it. `onMutate`
 * still runs when a write is queued, so a failure does not restore its own snapshot: while a later
 * write of the same account is pending, that write carries the visible details and its outcome
 * decides. When the last pending write fails, the cache returns to the details the server last
 * accepted in the chain. The PUT reply is not read back, since the server can answer with partial
 * details; once the last pending write settles the subscription is read again.
 */
export function useUpdateSubscription() {
  const client = useQueryClient();
  return useMutation({
    mutationKey: UPDATE_SUBSCRIPTION,
    scope: { id: "account-subscription" },
    mutationFn: ({ identity, details }: SubscriptionUpdate) => putSubscription(identity, details),
    onMutate: async ({ identity, details }) => {
      const account = identity.signer.toLowerCase();
      const key = accountKeys.subscription(account);
      await client.cancelQueries({ queryKey: key });
      const confirmed = confirmedFor(client);
      const current = client.getQueryData<SubscriptionResponse>(key);
      if (!confirmed.has(account) && current) confirmed.set(account, current);
      updateIfCached<SubscriptionResponse>(client, key, (cached) => ({ ...cached, details }));
    },
    onSuccess: (_result, { identity, details }) => {
      const account = identity.signer.toLowerCase();
      const confirmed = confirmedFor(client);
      const accepted = confirmed.get(account);
      if (accepted) confirmed.set(account, { ...accepted, details });
    },
    onError: (_error, { identity }) => {
      const account = identity.signer.toLowerCase();
      if (pendingWritesOf(client, account) > 1) return;
      const accepted = confirmedFor(client).get(account);
      if (accepted) updateIfCached<SubscriptionResponse>(client, accountKeys.subscription(account), () => accepted);
    },
    onSettled: (_result, _error, { identity }) => {
      const account = identity.signer.toLowerCase();
      if (pendingWritesOf(client, account) > 1) return;
      confirmedFor(client).delete(account);
      return client.invalidateQueries({ queryKey: accountKeys.subscription(account) });
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
  return useQuery(
    identity
      ? creditsStatusQuery(identity)
      : { queryKey: accountKeys.creditsStatus(""), queryFn: skipToken, staleTime: ACCOUNT_STALE_MS },
  );
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
