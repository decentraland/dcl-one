import { useCallback, useEffect, useMemo, useState } from "react";
import type { ReactNode } from "react";
import { useRouteLoaderData } from "react-router";

import { ChromeAuthContext } from "@ui/web/frames/chrome-auth";

import { useAuth } from "@/data/lib/auth/index";
import { installSitesShopClient } from "@/data/lib/auth/shop-client";
import { useProfileIdentity } from "@/data/lib/auth/use-profile-name";
import { NOTIFICATIONS_PREFIX } from "@/data/lib/catalyst/bases";
import { catalystBase } from "@/data/lib/catalyst/client";
import { openSignIn } from "../auth/signin-store";
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

  const [mounted, setMounted] = useState(false);
  useEffect(() => {
    setMounted(true);
  }, []);

  const { isConnected, address, fetch: signedFetch, disconnect } = useAuth();
  const effSignedIn = mounted ? isConnected : !!seedWallet;
  const effAddress = mounted ? (address ?? "") : seedWallet;

  usePrivateQueryLifecycle(effSignedIn ? effAddress : null);

  const { name, avatarUrl, avatarColor } = useProfileIdentity(effAddress || null, effSignedIn);

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
      avatarColor: avatarColor || undefined,
      onSignIn: openSignIn,
      ...hubAccountActions(disconnect),
      fetchNotifications,
    }),
    [effSignedIn, effAddress, name, committee, avatarUrl, avatarColor, fetchNotifications, disconnect],
  );

  return <ChromeAuthContext.Provider value={value}>{children}</ChromeAuthContext.Provider>;
}
