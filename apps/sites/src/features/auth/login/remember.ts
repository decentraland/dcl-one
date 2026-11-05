// from: auth@e081cc6:src/shared/connection/identity.ts (a session lasts one month), src/components/Pages/LoginPage
//       (the email a reader signed in with, kept for profile setup)
// decentraland/auth is Apache-2.0.
// What every sign-in path shares, kept apart from magic.ts so the header's modal loads no SDK or signing code for it.

/** Upstream's session length: one month. */
export const SESSION_MS = 30 * 24 * 60 * 60 * 1000;

export const MAGIC_USER_EMAIL_KEY = "dcl_magic_user_email";
export const THIRDWEB_USER_EMAIL_KEY = "dcl_thirdweb_user_email";

/** Upstream keeps the email a reader signed in with for their profile setup; a wallet sign-in has none. */
export function rememberEmail(email: string | null): void {
  try {
    localStorage.removeItem(MAGIC_USER_EMAIL_KEY);
    if (email) localStorage.setItem(THIRDWEB_USER_EMAIL_KEY, email);
    else localStorage.removeItem(THIRDWEB_USER_EMAIL_KEY);
  } catch {
    // Only the profile setup's prefill needs it.
  }
}
