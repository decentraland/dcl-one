// from: auth@e081cc6:src/components/FeatureFlagsProvider/FeatureFlagsProvider.tsx (`${FEATURE_FLAGS_URL}/dapps.json`),
//       src/components/Pages/LoginPage/utils.ts (the `dapps-sign-in-primary-option` variant)
// decentraland/auth is Apache-2.0.
// The sign-in layout's rollout, read from Decentraland's feature-flags service under decentraland.org's key, so one
// switch lays out both sign-in pages. The service answers by the site a browser's request comes from (its Referer), so
// the read names decentraland.org, or the site FEATURE_FLAGS_ORIGIN names (a deployment on another domain). It never
// depends on the request: one read a minute, and a failed or slow one counts as no variant (upstream's full layout)
// for that minute too, so a stalled service delays one page load, not every one.

import { withDeadline } from "../request-deadline";
import { ttlMemo } from "../ttl-memo";

export const SIGN_IN_PRIMARY_OPTION = "dapps-sign-in-primary-option";
const DEFAULT_SERVICE = "https://feature-flags.decentraland.org";
const DEFAULT_SITE = "https://decentraland.org";
const TIMEOUT_MS = 1_500;

type FlagsFile = { variants?: Record<string, { name?: unknown; enabled?: unknown } | undefined> };

function service(): string {
  return (process.env.FEATURE_FLAGS_URL || DEFAULT_SERVICE).replace(/\/+$/, "");
}

function site(): string {
  return (process.env.FEATURE_FLAGS_ORIGIN || DEFAULT_SITE).replace(/\/+$/, "");
}

async function load(origin: string): Promise<string | null> {
  try {
    return await withDeadline(async (signal) => {
      const res = await fetch(`${service()}/dapps.json`, { headers: { Referer: `${origin}/` }, signal });
      if (!res.ok) {
        await res.body?.cancel().catch(() => {});
        throw new Error(`feature flags answered ${res.status}`);
      }
      const variant = ((await res.json()) as FlagsFile).variants?.[SIGN_IN_PRIMARY_OPTION];
      return variant?.enabled === true && typeof variant.name === "string" ? variant.name : null;
    }, TIMEOUT_MS);
  } catch {
    return null;
  }
}

const readVariant = ttlMemo({ ttlMs: 60_000, keyOf: (origin: string) => origin, load });

/** The sign-in layout variant, or null for the full layout. */
export function signInLayoutVariant(): Promise<string | null> {
  return readVariant(site());
}

export const resetSignInLayoutVariant = (): void => readVariant.reset();
