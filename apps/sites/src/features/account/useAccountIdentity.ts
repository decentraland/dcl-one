import { useEffect, useSyncExternalStore } from "react";

import { useAuth } from "@/data/lib/auth/index";
import type { AuthIdentity } from "@/data/lib/auth/types";
import { openSignIn } from "../components/auth/signin-store";

const noSubscription = () => () => {};

/** False while the server's markup hydrates, so the first render matches it; true after. */
function useHydrated(): boolean {
  return useSyncExternalStore(
    noSubscription,
    () => true,
    () => false,
  );
}

export type AccountIdentity =
  | { state: "checking"; identity: null }
  | { state: "signed-out"; identity: null }
  | { state: "signed-in"; identity: AuthIdentity };

/**
 * The account pages sign every read with the browser's session, which the server never holds. The
 * server only sees the wallet cookie, so a session that expired since sends the reader to sign in here.
 */
export function useAccountIdentity(): AccountIdentity {
  const { identity, status } = useAuth();
  const hydrated = useHydrated();
  const signedOut = hydrated && status !== "connecting" && !identity;

  useEffect(() => {
    if (signedOut) openSignIn();
  }, [signedOut]);

  if (identity && hydrated) return { state: "signed-in", identity };
  return signedOut ? { state: "signed-out", identity: null } : { state: "checking", identity: null };
}
