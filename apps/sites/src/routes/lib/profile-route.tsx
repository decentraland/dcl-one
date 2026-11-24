import { HydrationBoundary } from "@tanstack/react-query";
import { useParams, useRouteError, useRouteLoaderData } from "react-router";

import { ProfileError } from "@dcl/one-ui";

import { normalizeAddress } from "@/data/lib/catalyst/social/profile";
import { profileKeys } from "@/data/lib/profile/queries";
import { isProfileError, profileErrorAddress, profileErrorReason, type ProfileErrorReason } from "@/features/profile/errors";
import { ProfilePage } from "@/features/profile/ProfilePage";
import { profileHref } from "@/features/profile/tabs";
import { useFailedReadRecovery } from "@/features/components/useFailedReadRecovery";

import type { ProfileLoaderData } from "./profile-loader.server";
import { absoluteImage, absoluteUrl, notFoundMeta, publicBase, socialMeta } from "./social-meta";

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
      <ProfilePage
        address={params.address ?? ""}
        tabSegment={params.tab}
        ownOnServer={loaderData?.own}
        sid={sid}
      />
    </HydrationBoundary>
  );
}

export const PROFILE_DESCRIPTION = "Meet this Decentraland member: see their creations, communities, places and photos.";

/**
 * A profile's link-preview card, as decentraland.org's: `{name} - Decentraland Profile`, the square
 * face as a small card, every tab canonical to the profile itself. An address that cannot be a
 * wallet is a missing page; any other state (a failed read, a client navigation) is the generic
 * profile card.
 */
export function profileMeta({
  loaderData,
  params,
  error,
  matches,
}: {
  loaderData?: ProfileLoaderData | null;
  params: { address?: string };
  error?: unknown;
  matches: Parameters<typeof publicBase>[0];
}) {
  if (isProfileError(error) && profileErrorReason(error) === "not-found") return notFoundMeta();
  const base = publicBase(matches);
  const card = loaderData?.card;
  const address = loaderData?.address ?? normalizeAddress(params.address);
  return socialMeta({
    title: card?.name ? `${card.name} - Decentraland Profile` : "Decentraland Profile",
    description: card?.description ?? PROFILE_DESCRIPTION,
    image: absoluteImage(base, card?.image),
    smallImage: true,
    type: "profile",
    url: address ? absoluteUrl(base, profileHref(address)) : undefined,
  });
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

/**
 * The document request's 404/503 response and a client navigation's failed profile read render the
 * same screen. Retry resets only failed reads without data; leaving additionally requires the read
 * to be inactive and idle. Newer data and requests remain owned by their current observers.
 */
export function ProfileErrorBoundary() {
  const error = useRouteError();
  const params = useParams();
  const address = normalizeAddress(params.address);
  // A bug is the root boundary's to show, not a profile failure.
  if (!isProfileError(error)) throw error;
  const reason = profileErrorReason(error);
  const copy = COPY[reason](profileErrorAddress(error) ?? params.address ?? "");

  const retry = useFailedReadRecovery(profileKeys.profile(address));

  return <ProfileError {...copy} onRetry={reason === "unavailable" ? retry : undefined} />;
}
