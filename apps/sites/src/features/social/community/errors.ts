import { isRouteErrorResponse } from "react-router";
import { ZodError } from "zod";

import { CatalystError } from "@/data/lib/catalyst/client";
import { CommunityNotFoundError } from "@/data/lib/catalyst/social/community-detail";

export type CommunityErrorReason = "not-found" | "unavailable";

/**
 * The failures of the community read that the route's boundary shows: an unknown or malformed id, an
 * error from the node (also raised for a network failure) or a reply that does not parse. Anything
 * else is a bug and goes to the root boundary.
 */
export function isRouteFatal(error: unknown): boolean {
  return error instanceof CommunityNotFoundError || error instanceof CatalystError || error instanceof ZodError;
}

export function isCommunityError(error: unknown): boolean {
  return isRouteErrorResponse(error) || isRouteFatal(error);
}

/** Read the same way from a document request's 404/503 response and from a client navigation's error. */
export function communityErrorReason(error: unknown): CommunityErrorReason {
  if (error instanceof CommunityNotFoundError) return "not-found";
  if (isRouteErrorResponse(error)) return error.status === 404 ? "not-found" : "unavailable";
  return "unavailable";
}
