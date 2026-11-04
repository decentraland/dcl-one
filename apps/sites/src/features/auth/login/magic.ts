// from: auth@e081cc6:src/components/Pages/LoginPage/utils.ts (connectToSocialProvider: Magic's OAuth redirect, with
//       the return target in its customData), src/shared/utils/magicSdk.ts (createMagicInstance),
//       src/components/Pages/CallbackPage/CallbackPage.tsx (getRedirectResult, the already-signed-in and
//       consumed-session cases), src/shared/locations.ts (parseStateCustomData), src/shared/connection/identity.ts
//       (a fresh session signed by the account; one month)
// decentraland/auth is Apache-2.0.
// Google, Apple, Discord and X sign in through Magic, as on decentraland.org, so an account is the same wallet on both
// sites. Magic's SDK is loaded only when a reader picks one of them, and only in the browser.

import { createIdentityWith } from "@/data/lib/auth/identity";
import type { AuthIdentity } from "@/data/lib/auth/types";

import type { ConnectionOption } from "./options";

/** Upstream's session length: one month. */
export const SESSION_MS = 30 * 24 * 60 * 60 * 1000;

export const MAGIC_USER_EMAIL_KEY = "dcl_magic_user_email";
export const THIRDWEB_USER_EMAIL_KEY = "dcl_thirdweb_user_email";

type Eip1193 = { request(args: { method: string; params?: unknown[] }): Promise<unknown> };
type MagicClient = {
  user: { isLoggedIn(): Promise<boolean>; logout(): Promise<unknown> };
  oauth2: {
    loginWithRedirect(config: { provider: string; redirectURI: string; customData?: string }): Promise<unknown>;
    getRedirectResult(): Promise<{ oauth?: { userInfo?: { email?: string } } }>;
  };
  rpcProvider: Eip1193;
};

async function magic(apiKey: string): Promise<MagicClient> {
  const [{ Magic }, { OAuthExtension }] = await Promise.all([import("magic-sdk"), import("@magic-ext/oauth2")]);
  return new Magic(apiKey, { extensions: [new OAuthExtension()] }) as unknown as MagicClient;
}

function forget(key: string): void {
  try {
    localStorage.removeItem(key);
  } catch {
    // Blocked storage holds nothing to forget.
  }
}

/** Ends any earlier Magic session, then hands the reader to the provider; Magic returns them to /auth/callback. */
export async function startMagicSignIn(apiKey: string, option: ConnectionOption, returnTo: string): Promise<void> {
  const client = await magic(apiKey);
  if (await client.user.isLoggedIn()) await client.user.logout();
  // A previous account's email must not follow this one into its profile setup.
  forget(MAGIC_USER_EMAIL_KEY);
  forget(THIRDWEB_USER_EMAIL_KEY);
  await client.oauth2.loginWithRedirect({
    provider: option === "x" ? "twitter" : option,
    redirectURI: `${window.location.origin}/auth/callback`,
    customData: JSON.stringify({ redirectTo: returnTo }),
  });
}

/** The return target Magic carried through the provider, from the callback's `state` (base64 JSON, then customData). */
export function returnFromState(state: string | null): string | null {
  if (!state) return null;
  try {
    const decoded = JSON.parse(atob(state)) as { customData?: unknown };
    const custom = typeof decoded.customData === "string" ? (JSON.parse(decoded.customData) as { redirectTo?: unknown }) : null;
    return typeof custom?.redirectTo === "string" ? custom.redirectTo : null;
  } catch {
    return null;
  }
}

type MagicFailure = { code?: unknown };

/** The OAuth session was already used (the reader came back to the callback): not an error, just sign in again. */
export function isConsumedSession(err: unknown): boolean {
  return typeof err === "object" && err !== null && (err as MagicFailure).code === "MISSING_PKCE_METADATA";
}

function isAlreadySignedIn(err: unknown): boolean {
  return typeof err === "object" && err !== null && (err as MagicFailure).code === -32600;
}

function utf8ToHex(input: string): `0x${string}` {
  let hex = "0x";
  for (const byte of new TextEncoder().encode(input)) hex += byte.toString(16).padStart(2, "0");
  return hex as `0x${string}`;
}

/** Finishes the provider's sign-in and signs a one-month session with the account Magic holds for it. */
export async function finishMagicSignIn(apiKey: string): Promise<AuthIdentity> {
  const client = await magic(apiKey);
  try {
    const result = await client.oauth2.getRedirectResult();
    const email = result?.oauth?.userInfo?.email;
    if (email) {
      try {
        localStorage.setItem(MAGIC_USER_EMAIL_KEY, email);
      } catch {
        // Only the profile setup's prefill needs it.
      }
    }
  } catch (err) {
    // Magic skips the OAuth check when its own session already exists; that session is the one to use.
    if (!isAlreadySignedIn(err)) throw err;
  }
  const accounts = (await client.rpcProvider.request({ method: "eth_accounts" })) as string[] | undefined;
  const signer = accounts?.[0]?.toLowerCase();
  if (!signer) throw new Error("Could not connect to your account. Please try again.");
  return createIdentityWith(
    signer,
    async (message) => {
      const signature = await client.rpcProvider.request({ method: "personal_sign", params: [utf8ToHex(message), signer] });
      if (typeof signature !== "string" || !signature.startsWith("0x")) throw new Error("The account returned no signature.");
      return signature;
    },
    { expirationMs: SESSION_MS },
  );
}
