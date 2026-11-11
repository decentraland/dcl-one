import { dehydrate, QueryClient, type DehydratedState } from "@tanstack/react-query";
import { data, redirect, type RouterContextProvider } from "react-router";

import { QUERY_CLIENT_CONFIG } from "@/core/lib/query/defaults";
import { sidLoader } from "@/core/lib/telemetry/session";
import { readWallet } from "@/data/lib/auth/wallet-cookie";
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
import { isRouteFatal, type ProfileErrorReason } from "@/features/profile/errors";
import { isOwnProfile, profileHref, readProfileTab, type ProfileTab } from "@/features/profile/tabs";

export type ProfileLoaderData = {
  sid: string;
  address: string;
  /** Whether the signed-in wallet's cookie names this profile, so the first render matches the server. */
  own: boolean;
  dehydratedState: DehydratedState;
};

/** The body of the status-bearing response the route's boundary renders on a document request. */
export type ProfileErrorData = { address: string; reason: ProfileErrorReason };

/** `path` with the request's query, minus the parameters a redirect has already turned into the path. */
function keepQuery(path: string, url: URL): string {
  const query = new URLSearchParams(url.search);
  query.delete("tab");
  query.delete("own");
  const rest = query.toString();
  return rest ? `${path}?${rest}` : path;
}

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
  context: Readonly<RouterContextProvider>,
  params: { address?: string; tab?: string },
): Promise<ReturnType<ReturnType<typeof sidLoader>["wrap"]>> {
  const url = new URL(request.url);
  const raw = params.address ?? "";
  const address = normalizeAddress(raw);
  if (!canReadProfile(address)) {
    throw data({ address: raw, reason: "not-found" } satisfies ProfileErrorData, { status: 404 });
  }

  const session = sidLoader(request, context);
  // The wallet cookie every sign-in method sets (the root loader reads the same one); the verified
  // cookie only exists after a thirdweb sign-in.
  const own = isOwnProfile(address, readWallet(request));

  const legacyTab = url.searchParams.get("tab");
  if (legacyTab !== null || url.searchParams.has("own")) {
    throw redirect(keepQuery(profileHref(address, readProfileTab(legacyTab ?? "", own) ?? "overview"), url));
  }
  const tab = readProfileTab(params.tab, own);
  if (tab === null) throw redirect(keepQuery(profileHref(address), url));
  const canonical = profileHref(address, tab);
  if (url.pathname !== canonical) throw redirect(keepQuery(canonical, url));

  // A visitor already gone is not read for; one who leaves mid-load stops the reads.
  request.signal.throwIfAborted();
  const client = new QueryClient(QUERY_CLIENT_CONFIG);
  const cancel = () => void client.cancelQueries();
  request.signal.addEventListener("abort", cancel, { once: true });
  try {
    await Promise.all([client.fetchQuery(profileQuery(address)), prefetchTab(client, address, tab)]);
  } catch (error) {
    // A profile read's failure is 503; anything else is a bug and stays a 500.
    if (!isRouteFatal(error)) throw error;
    throw data({ address, reason: "unavailable" } satisfies ProfileErrorData, { status: 503 });
  } finally {
    request.signal.removeEventListener("abort", cancel);
  }
  const dehydratedState = dehydrate(client);
  client.clear();

  return session.wrap({ sid: session.sid, address, own, dehydratedState } satisfies ProfileLoaderData);
}

/** `/profile`, `/profile/me[/:tab]`: the signed-in member's own profile, or sign-in first. */
export function ownProfileRedirect(request: Request, tab?: string): never {
  const url = new URL(request.url);
  const wallet = readWallet(request);
  if (!wallet) {
    throw redirect(`/auth/login?redirectTo=${encodeURIComponent(url.pathname + url.search)}`);
  }
  // `/profile/me?tab=places`, the query form of the profile site's old links, keeps its tab.
  const requested = tab ?? url.searchParams.get("tab") ?? undefined;
  throw redirect(keepQuery(profileHref(wallet, readProfileTab(requested, true) ?? "overview"), url));
}

/** `/profile/accounts/:address[/:tab]`, the profile site's old paths. */
export function legacyAccountsRedirect(request: Request, params: { address?: string; tab?: string }): never {
  const url = new URL(request.url);
  const path = `${profileHref(params.address ?? "")}${params.tab ? `/${encodeURIComponent(params.tab.toLowerCase())}` : ""}`;
  throw redirect(keepQuery(path, url));
}
