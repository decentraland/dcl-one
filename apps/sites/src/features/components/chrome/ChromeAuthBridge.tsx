import { useCallback, useEffect, useMemo, useState } from "react";
import type { ReactNode } from "react";
import { useNavigate, useRouteLoaderData } from "react-router";

import { ChromeAuthContext } from "@dcl/one-ui/components/AppChrome/chrome-auth";

import { useAuth } from "@/data/lib/auth/index";
import { installSitesShopClient } from "@/data/lib/auth/shop-client";
import { useProfileIdentity } from "@/data/lib/auth/use-profile-name";
import { watchBrowserWalletAccount } from "@/data/lib/auth/wallet-account-switch";
import { NOTIFICATIONS_PREFIX } from "@/data/lib/catalyst/bases";
import { catalystBase } from "@/data/lib/catalyst/client";
import { loginPath } from "@/features/auth/login/redirect";
import { navigateForAccountChange, openSignIn } from "../auth/signin-store";
import { hubAccountActions } from "./explorer-host";
import { usePrivateQueryLifecycle } from "./usePrivateQueryLifecycle";

// The top bar's credits chip reads the balance through the document's Shop client: install it before any render.
installSitesShopClient();

export default function ChromeAuthBridge({ children }: { children: ReactNode }) {
  const rootData = useRouteLoaderData("root") as
    | { wallet?: string | null; committee?: boolean }
    | undefined;
  const seedWallet = rootData?.wallet ?? "";
  const committee = rootData?.committee ?? false;
  const navigate = useNavigate();

  const [mounted, setMounted] = useState(false);
  useEffect(() => {
    setMounted(true);
  }, []);
  useEffect(() => watchBrowserWalletAccount(window, (change) => {
    const { pathname, search, hash } = window.location;
    if (pathname.startsWith("/auth/") && pathname !== "/auth/login") return;
    const returnTo = `${pathname}${search}${hash}`;
    if (change === "activated") {
      navigateForAccountChange(returnTo, () => window.location.reload());
    } else if (pathname !== "/auth/login") {
      navigateForAccountChange(returnTo, () => {
        void navigate(loginPath(returnTo, { walletSwitch: true }), { replace: true });
      });
    }
  }), [navigate]);

  const { isConnected, address, fetch: signedFetch, disconnect } = useAuth();
  const effSignedIn = mounted ? isConnected : !!seedWallet;
  const effAddress = mounted ? (address ?? "") : seedWallet;

  usePrivateQueryLifecycle(effSignedIn ? effAddress : null);

  const { name, avatarUrl, avatarColor, bodyUrl } = useProfileIdentity(effAddress || null, effSignedIn);

  const fetchNotifications = useCallback(async (signal?: AbortSignal): Promise<unknown> => {
    const res = await signedFetch(`${catalystBase()}${NOTIFICATIONS_PREFIX}/notifications?limit=50`, {
      headers: { accept: "application/json" },
      signal,
    });
    if (!res.ok) {
      await res.body?.cancel();
      throw new Error(`Notifications returned ${res.status}`);
    }
    return res.json() as Promise<unknown>;
  }, [signedFetch]);

  const value = useMemo(
    () => ({
      signedIn: effSignedIn,
      account: effAddress,
      mana: "",
      name,
      committee,
      avatarUrl: avatarUrl || undefined,
      bodyUrl: bodyUrl || undefined,
      avatarColor: avatarColor || undefined,
      onSignIn: openSignIn,
      ...hubAccountActions(disconnect),
      fetchNotifications,
      notificationsReady: mounted && isConnected,
    }),
    [mounted, isConnected, effSignedIn, effAddress, name, committee, avatarUrl, avatarColor, bodyUrl, fetchNotifications, disconnect],
  );

  return <ChromeAuthContext.Provider value={value}>{children}</ChromeAuthContext.Provider>;
}
