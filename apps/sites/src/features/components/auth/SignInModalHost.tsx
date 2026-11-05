import { lazy, Suspense, useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router";

import type { SignInSocialProvider } from "@ui/components/SignInModalView";

import { magicApiKey } from "@/data/lib/auth/magic-config";
import { hasWalletConnect } from "@ui/data/auth/walletconnect-config";
import { useAuth } from "@/data/lib/auth/index";
import {
  beginShellBrowserSignIn,
  isDesktopShell,
} from "@/data/lib/auth/native-shell";
import { rememberEmail, SESSION_MS } from "../../auth/login/remember";
import { closeSignIn, peekPendingRedirect, takePendingRedirect, useSignInOpen } from "./signin-store";

const SignInModalView = lazy(() => import("@ui/components/SignInModalView"));


export default function SignInModalHost() {
  const open = useSignInOpen();
  const auth = useAuth();
  const { isConnected } = auth;
  const navigate = useNavigate();
  // Social sign-in leaves the page for the provider; one start at a time, and a failure is said, not swallowed.
  const socialStarting = useRef(false);
  const [socialError, setSocialError] = useState<string | null>(null);

  // The host stays mounted: each opening starts clean, and so does a page restored from the back/forward cache after
  // the provider redirect (its social start would otherwise still count as in flight).
  useEffect(() => {
    socialStarting.current = false;
    setSocialError(null);
  }, [open]);
  useEffect(() => {
    const onPageShow = (e: PageTransitionEvent) => {
      if (!e.persisted) return;
      socialStarting.current = false;
      setSocialError(null);
    };
    window.addEventListener("pageshow", onPageShow);
    return () => window.removeEventListener("pageshow", onPageShow);
  }, []);

  useEffect(() => {
    if (open && isConnected) {
      closeSignIn();
      const dest = takePendingRedirect();
      if (dest) navigate(dest);
    }
  }, [open, isConnected, navigate]);

  if (!open || isConnected) return null;
  return (
    <Suspense fallback={null}>
      <SignInModalView
        onClose={closeSignIn}
        inAppAvailable={auth.inAppAvailable}
        socialAvailable={magicApiKey().length > 0}
        walletAvailable={auth.walletAvailable}
        wallets={auth.detectedWallets}
        desktopShell={isDesktopShell()}
        authError={socialError ?? auth.error}
        onStartEmailSignIn={(email) => auth.startEmailSignIn(email)}
        // Sessions as /auth/login signs them: a month, with the email kept for profile setup (a wallet keeps none).
        onVerifyEmailSignIn={async (email, code) => {
          const identity = await auth.verifyEmailSignIn(email, code, { expirationMs: SESSION_MS });
          if (identity) rememberEmail(email);
          return identity;
        }}
        // Through Magic, as /auth/login and decentraland.org: one social account, one wallet. Magic brings the reader
        // back to this page (with its query) through /auth/callback.
        onSocialSignIn={(provider: SignInSocialProvider) => {
          if (socialStarting.current) return;
          socialStarting.current = true;
          setSocialError(null);
          const here = window.location.pathname + window.location.search;
          // Loaded on use: the SDK and the signing code stay out of every page's first load.
          import("../../auth/login/magic")
            .then(({ startMagicSignIn }) => startMagicSignIn(magicApiKey(), provider, peekPendingRedirect() ?? here))
            .catch((err: unknown) => {
              socialStarting.current = false;
              setSocialError(err instanceof Error && err.message ? err.message : "Couldn't start social sign-in. Please try again.");
            });
        }}
        onConnectWallet={async (walletRdns) => {
          const identity = await auth.connect({ walletRdns, expirationMs: SESSION_MS });
          if (identity) rememberEmail(null);
          return identity;
        }}
        onWalletConnect={
          hasWalletConnect()
            ? async (signal) => {
                const session = await auth.connectWalletConnect({ signal, expirationMs: SESSION_MS });
                void session.connected.then((identity) => identity && rememberEmail(null)).catch(() => {});
                return session;
              }
            : undefined
        }
        onBeginShellSignIn={beginShellBrowserSignIn}
      />
    </Suspense>
  );
}
