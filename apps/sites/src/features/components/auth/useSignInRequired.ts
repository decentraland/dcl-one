import { useEffect, useRef } from "react";
import { useLocation } from "react-router";

import { isIdentityExpired } from "@/data/lib/auth/expiry";
import { getIdentity } from "@/data/lib/auth/session";

import { openSignIn } from "./signin-store";

export function useSignInRequired(required: boolean, isConnected: boolean): void {
  const { pathname, search, hash } = useLocation();
  const returnTo = pathname + search + hash;
  const lastAttempt = useRef<string | null>(null);

  useEffect(() => {
    if (!required || isConnected) {
      lastAttempt.current = null;
      return;
    }
    // The server snapshot is anonymous. Read the local session before acting during hydration.
    const identity = getIdentity();
    if (identity && !isIdentityExpired(identity)) return;
    if (lastAttempt.current === returnTo) return;
    lastAttempt.current = returnTo;
    openSignIn({ redirectTo: returnTo });
  }, [isConnected, required, returnTo]);
}
