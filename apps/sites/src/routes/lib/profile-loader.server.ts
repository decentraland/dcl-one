import { dehydrate, QueryClient, type DehydratedState } from "@tanstack/react-query";
import { data, redirect } from "react-router";

import { QUERY_CLIENT_CONFIG } from "@/core/lib/query/defaults";
import { sidLoader } from "@/core/lib/telemetry/session";
import { normalizeAddress } from "@/data/lib/catalyst/social/profile";
import {
  canReadProfile,
  creationsQuery,
  ownedNamesQuery,
  ownedWearablesQuery,
  photosQuery,
  placesQuery,
  profileQuery,
} from "@/data/lib/profile/queries";
import { isOwnProfile, profileHref, readProfileTab, type ProfileTab } from "@/features/profile/tabs";

export type ProfileLoaderData = {
  sid: string;
  address: string;
  tab: ProfileTab;
  /** Whether the verified wallet cookie names this profile, so the first render matches the server. */
  own: boolean;
  dehydratedState: DehydratedState;
};

/** The status-bearing error the route's boundary renders on a document request. */
export type ProfileRouteError = { address: string; reason: "not-found" | "unavailable" };

/** Public reads the active tab shows; a failure leaves them to the browser. */
/** `assets` reaches here only for the member's own profile: `readProfileTab` refuses it otherwise. */
function prefetchTab(client: QueryClient, address: string, tab: ProfileTab) {
  switch (tab) {
    case "creations":
      return client.prefetchQuery(creationsQuery(address));
    case "places":
      return client.prefetchQuery(placesQuery(address));
    case "photos":
      return client.prefetchQuery(photosQuery(address));
    case "assets":
      return Promise.all([
        client.prefetchQuery(ownedWearablesQuery(address)),
        client.prefetchQuery(ownedNamesQuery(address)),
      ]);
    default:
      return undefined;
  }
}

/**
 * `/profile/:address[/:tab]`. The profile itself is required: an address that cannot be a wallet is
 * 404, and a profile service that fails is 503. A wallet with no profile yet is a valid, empty profile.
 * Older links (`?tab=`, `?own=1`, a mixed-case address) are redirected to the canonical path.
 */
export async function profileLoader(
  request: Request,
  params: { address?: string; tab?: string },
): Promise<ReturnType<ReturnType<typeof sidLoader>["wrap"]>> {
  const url = new URL(request.url);
  const raw = params.address ?? "";
  const address = normalizeAddress(raw);
  if (!canReadProfile(address)) {
    throw data({ address: raw, reason: "not-found" } satisfies ProfileRouteError, { status: 404 });
  }

  const session = sidLoader(request);
  const own = isOwnProfile(address, session.wallet);

  const legacyTab = url.searchParams.get("tab");
  if (legacyTab !== null || url.searchParams.has("own")) {
    throw redirect(profileHref(address, readProfileTab(legacyTab ?? "", own) ?? "overview"));
  }
  const tab = readProfileTab(params.tab, own);
  if (tab === null) throw redirect(profileHref(address));
  if (raw !== address) throw redirect(profileHref(address, tab));

  const client = new QueryClient(QUERY_CLIENT_CONFIG);
  try {
    await Promise.all([client.fetchQuery(profileQuery(address)), prefetchTab(client, address, tab)]);
  } catch {
    throw data({ address, reason: "unavailable" } satisfies ProfileRouteError, { status: 503 });
  }
  const dehydratedState = dehydrate(client);
  client.clear();

  return session.wrap({ sid: session.sid, address, tab, own, dehydratedState } satisfies ProfileLoaderData);
}

/** `/profile`, `/profile/me[/:tab]`: the signed-in member's own profile, or sign-in first. */
export function ownProfileRedirect(request: Request, tab?: string): never {
  const wallet = sidLoader(request).wallet;
  if (!wallet) {
    const url = new URL(request.url);
    throw redirect(`/auth/login?redirectTo=${encodeURIComponent(url.pathname + url.search)}`);
  }
  throw redirect(profileHref(wallet, readProfileTab(tab, true) ?? "overview"));
}

/** `/profile/accounts/:address[/:tab]`, the profile site's old paths. */
export function legacyAccountsRedirect(params: { address?: string; tab?: string }): never {
  throw redirect(`${profileHref(params.address ?? "")}${params.tab ? `/${encodeURIComponent(params.tab)}` : ""}`);
}
