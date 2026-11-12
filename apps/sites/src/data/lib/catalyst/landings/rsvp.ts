import { z } from "zod";

import { okDataTotalOf } from "../envelope";
import { writeAttendance } from "../places/attendance";
import type { AuthIdentity } from "../../auth/types";
import type {
  CommitFn,
  RsvpResult,
} from "@/features/social/rsvp-event/machine";

const AttendeeListEnvelope = okDataTotalOf(z.array(z.unknown()));

export function buildRsvpCommit(identitySource: AuthIdentity | null | (() => AuthIdentity | null)): CommitFn {
  return async ({ eventId, direction, count, signal }): Promise<RsvpResult> => {
    const identity = typeof identitySource === "function" ? identitySource() : identitySource;
    if (!identity) throw new Error("Sign in to RSVP.");
    if (!eventId) throw new Error("This event is no longer available.");

    const raw = await writeAttendance(identity, eventId, direction === "going", signal);

    const env = AttendeeListEnvelope.safeParse(raw);
    if (!env.success) return { count };
    return { count: env.data.total ?? env.data.data.length };
  };
}
