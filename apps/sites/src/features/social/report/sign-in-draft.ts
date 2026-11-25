import { z } from "zod";

import { REPORT_LIMITS, REPORT_REASONS } from "@/data/lib/catalyst/landings/report";
import { signInDraftStore } from "../../components/auth/sign-in-draft-store";

const fields = z.object({
  reportedAddress: z.string(),
  reason: z.enum(["", ...REPORT_REASONS]),
  description: z.string().max(REPORT_LIMITS.descriptionMax),
  comments: z.string().max(REPORT_LIMITS.commentsMax),
  hadEvidence: z.boolean(),
});

const store = signInDraftStore("dcl:report:sign-in-draft:v2", fields, 15 * 60 * 1000);

export const saveSignInDraft = (returnTo: string, draft: z.input<typeof fields>): boolean => store.save(returnTo, draft);
export const takeSignInDraft = (returnTo: string) => store.take(returnTo);
