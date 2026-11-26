import { isRouteErrorResponse } from "react-router";
import { ZodError } from "zod";

import { CatalystError } from "@/data/lib/catalyst/client";

export type ProfileErrorReason = "not-found" | "unavailable";

/** A profile route cannot render: the address is not a wallet, or the profile service failed. */
export class ProfileRouteError extends Error {
  readonly reason: ProfileErrorReason;
  readonly address: string;
  constructor(reason: ProfileErrorReason, address: string) {
    super(reason === "not-found" ? `No profile can exist at ${address}` : "The profile is unavailable");
    this.name = "ProfileRouteError";
    this.reason = reason;
    this.address = address;
  }
}

/**
 * The failures of the profile read that the route's boundary shows: an error from the node, which
 * the catalyst client also raises for a network failure (`CatalystError`), or a reply that does not
 * parse. Any other error is a bug: the loader lets it become a 500 and the route's boundary hands it
 * to the root's.
 */
export function isRouteFatal(error: unknown): boolean {
  return error instanceof CatalystError || error instanceof ZodError;
}

/** Whether the profile route's boundary renders this error itself rather than passing it up. */
export function isProfileError(error: unknown): boolean {
  return error instanceof ProfileRouteError || isRouteErrorResponse(error) || isRouteFatal(error);
}

/**
 * What failed, read the same way from a document request's status-bearing response
 * (`{ address, reason }` with 404 or 503) and from a client navigation's thrown error.
 */
export function profileErrorReason(error: unknown): ProfileErrorReason {
  if (error instanceof ProfileRouteError) return error.reason;
  if (isRouteErrorResponse(error)) {
    const reason = (error.data as { reason?: ProfileErrorReason } | null)?.reason;
    return reason ?? (error.status === 404 ? "not-found" : "unavailable");
  }
  // Not found means only an address that cannot be a wallet; a node error is "unavailable" on a
  // client navigation as on a document request.
  return "unavailable";
}

/** The address a document request's error response names, when it names one. */
export function profileErrorAddress(error: unknown): string | undefined {
  if (error instanceof ProfileRouteError) return error.address;
  if (isRouteErrorResponse(error)) return (error.data as { address?: string } | null)?.address;
  return undefined;
}
