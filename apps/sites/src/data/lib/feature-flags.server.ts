// from: shop@94584a5c:app/src/lib/featureFlags.ts (getIsFeatureEnabled: `${FEATURE_FLAGS_URL}/dapps.json`, a flag on
//       only when the file says `true`)
// The `dapps` rollout flags decentraland.org's sites read, read here by the server. The service answers by the site a
// browser's request comes from (its Referer), so a browser on any other domain (a preview, this node's own) would be
// told every flag is off: the server asks as decentraland.org (or the site FEATURE_FLAGS_ORIGIN names) and hands the
// answer to the page. One read a minute whatever the request; a failed or slow one keeps the last good answer for that
// minute ("all off" only before any read has succeeded). The variants' payloads (an address list, for one) come with
// the flags, kept as upstream keeps them (`parseRemoteFlags`).

import { parseRemoteFlags, type RemoteFlagsAnswer } from "@ui/shop/discovery/lib/remoteFlags";

import { withDeadline } from "./request-deadline";
import { ttlMemo } from "./ttl-memo";

const DEFAULT_SERVICE = "https://feature-flags.decentraland.org";
const DEFAULT_SITE = "https://decentraland.org";
const TIMEOUT_MS = 1_500;

export type DappsFlags = Readonly<Record<string, boolean>>;
export type DappsFlagVariants = Readonly<Record<string, string>>;
export type DappsFlagsAnswer = { readonly flags: DappsFlags; readonly variants: DappsFlagVariants };

const NONE: DappsFlagsAnswer = { flags: {}, variants: {} };

function service(): string {
  return (process.env.FEATURE_FLAGS_URL || DEFAULT_SERVICE).replace(/\/+$/, "");
}

function site(): string {
  return (process.env.FEATURE_FLAGS_ORIGIN || DEFAULT_SITE).replace(/\/+$/, "");
}

const lastGood = new Map<string, DappsFlagsAnswer>();

async function load(origin: string): Promise<DappsFlagsAnswer> {
  try {
    const answer: RemoteFlagsAnswer = await withDeadline(async (signal) => {
      const res = await fetch(`${service()}/dapps.json`, { headers: { Referer: `${origin}/` }, signal });
      if (!res.ok) {
        await res.body?.cancel().catch(() => {});
        throw new Error(`feature flags answered ${res.status}`);
      }
      return parseRemoteFlags(await res.json());
    }, TIMEOUT_MS);
    lastGood.set(origin, answer);
    return answer;
  } catch {
    return lastGood.get(origin) ?? NONE;
  }
}

const read = ttlMemo({ ttlMs: 60_000, keyOf: (origin: string) => origin, load });

/** decentraland.org's `dapps` flags and their variants' payloads, as last read. */
export function dappsFlagsAnswer(): Promise<DappsFlagsAnswer> {
  return read(site());
}

/** decentraland.org's `dapps` flags, each true only where the service says so. */
export async function dappsFlags(): Promise<DappsFlags> {
  return (await read(site())).flags;
}

export const resetDappsFlags = (): void => {
  read.reset();
  lastGood.clear();
};
