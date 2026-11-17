import { createShopApi, ENDPOINTS, shopApiMode, type Endpoint, type ShopApiMode } from "@ui/shop/api";
import { z } from "zod";

import type { AuthIdentity } from "../auth/types";
import { CatalystError, postJSON, signedGetJSON } from "../catalyst/client";

const CreditsStatusSchema = z.object({
  status: z.enum(["enrolled", "opted_out", "not_registered"]),
  optedOutAt: z.string().nullable(),
});
const CreditsStatusEnvelopeSchema = z.object({ data: CreditsStatusSchema });

export type CreditsStatus = z.infer<typeof CreditsStatusSchema>;

export const NOT_REGISTERED: CreditsStatus = { status: "not_registered", optedOutAt: null };

type CreditsOptions = { signal?: AbortSignal; mode?: ShopApiMode };

/**
 * The node serves the credits user routes under `/shop-api` or, in legacy mode, under `/credits`. The
 * Shop API's endpoint table owns that mapping; only the path is taken from it, the request is signed here.
 */
function creditsPath(endpoint: Endpoint, identity: AuthIdentity, mode: ShopApiMode = shopApiMode()) {
  const paths = createShopApi({
    base: "",
    mode,
    transport: () => Promise.reject(new Error("credits paths are resolved, not requested, here")),
  });
  return paths.path(endpoint, { address: identity.signer.toLowerCase() });
}

const isNotFound = (error: unknown) => error instanceof CatalystError && error.status === 404;

/** A wallet the credits program has never seen answers 404, which is "not registered", not an error. */
export async function fetchCreditsStatus(
  identity: AuthIdentity,
  opts: CreditsOptions = {},
): Promise<CreditsStatus> {
  const path = creditsPath(ENDPOINTS.userStatus, identity, opts.mode);
  try {
    const raw = await signedGetJSON<unknown>(path, { identity, signal: opts.signal });
    return CreditsStatusEnvelopeSchema.parse(raw).data;
  } catch (error) {
    if (isNotFound(error)) return NOT_REGISTERED;
    throw error;
  }
}

/** Leaves the credits program. A wallet that was never registered is already out, so 404 succeeds. */
export async function optOutOfCredits(identity: AuthIdentity, opts: CreditsOptions = {}): Promise<void> {
  const path = creditsPath(ENDPOINTS.unregisterUser, identity, opts.mode);
  try {
    await postJSON<unknown>(path, undefined, { identity, method: "DELETE", signal: opts.signal });
  } catch (error) {
    if (!isNotFound(error)) throw error;
  }
}
