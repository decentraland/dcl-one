// decentraland.org's rollout flags for a page the server did not render: a client navigation into the Shop (its
// loader runs in the browser) or the native shop. The feature-flags service would tell this document's own Referer that
// every flag is off, so they come from this server's reading (/internal/feature-flags). Read once per document: a page
// the server rendered, or an earlier navigation, has already primed them. A flag hook on any other page takes
// `readDappsFlags` as its reader, never the service: it refuses this site's origin.

import { primeRemoteFlags, remoteFlagsPrimed, type RemoteFlagsAnswer } from "@dcl/one-ui/apps/shop/remote-flags";

const TIMEOUT_MS = 1_500;

/** This server's reading of the flags; throws when it has no answer. */
export async function readDappsFlags(signal: AbortSignal): Promise<RemoteFlagsAnswer> {
  const res = await fetch("/internal/feature-flags?variants=1", { signal });
  if (!res.ok) {
    await res.body?.cancel().catch(() => {});
    throw new Error(`feature flags: HTTP ${res.status}`);
  }
  const body = (await res.json()) as { flags?: Record<string, boolean>; variants?: Record<string, string> } | Record<string, boolean>;
  // `{ flags, variants }`; a bare flags map is what a server from before the variants answers.
  if (body && typeof body.flags === "object" && body.flags !== null) {
    const answer = body as { flags: Record<string, boolean>; variants?: Record<string, string> };
    return { flags: answer.flags, variants: answer.variants ?? {} };
  }
  return { flags: body as Record<string, boolean>, variants: {} };
}

/** Primes the flags unless already primed; on no answer (or a slow one) they stay off, as before any answer. */
export async function primeDappsFlags(): Promise<void> {
  if (remoteFlagsPrimed()) return;
  try {
    const { flags, variants } = await readDappsFlags(AbortSignal.timeout(TIMEOUT_MS));
    // Shop may have hydrated with its loader answer while this request was pending.
    if (!remoteFlagsPrimed()) primeRemoteFlags(flags, variants);
  } catch {
    // Off, as before any answer.
  }
}
