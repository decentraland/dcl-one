// decentraland.org's rollout flags for a page the server did not render: a client navigation into the Shop (its
// loader runs in the browser) or the native shop. The feature-flags service would tell this document's own Referer that
// every flag is off, so they come from this server's reading (/internal/dapps-flags). Read once per document: a page
// the server rendered, or an earlier navigation, has already primed them.

import { primeRemoteFlags, remoteFlagsPrimed } from "@ui/shop/discovery/lib/remoteFlags";

const TIMEOUT_MS = 1_500;

/** Primes the flags unless already primed; on no answer (or a slow one) they stay off, as before any answer. */
export async function primeDappsFlags(): Promise<void> {
  if (remoteFlagsPrimed()) return;
  try {
    const res = await fetch("/internal/dapps-flags", { signal: AbortSignal.timeout(TIMEOUT_MS) });
    if (!res.ok) {
      await res.body?.cancel().catch(() => {});
      return;
    }
    primeRemoteFlags((await res.json()) as Record<string, boolean>);
  } catch {
    // Off, as before any answer.
  }
}
