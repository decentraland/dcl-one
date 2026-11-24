import type { CommunityCreateInput } from "@dcl/one-ui";

import { toErrorMessage } from "@/core/lib/errors";
import { track as defaultTrack, type TrackContext, type TrackFn } from "@/core/lib/telemetry/track";
import { emptyDraft, type CommunityDraft, type CreateResult } from "@/data/lib/catalyst/social/create-community";

import { openSignIn } from "../../components/auth/signin-store";

export type CreateFn = (args: { draft: CommunityDraft; signal?: AbortSignal }) => Promise<CreateResult>;

type SubmitOptions = {
  /** The session is known to be signed out (not still being restored). */
  signedOut: boolean;
  trackCtx: TrackContext;
  track?: TrackFn;
};

// The typed form, kept across the sign-in round trip in this tab only.
const DRAFT_KEY = "dcl.create-community.draft";

/**
 * The form's submit: the route's signed create, with its telemetry. A signed-out visitor keeps the
 * typed form and goes to sign in; while the session is still being restored nothing is sent.
 */
export function submitWith(create: CreateFn | undefined, { signedOut, trackCtx, track = defaultTrack }: SubmitOptions) {
  return async (input: CommunityCreateInput, typed: CommunityCreateInput = input): Promise<CreateResult> => {
    if (!create) {
      if (!signedOut) throw new Error("Still checking your sign-in. Try again in a moment.");
      // What the visitor typed, so a blank description comes back blank; the fallback is applied when sending.
      saveDraft(typed);
      openSignIn();
      throw new Error("Sign in to create a community.");
    }
    track("lp_community_submit_attempted", { privacy: input.privacy, visibility: input.visibility }, trackCtx);
    try {
      const result = await create({ draft: { ...emptyDraft(), ...input } });
      track("lp_community_created", { community_id: result.id }, trackCtx);
      return result;
    } catch (error) {
      track("lp_community_submit_failed", { error: toErrorMessage(error, "create failed") }, trackCtx);
      throw error;
    }
  };
}

function saveDraft(input: CommunityCreateInput): void {
  try {
    sessionStorage.setItem(DRAFT_KEY, JSON.stringify(input));
  } catch {
    // Storage refused (private mode, quota): the visitor signs in and types the form again.
  }
}

/** The form kept before a sign-in, removed once read; nothing when there is none or it is unreadable. */
export function takeDraft(): Partial<CommunityCreateInput> | undefined {
  let raw: string | null;
  try {
    raw = sessionStorage.getItem(DRAFT_KEY);
    sessionStorage.removeItem(DRAFT_KEY);
  } catch {
    return undefined;
  }
  if (!raw) return undefined;
  try {
    const value = JSON.parse(raw) as Record<string, unknown> | null;
    if (!value || typeof value !== "object") return undefined;
    const draft: Partial<CommunityCreateInput> = {};
    if (typeof value.name === "string") draft.name = value.name;
    if (typeof value.description === "string") draft.description = value.description;
    if (value.privacy === "public" || value.privacy === "private") draft.privacy = value.privacy;
    if (value.visibility === "all" || value.visibility === "unlisted") draft.visibility = value.visibility;
    return draft;
  } catch {
    return undefined;
  }
}
