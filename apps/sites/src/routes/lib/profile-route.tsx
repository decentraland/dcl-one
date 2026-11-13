import { HydrationBoundary, type QueryClient, useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";
import { useLocation, useNavigate, useParams, useRouteError, useRouteLoaderData } from "react-router";

import { Button } from "@dcl/one-ui/apps/sites/shared/Button/Button";

import { normalizeAddress } from "@/data/lib/catalyst/social/profile";
import { profileKeys } from "@/data/lib/profile/queries";
import { isProfileError, profileErrorAddress, profileErrorReason, type ProfileErrorReason } from "@/features/profile/errors";
import { ProfilePage } from "@/features/profile/ProfilePage";

import type { ProfileLoaderData } from "./profile-loader.server";

/** Client navigations fetch through the browser client; only the first document carries server data. */
export async function profileClientLoader() {
  return null;
}

export function ProfileRoute({ loaderData }: { loaderData: ProfileLoaderData | null | undefined }) {
  const params = useParams();
  const root = useRouteLoaderData("root") as { sid?: string | null } | undefined;
  const sid = loaderData?.sid ?? root?.sid ?? "";
  return (
    <HydrationBoundary state={loaderData?.dehydratedState}>
      <ProfilePage address={params.address ?? ""} tabSegment={params.tab} ownOnServer={loaderData?.own} sid={sid} />
    </HydrationBoundary>
  );
}

const COPY: Record<ProfileErrorReason, (address: string) => { title: string; body: string }> = {
  "not-found": (address) => ({
    title: "Profile not found",
    body: `The address "${address}" is not a valid Ethereum address.`,
  }),
  unavailable: () => ({
    title: "Profile unavailable",
    body: "We couldn't load this profile right now. Try again in a moment.",
  }),
};

function resetFailedProfile(client: QueryClient, address: string) {
  return client.resetQueries({
    queryKey: profileKeys.profile(address),
    exact: true,
    predicate: ({ state }) => state.status === "error" && state.data === undefined,
  });
}

/**
 * The document request's 404/503 response and a client navigation's failed profile read render the
 * same screen. Retry and leaving reset only failed reads without data; newer successful or pending
 * reads remain owned by their current observers.
 */
export function ProfileErrorBoundary() {
  const error = useRouteError();
  const params = useParams();
  const client = useQueryClient();
  const navigate = useNavigate();
  const location = useLocation();
  const address = normalizeAddress(params.address);
  // A bug is the root boundary's to show, not a profile failure.
  if (!isProfileError(error)) throw error;
  const reason = profileErrorReason(error);
  const copy = COPY[reason](profileErrorAddress(error) ?? params.address ?? "");

  useEffect(
    () => () => {
      void resetFailedProfile(client, address);
    },
    [client, address],
  );

  const retry = async () => {
    await resetFailedProfile(client, address);
    // A navigation to the same URL clears the router's error, which a revalidation does not for an
    // error thrown while rendering.
    void navigate(`${location.pathname}${location.search}`, { replace: true });
  };

  return (
    <main className="landings-profile" role="alert">
      <h1>{copy.title}</h1>
      <p>{copy.body}</p>
      {reason === "unavailable" ? (
        <Button variant="secondary" onClick={retry}>
          Try again
        </Button>
      ) : null}
    </main>
  );
}
