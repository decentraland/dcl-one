// from: auth@e081cc6:src/components/Pages/CallbackPage/CallbackPage.tsx (logInAndRedirect: a cancelled provider and a
//       consumed OAuth session go back to sign-in, anything else shows the error with Try Again)
// decentraland/auth is Apache-2.0.
// Where Magic returns a reader after Google, Apple, Discord or X: the account signs a session, which is kept here and
// shared with decentraland.org's dapps, and the reader goes on to the page they signed in from.

import { useCallback, useEffect, useRef, useState } from "react";

import { setIdentity } from "@/data/lib/auth/session";

import type { ConnectionState } from "./flow";
import { ConnectionModal } from "./LoginModals";
import { finishMagicSignIn, isConsumedSession, returnFromState } from "./magic";
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
    // The reader cancelled at the provider, or this is no return from Magic at all (a bookmark, the history): only a
    // return carrying Magic's state may finish a sign-in, never a Magic session left over from an earlier one.
    if (params.get("error") === "access_denied" || !params.has("state")) {
      backToSignIn();
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
