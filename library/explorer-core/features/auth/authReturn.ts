// A leaf with no imports: each host's lib/entryParams.ts lists this param, and @hud-host reads that
// module, so importing anything here would pull the HUD into the host module's own load.

/** On the url the auth site returns to (features/auth/sso): the sign-in just happened. */
export const AUTH_RETURN_PARAM = 'signedIn'
