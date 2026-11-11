import { queryOptions, skipToken, useMutation, useQueries, useQuery, useQueryClient } from "@tanstack/react-query";

import type { AuthIdentity } from "@/data/lib/auth/types";
import {
  attendanceKeys,
  attendingQuery,
  ATTENDANCE_STALE_MS,
  writeAttendance,
} from "@/data/lib/catalyst/places/attendance";

type Toggle = { identity: AuthIdentity; eventId: string; attending: boolean };

/**
 * The attending write, flipped optimistically and rolled back on failure. The identity travels in the
 * mutation variables, so a write started before an account switch signs as the account that started it.
 */
function useAttendanceToggle() {
  const client = useQueryClient();
  return useMutation({
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
    },
  });
}

/** No member, no read: the entry stays unread under an empty account. */
const unread = (eventId: string) =>
  queryOptions<boolean, Error, boolean, ReturnType<typeof attendanceKeys.attending>>({
    queryKey: attendanceKeys.attending("", eventId),
    queryFn: skipToken,
    staleTime: ATTENDANCE_STALE_MS,
  });

/**
 * Remind Me for one event: the member's attending flag, flipped optimistically and rolled back on failure.
 * `enabled` is false where the page offers no Remind me, so no signed read goes out for it.
 */
export function useRemindMe(identity: AuthIdentity | null, eventId: string, onSignIn: () => void, enabled = true) {
  const query = useQuery(identity && enabled ? attendingQuery(identity, eventId) : unread(eventId));
  const mutation = useAttendanceToggle();

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

export type Reminder = { active: boolean; pending: boolean; toggle: () => void };

/**
 * Remind Me for a list of events: one signed read per event while signed in, none signed out. `failed`
 * names the event whose last write failed for the member signed in now.
 */
export function useReminders(identity: AuthIdentity | null, eventIds: readonly string[], onSignIn: () => void) {
  const results = useQueries({
    queries: eventIds.map((eventId) => (identity ? attendingQuery(identity, eventId) : unread(eventId))),
  });
  const mutation = useAttendanceToggle();
  const byId = new Map(eventIds.map((eventId, i) => [eventId, results[i]]));
  const failedId =
    mutation.isError && mutation.variables?.identity.signer === identity?.signer ? mutation.variables.eventId : null;

  return {
    failed: failedId,
    get: (eventId: string): Reminder => {
      const query = byId.get(eventId);
      const writing = mutation.isPending && mutation.variables?.eventId === eventId;
      return {
        active: identity ? query?.data === true : false,
        pending: Boolean(identity) && (query?.isPending !== false || writing),
        toggle: () => {
          if (!identity) {
            onSignIn();
            return;
          }
          mutation.mutate({ identity, eventId, attending: query?.data !== true });
        },
      };
    },
  };
}
