// from: auth@e081cc6:src/components/Pages/CallbackPage/CallbackPage.tsx (logInAndRedirect: a cancelled provider and a
//       consumed OAuth session go back to sign-in, anything else shows the error with Try Again)
// decentraland/auth is Apache-2.0.
// Where Magic returns a reader after Google, Apple, Discord or X: the account signs a session, which is kept here and
// shared with decentraland.org's dapps, and the reader goes on to the page they signed in from.

import { useCallback, useEffect, useRef, useState } from "react";

import { setIdentity } from "@/data/lib/auth/session";

import type { ConnectionState } from "./flow";
import { ConnectionModal } from "./LoginModals";
import { claimMagicReturn, finishMagicSignIn, isConsumedSession, returnFromState } from "./magic";
import { loginPath, safeRedirectTarget } from "./redirect";

export default function MagicCallback({ magicApiKey }: { magicApiKey: string | null }) {
  const [state, setState] = useState<ConnectionState>("validating");
  const [detail, setDetail] = useState<string | null>(null);
  const started = useRef(false);
  const returnTo = useRef("/");

  const backToSignIn = useCallback(() => window.location.replace(loginPath(returnTo.current)), []);

  useEffect(() => {
    if (started.current) return;
    started.current = true;
    const params = new URLSearchParams(window.location.search);
    returnTo.current = safeRedirectTarget(returnFromState(params.get("state")), new URL(window.location.href));
    const error = params.get("error");
    // Only the return of a sign-in this tab started finishes, once. Anything else (a cancel at the provider, a bookmark,
    // the history, a crafted link) must not sign in whatever Magic session this browser still holds.
    const claimed = claimMagicReturn(params.get("state"));
    if (error === "access_denied" || !claimed) {
      backToSignIn();
      return;
    }
    if (error) {
      setDetail(params.get("error_description") || null);
      setState("error-generic");
      return;
    }
    if (!magicApiKey) {
      setDetail("Social sign-in is not configured on this site.");
      setState("error-generic");
      return;
    }
    void (async () => {
      try {
        setIdentity(await finishMagicSignIn(magicApiKey));
        setState("redirecting");
        window.location.assign(returnTo.current);
      } catch (err) {
        if (isConsumedSession(err)) {
          backToSignIn();
          return;
        }
        setDetail(err instanceof Error ? err.message : null);
        setState("error-generic");
      }
    })();
  }, [backToSignIn, magicApiKey]);

  return <ConnectionModal state={state} detail={detail} pairing={null} onClose={backToSignIn} onTryAgain={backToSignIn} />;
}
