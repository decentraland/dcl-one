import { skipToken, useQuery } from "@tanstack/react-query";

import { useAuth } from "@/data/lib/auth/context";
import type { AuthIdentity } from "@/data/lib/auth/types";
import { CatalystError } from "@/data/lib/catalyst/client";
import { eventAccessQuery, eventAdminKeys, type EventAccess } from "@/data/lib/catalyst/places/event-management";

export type EventAdminAccess = {
  identity: AuthIdentity | null;
  /** The signed-in account's permissions are still being read. */
  loading: boolean;
  /** The read failed for a reason other than a refusal; the page offers a retry. */
  error: Error | null;
  retry: () => void;
  permissions: readonly string[];
  /** Any grant at all: decentraland.org shows the admin bar to such an account. */
  isAdmin: boolean;
  /** Approving any event: another creator's only together with `canEditAny`, as the API requires. */
  canApproveAny: boolean;
  /** Approving the account's own events only; the API scopes its list and its approvals to them. */
  canApproveOwn: boolean;
  canEditAny: boolean;
  /** Pending Events, as decentraland.org's: approving any or own events, or editing any event. */
  canSeePending: boolean;
  /** Users: the moderator-only list of grants answered for this account. */
  canManageUsers: boolean;
  profiles: EventAccess["profiles"];
};

const NONE: readonly string[] = [];

/** The signed-in account's event administration rights, shared by the admin bar and the admin pages. */
export function useEventAdmin(): EventAdminAccess {
  const auth = useAuth();
  const identity = auth.isConnected ? auth.identity : null;
  const query = useQuery(
    identity
      ? eventAccessQuery(identity)
      : { queryKey: eventAdminKeys.access("anonymous"), queryFn: skipToken },
  );
  // A refused read means the account administers nothing; it is not a failure to retry.
  const refused = query.error instanceof CatalystError && (query.error.status === 401 || query.error.status === 403);
  const permissions = query.data?.profile.permissions ?? NONE;
  const has = (permission: string) => permissions.includes(permission);
  const canApproveAny = has("approve_any_event");
  const canApproveOwn = has("approve_own_event");
  const canEditAny = has("edit_any_event");
  const profiles = query.data?.profiles ?? null;
  return {
    identity,
    loading: Boolean(identity) && query.isPending && !refused,
    error: query.error && !refused && !query.data ? query.error : null,
    retry: () => void query.refetch(),
    permissions,
    isAdmin: permissions.length > 0,
    canApproveAny,
    canApproveOwn,
    canEditAny,
    canSeePending: canApproveAny || canApproveOwn || canEditAny,
    canManageUsers: profiles !== null,
    profiles,
  };
}
