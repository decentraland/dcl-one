// from: auth@e081cc6:src/components/Pages/LoginPage/utils.ts (connectToSocialProvider: Magic's OAuth redirect, with
//       the return target in its customData), src/shared/utils/magicSdk.ts (createMagicInstance),
//       src/components/Pages/CallbackPage/CallbackPage.tsx (getRedirectResult, the already-signed-in and
//       consumed-session cases), src/shared/locations.ts (parseStateCustomData), src/shared/connection/identity.ts
//       (a fresh session signed by the account; one month)
// decentraland/auth is Apache-2.0.
// Google, Apple, Discord and X sign in through Magic, as on decentraland.org, so an account is the same wallet on both
// sites. Magic's SDK is loaded only when a reader picks one of them, and only in the browser.

import { createIdentityWith } from "@/data/lib/auth/identity";
import { magicClient, markMagicSession } from "@/data/lib/auth/magic-session";
import type { AuthIdentity } from "@/data/lib/auth/types";

import type { ConnectionOption } from "./options";
import { MAGIC_USER_EMAIL_KEY, SESSION_MS, THIRDWEB_USER_EMAIL_KEY } from "./remember";

export { MAGIC_USER_EMAIL_KEY, rememberEmail, SESSION_MS, THIRDWEB_USER_EMAIL_KEY } from "./remember";

// The sign-in this tab started, also carried through the provider in Magic's state. /auth/callback finishes only a
// return that matches it, once: never a Magic session some earlier sign-in left behind.
const PENDING_RETURN = "dcl:auth:magic-return";

function newNonce(): string {
  const bytes = crypto.getRandomValues(new Uint8Array(16));
  return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
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
  const client = await magicClient(apiKey);
  if (await client.user.isLoggedIn()) await client.user.logout();
  // A previous account's email must not follow this one into its profile setup.
  forget(MAGIC_USER_EMAIL_KEY);
  forget(THIRDWEB_USER_EMAIL_KEY);
  const nonce = newNonce();
  // Without session storage the return could not be matched, so it would be refused: say so now, not after the provider.
  sessionStorage.setItem(PENDING_RETURN, nonce);
  await client.oauth2.loginWithRedirect({
    provider: option === "x" ? "twitter" : option,
    redirectURI: `${window.location.origin}/auth/callback`,
    customData: JSON.stringify({ redirectTo: returnTo, nonce }),
  });
}

/** The customData Magic carried through the provider, from the callback's `state` (base64 JSON, then customData). */
function customData(state: string | null): { redirectTo?: unknown; nonce?: unknown } | null {
  if (!state) return null;
  try {
    const decoded = JSON.parse(atob(state)) as { customData?: unknown };
    return typeof decoded.customData === "string" ? (JSON.parse(decoded.customData) as { redirectTo?: unknown }) : null;
  } catch {
    return null;
  }
}

/** The return target Magic carried through the provider. */
export function returnFromState(state: string | null): string | null {
  const redirectTo = customData(state)?.redirectTo;
  return typeof redirectTo === "string" ? redirectTo : null;
}

/** Whether this return answers the sign-in this tab started; either way that sign-in is used up. */
export function claimMagicReturn(state: string | null): boolean {
  let pending: string | null = null;
  try {
    pending = sessionStorage.getItem(PENDING_RETURN);
    sessionStorage.removeItem(PENDING_RETURN);
  } catch {
    return false;
  }
  const nonce = customData(state)?.nonce;
  return pending !== null && typeof nonce === "string" && nonce === pending;
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
  const client = await magicClient(apiKey);
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
  // Magic now holds a session, whether or not signing below succeeds: sign-out must end it
  // (data/lib/auth/magic-session.ts).
  markMagicSession();
  const accounts = (await client.rpcProvider.request({ method: "eth_accounts" })) as string[] | undefined;
  const signer = accounts?.[0]?.toLowerCase();
  if (!signer) throw new Error("Could not connect to your account. Please try again.");
  const identity = await createIdentityWith(
    signer,
    async (message) => {
      const signature = await client.rpcProvider.request({ method: "personal_sign", params: [utf8ToHex(message), signer] });
      if (typeof signature !== "string" || !signature.startsWith("0x")) throw new Error("The account returned no signature.");
      return signature;
    },
    { expirationMs: SESSION_MS },
  );
  return identity;
}
