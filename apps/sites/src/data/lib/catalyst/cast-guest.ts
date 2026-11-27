import { generatePrivateKey } from "viem/accounts";

import { createIdentityFromPrivateKey } from "../auth/identity";
import type { AuthIdentity } from "../auth/types";

const LIFETIME_MS = 60 * 60_000;
const REFRESH_BEFORE_MS = 5 * 60_000;
let guest: AuthIdentity | undefined;
let creating: Promise<AuthIdentity> | undefined;

/** A watcher-only identity held in this tab's memory, without changing the signed-in session. */
export async function guestWatcherIdentity(): Promise<AuthIdentity> {
  if (guest && Date.parse(guest.expiration) > Date.now() + REFRESH_BEFORE_MS) return guest;
  if (!creating) {
    creating = createIdentityFromPrivateKey(generatePrivateKey(), { expirationMs: LIFETIME_MS })
      .then((identity) => {
        guest = identity;
        return identity;
      })
      .finally(() => { creating = undefined; });
  }
  return creating;
}
