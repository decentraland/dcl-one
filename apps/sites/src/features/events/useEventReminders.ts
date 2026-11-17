import { skipToken, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";

import type { AuthIdentity } from "@/data/lib/auth/types";
import {
  attendanceKeys,
  attendingListQuery,
  ATTENDANCE_STALE_MS,
  writeAttendance,
} from "@/data/lib/catalyst/places/attendance";

type Toggle = { identity: AuthIdentity; eventId: string; attending: boolean };

export type Reminder = { active: boolean; pending: boolean; toggle: () => void };

const withId = (ids: readonly string[], id: string, attending: boolean): string[] =>
  attending ? (ids.includes(id) ? [...ids] : [...ids, id]) : ids.filter((other) => other !== id);

const TOGGLE_KEY = ["attendance", "toggle"] as const;

/** Pending and failed writes, each by account and event, so concurrent toggles never share state. */
const slot = (signer: string, eventId: string) => `${signer.toLowerCase()}\u0000${eventId}`;

/**
 * Remind Me for every event on a page from one signed read, the member's attending list. A toggle
 * flips that event in the list at once; a failure puts back only that event, so a concurrent toggle
 * on another card keeps its own outcome. The identity travels in the mutation variables, so a write
 * started before an account switch signs, and rolls back, as the account that started it.
 */
export function useEventReminders(identity: AuthIdentity | null, onSignIn: () => void) {
  const client = useQueryClient();
  const list = useQuery(
    identity
      ? attendingListQuery(identity)
      : { queryKey: attendanceKeys.list(""), queryFn: skipToken, staleTime: ATTENDANCE_STALE_MS },
  );
  const [pending, setPending] = useState<ReadonlySet<string>>(new Set());
  const [failed, setFailed] = useState<{ slot: string; eventId: string } | null>(null);

  const mutation = useMutation({
    mutationKey: TOGGLE_KEY,
    mutationFn: ({ identity, eventId, attending }: Toggle) => writeAttendance(identity, eventId, attending),
    onMutate: async ({ identity, eventId, attending }: Toggle) => {
      const key = attendanceKeys.list(identity.signer);
      setPending((prev) => new Set(prev).add(slot(identity.signer, eventId)));
      // A new attempt replaces the last failure notice, whichever card it was on.
      setFailed(null);
      await client.cancelQueries({ queryKey: key });
      const ids = client.getQueryData<string[]>(key);
      if (ids) client.setQueryData(key, withId(ids, eventId, attending));
      return { key, was: ids?.includes(eventId) };
    },
    onError: (_error, { identity, eventId }, context) => {
      setFailed({ slot: slot(identity.signer, eventId), eventId });
      // A sign-out or account switch removed the list meanwhile; restoring it would bring back another account's.
      if (!context || context.was === undefined) return;
      const ids = client.getQueryData<string[]>(context.key);
      if (ids) client.setQueryData(context.key, withId(ids, eventId, context.was));
    },
    onSuccess: () => setFailed(null),
    onSettled: (_data, _error, { identity, eventId }) => {
      setPending((prev) => {
        const next = new Set(prev);
        next.delete(slot(identity.signer, eventId));
        return next;
      });
      // Re-read once the last write settles: an earlier re-read would overwrite the optimistic flags of
      // writes still in flight. Not awaited, so a slow re-read never keeps a toggle disabled.
      if (client.isMutating({ mutationKey: TOGGLE_KEY }) > 1) return;
      // The list and any single-event flag (/jump) of this account.
      void client.invalidateQueries({ queryKey: ["private", identity.signer.toLowerCase(), "attendance"] });
    },
  });

  const signer = identity?.signer ?? "";
  const ids = identity ? list.data : undefined;
  return {
    /** The event whose last write failed for the member signed in now. */
    failed: identity && failed && failed.slot === slot(signer, failed.eventId) ? failed.eventId : null,
    get: (eventId: string): Reminder => ({
      active: ids?.includes(eventId) ?? false,
      pending: Boolean(identity) && (list.isPending || pending.has(slot(signer, eventId))),
      toggle: () => {
        if (!identity) {
          onSignIn();
          return;
        }
        mutation.mutate({ identity, eventId, attending: !(ids?.includes(eventId) ?? false) });
      },
    }),
  };
}
