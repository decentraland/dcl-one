import { z } from "zod";

import type { AuthIdentity } from "../auth/types";
import { CatalystError, postJSON, signedGetJSON } from "../catalyst/client";

/** The credits user routes are nested under `/shop-api` on the node. */
const USERS = "/shop-api/v1/users";

const CreditsStatusSchema = z.object({
  status: z.enum(["enrolled", "opted_out", "not_registered"]),
  optedOutAt: z.string().nullable(),
});
const CreditsStatusEnvelopeSchema = z.object({ data: CreditsStatusSchema });

export type CreditsStatus = z.infer<typeof CreditsStatusSchema>;

export const NOT_REGISTERED: CreditsStatus = { status: "not_registered", optedOutAt: null };

const isNotFound = (error: unknown) => error instanceof CatalystError && error.status === 404;

/** A wallet the credits program has never seen answers 404, which is "not registered", not an error. */
export async function fetchCreditsStatus(
  identity: AuthIdentity,
  opts: { signal?: AbortSignal } = {},
): Promise<CreditsStatus> {
  const path = `${USERS}/${encodeURIComponent(identity.signer.toLowerCase())}/status`;
  try {
    const raw = await signedGetJSON<unknown>(path, { identity, signal: opts.signal });
    return CreditsStatusEnvelopeSchema.parse(raw).data;
  } catch (error) {
    if (isNotFound(error)) return NOT_REGISTERED;
    throw error;
  }
}

/** Leaves the credits program. A wallet that was never registered is already out, so 404 succeeds. */
export async function optOutOfCredits(
  identity: AuthIdentity,
  opts: { signal?: AbortSignal } = {},
): Promise<void> {
  const path = `${USERS}/${encodeURIComponent(identity.signer.toLowerCase())}`;
  try {
    await postJSON<unknown>(path, undefined, { identity, method: "DELETE", signal: opts.signal });
  } catch (error) {
    if (!isNotFound(error)) throw error;
  }
}
