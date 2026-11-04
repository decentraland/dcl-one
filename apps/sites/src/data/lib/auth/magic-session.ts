// from: auth@e081cc6:src/shared/utils/magicSdk.ts (createMagicInstance), src/components/Pages/LoginPage/utils.ts (a new
//       social sign-in logs out of Magic first)
// decentraland/auth is Apache-2.0.
// Magic's client and the session it keeps in its own iframe. Signing out here must end that session too: otherwise
// Magic still answers "already signed in" and /auth/callback would sign the previous account back in. A sign-in through
// Magic leaves a marker, so a wallet user's sign-out never loads the SDK.
//
// The SDK versions are upstream's exact pins (decentraland/auth package.json): magic-sdk 27.0.0 and the
// @magic-ext/oauth2 canary that backs the PKCE verifier up to localStorage (auth#304). Move to a stable
// @magic-ext/oauth2 when upstream does.

import { magicApiKey } from "./magic-config";

type Eip1193 = { request(args: { method: string; params?: unknown[] }): Promise<unknown> };
export type MagicClient = {
  user: { isLoggedIn(): Promise<boolean>; logout(): Promise<unknown> };
  oauth2: {
    loginWithRedirect(config: { provider: string; redirectURI: string; customData?: string }): Promise<unknown>;
    getRedirectResult(): Promise<{ oauth?: { userInfo?: { email?: string } } }>;
  };
  rpcProvider: Eip1193;
};

const MARKER = "dcl:auth:magic-session";

export async function magicClient(apiKey: string): Promise<MagicClient> {
  const [{ Magic }, { OAuthExtension }] = await Promise.all([import("magic-sdk"), import("@magic-ext/oauth2")]);
  return new Magic(apiKey, { extensions: [new OAuthExtension()] }) as unknown as MagicClient;
}

/** Notes that this browser's session came through Magic. */
export function markMagicSession(): void {
  try {
    localStorage.setItem(MARKER, "1");
  } catch {
    // Without the marker a sign-out still clears this site's session; only Magic's own may outlive it.
  }
}

/** Ends Magic's session if this browser signed in through it; resolves once Magic has logged out (or could not). */
export async function endMagicSession(): Promise<void> {
  let marked = false;
  try {
    marked = localStorage.getItem(MARKER) !== null;
    localStorage.removeItem(MARKER);
  } catch {
    return;
  }
  const key = magicApiKey();
  if (!marked || !key) return;
  try {
    await (await magicClient(key)).user.logout();
  } catch {
    // Magic unreachable: its session expires on its own; this site's is already gone.
  }
}
