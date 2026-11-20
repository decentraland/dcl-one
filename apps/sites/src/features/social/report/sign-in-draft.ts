import { z } from "zod";

import { REPORT_LIMITS, REPORT_REASONS } from "@/data/lib/catalyst/landings/report";

const KEY = "dcl:report:sign-in-draft:v1";
const MAX_AGE_MS = 15 * 60 * 1000;

const fields = z.object({
  reportedAddress: z.string(),
  reason: z.enum(["", ...REPORT_REASONS]),
  description: z.string().max(REPORT_LIMITS.descriptionMax),
  comments: z.string().max(REPORT_LIMITS.commentsMax),
  hadEvidence: z.boolean(),
});
const storedDraft = fields.extend({ expiresAt: z.number(), returnTo: z.string() });

export function saveSignInDraft(returnTo: string, draft: z.input<typeof fields>) {
  try {
    sessionStorage.setItem(KEY, JSON.stringify({ ...draft, returnTo, expiresAt: Date.now() + MAX_AGE_MS }));
  } catch {
    // Sign-in is still available when the browser blocks tab storage.
  }
}

export function takeSignInDraft(returnTo: string) {
  try {
    const value = sessionStorage.getItem(KEY);
    if (value === null) return null;
    sessionStorage.removeItem(KEY);
    const result = storedDraft.safeParse(JSON.parse(value));
    if (!result.success || result.data.expiresAt <= Date.now()) return null;
    if (result.data.returnTo !== returnTo) return null;
    return result.data;
  } catch {
    return null;
  }
}
