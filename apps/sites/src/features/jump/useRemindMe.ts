import { skipToken, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import type { AuthIdentity } from "@/data/lib/auth/types";
import {
  attendanceKeys,
  attendingQuery,
  ATTENDANCE_STALE_MS,
  writeAttendance,
} from "@/data/lib/catalyst/places/attendance";

type Toggle = { identity: AuthIdentity; eventId: string; attending: boolean };

/**
 * Remind Me for one event: the member's attending flag, flipped optimistically and rolled back on failure.
 * The identity travels in the mutation variables, so a write started before an account switch signs as the
 * account that started it.
 */
/** `enabled` is false where the page offers no Remind me, so no signed read goes out for it. */
export function useRemindMe(identity: AuthIdentity | null, eventId: string, onSignIn: () => void, enabled = true) {
  const client = useQueryClient();
  const query = useQuery(
    identity && enabled
      ? attendingQuery(identity, eventId)
      : { queryKey: attendanceKeys.attending("", eventId), queryFn: skipToken, staleTime: ATTENDANCE_STALE_MS },
  );
  const mutation = useMutation({
    mutationFn: ({ identity, eventId, attending }: Toggle) => writeAttendance(identity, eventId, attending),
    onMutate: async ({ identity, eventId, attending }: Toggle) => {
      const key = attendanceKeys.attending(identity.signer, eventId);
      await client.cancelQueries({ queryKey: key });
      const previous = client.getQueryData<boolean>(key);
      client.setQueryData(key, attending);
      return { key, previous };
    },
    onError: (_error, _toggle, context) => {
      // A sign-out or account switch removed the entry meanwhile; restoring it would bring back another account's flag.
      if (!context || client.getQueryState(context.key) === undefined) return;
      // setQueryData ignores undefined, so a flag that was never read goes back to unread instead.
      if (context.previous === undefined) client.removeQueries({ queryKey: context.key, exact: true });
      else client.setQueryData(context.key, context.previous);
    },
    // Not awaited: a slow re-read must not keep the toggle disabled after the write settled.
    onSettled: (_data, _error, { identity, eventId }) => {
      void client.invalidateQueries({ queryKey: attendanceKeys.attending(identity.signer, eventId) });
      // The Events page reads the member's whole attending list; it changed too.
      void client.invalidateQueries({ queryKey: attendanceKeys.list(identity.signer) });
    },
  });

  return {
    active: identity ? query.data === true : false,
    pending: Boolean(identity) && (query.isPending || mutation.isPending),
    // A failed write belongs to the account that made it, not to whoever is signed in now.
    failed: mutation.isError && mutation.variables?.identity.signer === identity?.signer,
    toggle: () => {
      if (!identity) {
        onSignIn();
        return;
      }
      mutation.mutate({ identity, eventId, attending: query.data !== true });
    },
  };
}
