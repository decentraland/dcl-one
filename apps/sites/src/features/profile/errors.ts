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
 * The profile is the route's required read, so any failure of it with nothing to show stops the
 * route: an HTTP error (`CatalystError`), a network failure or a reply that does not parse.
 */
export function isRouteFatal(error: unknown): boolean {
  return error instanceof CatalystError || error instanceof Error;
}

/** The boundary's reading of what failed, for a document response or a client error alike. */
export function profileErrorReason(error: unknown): ProfileErrorReason {
  if (error instanceof ProfileRouteError) return error.reason;
  if (typeof error === "object" && error !== null && "status" in error && (error as { status: number }).status === 404) {
    return "not-found";
  }
  return "unavailable";
}
