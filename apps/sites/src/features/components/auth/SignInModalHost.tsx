import { lazy, Suspense, useEffect } from "react";
import { useNavigate } from "react-router";

import type { SignInSocialProvider } from "@ui/components/SignInModalView";

import { magicApiKey } from "@/data/lib/auth/magic-config";
import { hasWalletConnect } from "@/data/lib/auth/walletconnect";
import { useAuth } from "@/data/lib/auth/index";
import {
  beginShellBrowserSignIn,
  isDesktopShell,
} from "@/data/lib/auth/native-shell";
import { startMagicSignIn } from "../../auth/login/magic";
import { closeSignIn, takePendingRedirect, useSignInOpen } from "./signin-store";

const SignInModalView = lazy(() => import("@ui/components/SignInModalView"));


export default function SignInModalHost() {
  const open = useSignInOpen();
  const auth = useAuth();
  const { isConnected } = auth;
  const navigate = useNavigate();

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
        authError={auth.error}
        onStartEmailSignIn={(email) => auth.startEmailSignIn(email)}
        onVerifyEmailSignIn={(email, code) => auth.verifyEmailSignIn(email, code)}
        // Through Magic, as /auth/login and decentraland.org: one social account, one wallet. Magic brings the reader
        // back to this page (with its query) through /auth/callback.
        onSocialSignIn={(provider: SignInSocialProvider) => {
          const here = window.location.pathname + window.location.search;
          void startMagicSignIn(magicApiKey(), provider, takePendingRedirect() ?? here).catch(() => {});
        }}
        onConnectWallet={(walletRdns) => auth.connect({ walletRdns })}
        onWalletConnect={hasWalletConnect() ? (signal) => auth.connectWalletConnect({ signal }) : undefined}
        onBeginShellSignIn={beginShellBrowserSignIn}
      />
    </Suspense>
  );
}
